# SPEC-0001 — Frequency-ratio core

- **Status:** Implemented — QA PASS, architect APPROVE (awaiting owner PR review)
- **Realizes:** R-0001
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-06-10
- **Depends on:** none
- **Module(s):** `crates/gooz-ratio`

## 1. Motivation

Realizes R-0001: the exact, dependency-free pitch-math foundation. Everything
pitch-shaped in later milestones (voice-to-riff quantization, synth tuning,
influence-model bias) calls into this crate, so its API must be exact,
panic-free, and small.

## 2. Design

Three responsibility modules (plus `lib.rs` for docs/re-exports only) inside
`gooz-ratio`, no external dependencies:

```
crates/gooz-ratio/src/
├── lib.rs       crate docs, re-exports (Ratio, PitchGrid, SnappedPitch, RatioError)
├── error.rs     RatioError — the crate's single typed error
├── ratio.rs     Ratio — exact positive rational + interval operations
└── grid.rs      PitchGrid — root frequency + octave-reduced degrees + snapping
```

### `Ratio` — exact positive rational

- Representation: `{ num: u64, den: u64 }`, invariant **reduced to lowest
  terms, both components ≥ 1**. The invariant is established in `new` (Euclid
  gcd) and preserved by every operation, so derived `PartialEq`/`Eq`/`Hash`
  are correct (AC1).
- Total ordering by musical size: cross-multiplication in `u128` so comparing
  never overflows (`self.num·other.den` vs `other.num·self.den`).
- Operations (AC2, AC3, AC7):
  - `stack`/`unstack` — exact multiplication/division with **gcd
    cross-cancellation first** (`gcd(self.num, other.den)` and
    `gcd(other.num, self.den)`), then `checked_mul`; the result is in lowest
    terms by construction and exactly-representable results near the u64
    boundary do not spuriously overflow. True overflow →
    `RatioError::Overflow`.
  - `invert` — swap components (cannot fail under the invariant; gcd is
    symmetric).
  - `reduce_to_octave` — canonical-preserving octave steps until the value is
    in [1, 2): while ≥ 2, *halve `num` if even, else double `den`*; while
    < 1, *halve `den` if even, else double `num`*. Each step keeps gcd = 1,
    so the lowest-terms invariant survives without re-reduction, and the
    halving direction shrinks components. Doubling uses checked arithmetic;
    a ratio whose octave-reduced form is not u64-representable (e.g.
    1:(2⁶⁴−1) → 2⁶⁴:(2⁶⁴−1)) surfaces `RatioError::Overflow` — AC3's "any
    ratio" is bounded by representability. Idempotent by construction
    (no-op when already in [1, 2)).
  - `complexity` — Tenney height `log₂(num·den)`, computed in `f64` (a
    metric, not an exact quantity; product computed in `f64` so it cannot
    overflow) (AC4).
  - `cents` — `1200·log₂(num/den)` (AC7).
  - `to_hz(root_hz)` — computed as exactly
    `root_hz * (num as f64 / den as f64)`; this single formula and
    evaluation order is shared verbatim by `PitchGrid::snap` when it
    recomputes the snapped frequency, so `to_hz` outputs are bitwise fixed
    points of `snap` (AC6). Validates `root_hz` finite and positive.
- Constants: `Ratio::UNISON` (1:1), `Ratio::OCTAVE` (2:1).

### `PitchGrid` — degrees + snapping

- Representation: `{ root_hz: f64, degrees: Vec<Ratio> }`, invariant: degrees
  octave-reduced, sorted ascending, deduplicated, first degree 1:1,
  `root_hz` finite and positive.
- Constructors (both validate `root_hz` finite and positive →
  `RatioError::InvalidFrequency`):
  - `from_ratios(root_hz, ratios)` — octave-reduces, sorts, dedups, inserts
    1:1; empty input → `RatioError::EmptyGrid`.
  - `harmonic(root_hz, odd_limit)` — degrees from the odd harmonics
    `1, 3, 5, … ≤ odd_limit` as `h:1` octave-reduced (AC5). An even
    `odd_limit` simply bounds the same odd set (`harmonic(root, 8)` ≡
    `harmonic(root, 7)`); `odd_limit` of 0 → `RatioError::EmptyGrid`.
- `snap(hz)` (AC6): work in log-frequency where octaves are unit steps.
  `t = log₂(hz / root_hz)`; reject a non-finite `t` with
  `RatioError::InvalidFrequency` (guards quotient overflow/underflow such as
  `1e308 / 1e-308`, and bounds `octave` well inside `i32`). Split into
  `octave = ⌊t⌋` and fractional position `frac ∈ [0, 1)`. Each degree sits
  at `p = log₂(degree) ∈ [0, 1)`. For every degree consider the three
  candidates `p − 1`, `p`, `p + 1` (previous/this/next octave) and take the
  minimum `|frac − candidate|` — this handles wrap-around (a frequency just
  under the next octave's unison). **Ties resolve to the lower-pitched
  candidate** (ascending-degrees scan keeps a strictly-better-only update),
  making the result deterministic and testable. O(3·n) comparisons, no
  allocation. Returns:

  ```
  SnappedPitch { degree: Ratio, octave: i32, hz: f64, cents_offset: f64 }
  ```

  - `octave` — whole octaves above the root: 0 ⇔ snapped hz ∈
    [root, 2·root); may be negative.
  - `hz` — **recomputed** from `(degree, octave)` via the shared `to_hz`
    formula (never echoes the input), which is what makes grid pitches
    bitwise fixed points and snapping idempotent.
  - `cents_offset` — input relative to snapped (input − snapped, in cents).

  Non-finite/non-positive input → `RatioError::InvalidFrequency`.

### `RatioError`

```rust
pub enum RatioError {
    ZeroComponent,      // ratio with a zero numerator or denominator
    Overflow,           // exact arithmetic exceeded u64
    InvalidFrequency,   // non-finite or non-positive Hz
    EmptyGrid,          // grid construction with no degrees
}
```

Implements `Display` + `std::error::Error`. The crate has **no** `panic!`,
`unwrap`, or `expect` on library paths (AC1, AC8).

## 3. Code outline

Representative shape (not final):

```rust
// ratio.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ratio {
    num: u64,
    den: u64,
}

impl Ratio {
    pub const UNISON: Ratio = Ratio { num: 1, den: 1 };
    pub const OCTAVE: Ratio = Ratio { num: 2, den: 1 };

    pub fn new(num: u64, den: u64) -> Result<Ratio, RatioError> {
        if num == 0 || den == 0 {
            return Err(RatioError::ZeroComponent);
        }
        let g = gcd(num, den);
        Ok(Ratio { num: num / g, den: den / g })
    }

    pub fn stack(self, other: Ratio) -> Result<Ratio, RatioError> {
        // cross-cancel before multiplying: result is reduced by construction
        let ga = gcd(self.num, other.den);
        let gb = gcd(other.num, self.den);
        let num = (self.num / ga).checked_mul(other.num / gb).ok_or(RatioError::Overflow)?;
        let den = (self.den / gb).checked_mul(other.den / ga).ok_or(RatioError::Overflow)?;
        Ok(Ratio { num, den })
    }

    pub fn reduce_to_octave(self) -> Result<Ratio, RatioError> {
        /* canonical-preserving octave steps — see §2 */
    }
    pub fn complexity(self) -> f64 { (self.num as f64 * self.den as f64).log2() }
    pub fn cents(self) -> f64 { 1200.0 * (self.num as f64 / self.den as f64).log2() }
}

// Ord is hand-written (u128 cross-multiplication); PartialOrd must delegate
// to Ord (clippy non_canonical_partial_ord_impl is part of the -D warnings gate).

// grid.rs
pub struct PitchGrid { root_hz: f64, degrees: Vec<Ratio> }

impl PitchGrid {
    pub fn harmonic(root_hz: f64, odd_limit: u64) -> Result<PitchGrid, RatioError> { ... }
    pub fn snap(&self, hz: f64) -> Result<SnappedPitch, RatioError> { ... }
}
```

## 4. Non-goals

- Beat/rhythm math (`E(k,n)`, bar grids) — R-0002 / SPEC-0002.
- Tempered tunings (12-EDO etc.) and scale import — Advanced Mode milestone.
- Real-time/audio integration — `gooz-audio` consumes this crate later.
- Serialization of grids — lands with the session format (R-0010).

## 5. Open questions

None — settled in the decision log.

## 6. Acceptance criteria

Each maps to R-0001's ACs; the qa agent owns the acceptance tests in
`crates/gooz-ratio/tests/acceptance_r0001.rs`, units live beside the modules.

Ticked at QA sign-off (loop step 7) — PASS, 43 acceptance/unit + 19 doc
tests, clippy `-D warnings` clean, fmt clean:

- [x] AC1 — canonical form, equality across unreduced spellings, zero rejected.
- [x] AC2 — exact stack/unstack/invert, overflow as typed error.
- [x] AC3 — octave reduction into [1,2), octave-shift invariance, idempotence,
      `Overflow` on unrepresentable reduced forms.
- [x] AC4 — Tenney complexity reproduces the canonical consonance order.
- [x] AC5 — harmonic grids: exact degree sets for odd limits (e.g. L=9, L=15).
- [x] AC6 — to_hz exactness; snapping correctness across octaves, fixed
      points, idempotence, tie-break, cents offset, invalid input errors
      (including non-finite quotient).
- [x] AC7 — cents values (1200 exact, 701.955 ± 0.001).
- [x] AC8 — doc tests on every public item; build/test/clippy/fmt green.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-10 | Snap in log-frequency with ±1-octave candidates per degree | Octave wrap-around falls out naturally; allocation-free; O(n) in grid size. |
| 2026-06-10 | `complexity`/`cents` return `f64` (not exact) | They are metrics for ordering/display; exactness lives in the rational representation, not in derived measures. |
| 2026-06-10 | Grids always contain 1:1 | The root must always be a valid snap target; simplifies octave bookkeeping. |
| 2026-06-10 | Architect review (REQUEST CHANGES) applied: canonical-preserving octave-step rule; reject non-finite log-quotient in `snap`; single pinned ratio→Hz formula + `hz` recomputed from `(degree, octave)`; gcd cross-cancellation in `stack`/`unstack`; even `odd_limit` bounds the odd set; ties snap to the lower pitch | Findings 1–6 of the architect's SPEC-0001 review — fixes keep AC1/AC3/AC6 actually testable and the lowest-terms invariant unbreakable. |

## Changelog

- 2026-06-10 — created; accepted alongside R-0001.
