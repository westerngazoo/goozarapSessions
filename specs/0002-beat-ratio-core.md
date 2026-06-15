# SPEC-0002 — Beat-ratio core

- **Status:** Implemented — QA PASS, architect APPROVE (awaiting owner PR review)
- **Realizes:** R-0002
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-06-15
- **Depends on:** SPEC-0001 (same crate)
- **Module(s):** `crates/gooz-ratio`

## 1. Motivation

Realizes R-0002: the exact, dependency-free rhythm core and the math behind the
"sparse↔busy" control. Lands in `gooz-ratio` beside the pitch core; the beat
builder (R-0009), transport (R-0004), and voice-onset quantization (R-0006)
call into it.

## 2. Design

Two new responsibility modules plus a small shared integer-math helper, all
std-only:

```
crates/gooz-ratio/src/
├── lib.rs       (+ re-exports: BarGrid, QuantizedBeat, Pattern, Polyrhythm, Tempo, BeatError)
├── math.rs      pub(crate) gcd(u64,u64)->u64 — shared by ratio.rs and beat.rs (DRY)
├── beat_error.rs BeatError — the rhythm modules' typed error
├── rhythm.rs    Pattern — Euclidean E(k,n) via Bjorklund, rotation, onsets
└── beat.rs      BarGrid + QuantizedBeat, Polyrhythm, Tempo
```

`ratio.rs`'s private `gcd` is replaced by `math::gcd` (one-line call-site
changes) so the crate has a single gcd. No other R-0001 code changes. The moved
`gcd` keeps identical semantics, including `gcd(0, n) = n` — `BarGrid::position`
relies on it to reduce the downbeat `0/steps` to `(0, 1)`. The R-0001
acceptance suite (`tests/acceptance_r0001.rs`) is the regression gate for this
move and must stay green. There is no shared `lcm` helper: the only lcm need is
`Polyrhythm::grid_steps`, which composes two `u32`s — `(a / gcd(a, b)) * b`
widened to `u64` is at most `(2³²−1)² < u64::MAX`, so it is computed inline with
a plain (non-fallible, non-panicking) multiply rather than via a general
`Option`-returning helper.

### `BeatError` — the rhythm modules' typed error

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeatError {
    EmptyGrid,     // a step or pulse count that must be >= 1 was 0
    TooManyOnsets, // k > n in E(k, n)
    InvalidPhase,  // non-finite bar phase passed to quantize
    InvalidTempo,  // non-finite or non-positive BPM / beats-per-bar
}
```

Kept separate from `RatioError` (interface segregation): the pitch and rhythm
error sets barely overlap, and `InvalidFrequency`/`InvalidTempo` should not
share a variant. Derives exactly what `RatioError` does
(`Debug, Clone, Copy, PartialEq, Eq`) and implements `Display` +
`std::error::Error` identically; no fields. Note `EmptyGrid` exists in **both**
`RatioError` ("no degrees") and `BeatError` ("a step/pulse count was 0") — same
name, different type and meaning; they never share a `Result`, and each doc
string states its own meaning. No `panic!`/`unwrap`/`expect` on library paths.

### `Pattern` — Euclidean rhythms (`rhythm.rs`)

- Representation: `{ steps: Vec<bool> }`, `true` = onset.
- `euclidean(onsets: u32, steps: u32) -> Result<Pattern, BeatError>` — validate
  first: `steps == 0` → `EmptyGrid`; `onsets > steps` → `TooManyOnsets`;
  `onsets == 0` → return an all-rest pattern of length `steps` **before**
  entering Bjorklund (the loop makes no progress with an empty onset pile — it
  must be short-circuited, not entered). Otherwise run Bjorklund: start with
  `onsets` `[true]` groups and `steps - onsets` `[false]` groups, repeatedly
  distribute the smaller pile onto the larger until at most one group remains in
  the remainder, then concatenate. Yields a maximally-even pattern with exactly
  `onsets` `true`s, first step `true` when `onsets > 0`; `onsets == steps`
  yields all-onset naturally (AC1, AC2).
- Accessors: `len`, `is_empty`, `onset_count`, `is_onset(i)`, `steps() -> &[bool]`,
  `onsets() -> Vec<usize>` (ascending onset indices).
- `rotate(by: i64) -> Pattern` — cyclic shift; `by` reduced modulo `len`
  (handles negative and multiples-of-len = identity); preserves length and
  onset count (AC3).

### `BarGrid` + `QuantizedBeat` (`beat.rs`)

- `BarGrid { steps: u32 }`, invariant `steps >= 1`.
- `new(steps) -> Result<BarGrid, BeatError>` (`0` → `EmptyGrid`); `steps()`.
- `position(index: u32) -> (u64, u64)` — the exact reduced fraction
  `(index mod steps) / steps` of the bar, downbeat `(0, 1)` (AC4).
- `phase(index: u32) -> f64` — `(index mod steps) as f64 / steps as f64`.
- `quantize(phase: f64) -> Result<QuantizedBeat, BeatError>` (AC5): reject
  non-finite `phase` (`InvalidPhase`). Let `scaled = phase * steps`; the nearest
  step with **ties to the earlier step** is `m = (scaled - 0.5).ceil() as i64`;
  `target_phase = m as f64 / steps`; `offset = phase - target_phase`;
  `step = ((m % steps) + steps) % steps`. Wrap-around falls out: a phase just
  below the barline rounds `m` up to `steps`, giving `step 0` with a small
  negative offset. The snapped step's own phase is a fixed point and quantizing
  is idempotent. No allocation. Contract: `phase` is conventionally in `[0, 1)`;
  a negative phase is well-defined (it rotates backwards — `-0.30` on an 8-step
  grid → step 3); an out-of-`[0,1)` *finite* phase is accepted with defined,
  non-panicking behavior (the `f64`→`i64` cast saturates rather than wrapping at
  pathological magnitudes); only a non-finite phase is an error.

```rust
pub struct QuantizedBeat {
    pub step: u32,   // grid step in 0..steps
    pub phase: f64,  // that step's phase = step / steps
    pub offset: f64, // input − snapped target, in fractions of a bar (signed)
}
```

### `Polyrhythm` (`beat.rs`)

- `Polyrhythm { a: u32, b: u32 }`, invariant `a, b >= 1`.
- `new(a, b) -> Result<Polyrhythm, BeatError>` (`0` → `EmptyGrid`).
- `grid_steps() -> u64` — `lcm(a, b)` as `(a / gcd(a, b)) * b` with `a, b`
  widened from `u32` to `u64`, the shared grid both pulse streams align on
  (AC6). The `u32 × u32` domain is provably overflow-free in `u64` (worst case
  `(2³²−1)² < u64::MAX`), so this uses a plain multiply — no `Option`, no
  `unwrap`/`expect`, no panic path.
- `a_pulses() -> Vec<(u64, u64)>` / `b_pulses() -> Vec<(u64, u64)>` — the evenly
  spaced pulse positions as reduced bar fractions `i/a` (`i` in `0..a`) and
  `i/b`. For `3:2`: `a_pulses = [(0,1),(1,3),(2,3)]`, `b_pulses = [(0,1),(1,2)]`.

### `Tempo` (`beat.rs`)

- `Tempo { bpm: f64, beats_per_bar: f64 }`.
- `new(bpm, beats_per_bar) -> Result<Tempo, BeatError>` — both must be finite
  and positive, else `InvalidTempo`.
- `seconds_per_beat() -> f64` = `60.0 / bpm` (120 BPM → `0.5`, exactly).
- `bar_seconds() -> f64` = `beats_per_bar * 60.0 / bpm`.
- `step_time(phase: f64) -> f64` = `phase * bar_seconds()` (AC7).

## 3. Code outline

```rust
// rhythm.rs
pub fn euclidean(onsets: u32, steps: u32) -> Result<Pattern, BeatError> {
    if steps == 0 { return Err(BeatError::EmptyGrid); }
    if onsets > steps { return Err(BeatError::TooManyOnsets); }
    if onsets == 0 { return Ok(Pattern { steps: vec![false; steps as usize] }); }
    // Bjorklund: groups of [true] and [false], distribute remainder repeatedly.
    // onsets >= 1 here, so the loop always makes progress and terminates.
    let mut a: Vec<Vec<bool>> = (0..onsets).map(|_| vec![true]).collect();
    let mut b: Vec<Vec<bool>> = (onsets..steps).map(|_| vec![false]).collect();
    while b.len() > 1 {
        let count = a.len().min(b.len());
        let mut next_a = Vec::new();
        for i in 0..count { let mut g = a[i].clone(); g.extend(b[i].clone()); next_a.push(g); }
        let remainder = if a.len() > count { a[count..].to_vec() } else { b[count..].to_vec() };
        a = next_a;
        b = remainder;
    }
    let steps: Vec<bool> = a.into_iter().chain(b).flatten().collect();
    Ok(Pattern { steps })
}

// beat.rs
pub fn quantize(&self, phase: f64) -> Result<QuantizedBeat, BeatError> {
    if !phase.is_finite() { return Err(BeatError::InvalidPhase); }
    let n = self.steps as f64;
    let m = (phase * n - 0.5).ceil() as i64;            // nearest, ties to earlier
    let target = m as f64 / n;
    let step = ((m % self.steps as i64) + self.steps as i64) % self.steps as i64;
    Ok(QuantizedBeat { step: step as u32, phase: step as f64 / n, offset: phase - target })
}
```

## 4. Non-goals

- Pitch/frequency ratios — R-0001 (already in `gooz-ratio`).
- Audio scheduling / the clock thread — `gooz-audio`, R-0004.
- Swing, groove humanization, tempo curves, non-isochronous meter — Advanced Mode.
- Mapping recorded-voice onsets onto the grid — R-0006 (consumes this core).

## 5. Open questions

None — settled in the decision log.

## 6. Acceptance criteria

Maps to R-0002 AC1–AC8; qa owns `tests/acceptance_r0002.rs`.

- [x] AC1 — `E(k,n)` exact onset count + known patterns (3,8)/(5,8)/(4,16),
      first step onset, deterministic.
- [x] AC2 — boundaries: `k=0` all rests, `k=n` all onsets, `k>n` + `n=0` typed
      errors.
- [x] AC3 — rotation preserves count/length; multiples of `n` are identity;
      negative offsets valid.
- [x] AC4 — bar grid: `n` positions, exact `i/n`, downbeat 0, ascending; `n=0`
      error.
- [x] AC5 — quantize nearest step, wrap-around, idempotence, signed offset,
      earlier-step tie-break, non-finite rejected.
- [x] AC6 — polyrhythm on `lcm(a,b)`; `3:2` pulse fractions exact; `0` error.
- [x] AC7 — tempo: `60/BPM` per beat (120→0.5 exact), `step_time`, invalid
      tempo rejected.
- [x] AC8 — doc tests on every public item; build/test/clippy/fmt green.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-15 | Integer step grids (not `Ratio`) for beat positions | `Ratio` forbids a zero numerator; the downbeat is 0. Integers represent 0 and keep lcm composition exact. |
| 2026-06-15 | Dedicated `BeatError`, not shared `RatioError` | Interface segregation; pitch and rhythm error sets barely overlap. |
| 2026-06-15 | Shared `pub(crate) math::{gcd,lcm}`; `ratio.rs` refactored to use it | One gcd in the crate (DRY); the only R-0001 change is call-sites. |
| 2026-06-15 | Tie-break to the earlier step via `(scaled-0.5).ceil()` | Deterministic and testable; `f64::round` ties away-from-zero, the wrong direction. |
| 2026-06-15 | Architect review (REQUEST CHANGES) applied: short-circuit `onsets==0` before Bjorklund (the loop hangs on an empty onset pile — a worse failure than a panic); quantize cast contract documented; `BeatError` derives/`Display` named; `gcd(0,n)=n` preserved through the `math.rs` move with the R-0001 suite as regression gate | Findings 1–5 of the SPEC-0002 review. Finding 1 was blocking (non-terminating `E(0,n)`). |
| 2026-06-15 | QA sign-off note resolved: `grid_steps` computes lcm inline with a plain multiply (provably overflow-free over the `u32` domain) instead of a general `Option`-returning `lcm` + `expect` | Removes the only `expect` on a library path — the no-panic guarantee is structural, not justified-unreachable; also drops a now-unused helper. |

## Changelog

- 2026-06-15 — created; accepted alongside R-0002.
