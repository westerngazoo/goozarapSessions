# R-0001 — Frequency-ratio core

- **Status:** Accepted
- **Milestone:** M1
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-06-10
- **Depends on:** none
- **Realized by:** SPEC-0001
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must have an exact frequency-ratio core: the mathematical
foundation every pitch-related feature quantizes against. It must represent
pitch relationships as exact small-integer ratios (3:2, 5:4, …), combine and
invert them without rounding error, fold any ratio into a single octave
(octave equivalence), order ratios by *consonance* so the UI can expose a
"smooth↔tense" control with no music-theory vocabulary, build pitch grids
from the harmonic series, and snap an arbitrary frequency in Hz (e.g. a
tracked vocal pitch) onto the nearest grid pitch in the correct octave.

This is pitch only. Rhythm and beat ratios are R-0002.

## 2. Rationale

The product promise is "no music knowledge needed": users never see note
names or scales — they pick ratios (directly or via sliders) and the app
renders them musically. Every later pipeline depends on this core: voice-to-
riff quantizes tracked pitches onto its grids (M2), the synthesizers tune to
its frequencies (M2), influence models bias *which* ratios are favoured (M4).
It must therefore be exact (no floating-point drift when stacking intervals),
deterministic, and dependency-free, so everything above it is testable.

## 3. Acceptance criteria

- **AC1 — Canonical form.** A ratio is always stored reduced to lowest terms:
  constructing 6:4 yields a value equal to 3:2 (equality, hashing, and
  ordering all agree). Construction with a zero numerator or denominator is
  rejected with a typed error; the library never panics.
- **AC2 — Exact interval arithmetic.** Stacking is exact rational
  multiplication: (3:2)·(4:3) = 2:1 precisely. Unstacking (division) and
  inversion are exact; 1:1 is the identity. Arithmetic that would overflow
  the integer representation surfaces a typed error — never a wraparound or
  panic.
- **AC3 — Octave equivalence.** Any ratio can be reduced into [1, 2). A ratio
  r and its octave shifts (2r, 4r, r/2, …) reduce to the same value, and
  reduction is idempotent.
- **AC4 — Consonance ordering.** Ratios expose a complexity measure such that
  sorting the classic just intervals ascending yields the canonical
  consonance order: 1:1 (unison) < 2:1 (octave) < 3:2 (fifth) < 4:3 (fourth)
  < 5:3 (major sixth) < 5:4 (major third) < 6:5 (minor third) < 9:8 < 16:15.
  This measure is what the "smooth↔tense" slider walks.
- **AC5 — Harmonic pitch grids.** A grid built from the harmonic series with
  odd limit L contains exactly the octave-reduced ratios of the odd harmonics
  1, 3, 5, …, L — deduplicated, sorted ascending, starting at 1:1. (Example,
  L = 9: 1:1, 9:8, 5:4, 3:2, 7:4.)
- **AC6 — Frequency mapping & snapping.** Given a root frequency, a ratio
  maps to Hz exactly (3:2 over root 220 Hz → 330 Hz). Snapping an arbitrary
  positive frequency returns the nearest grid pitch *in the correct octave*
  (660 Hz against a root of 220 Hz with 3:2 on the grid snaps to 660 Hz =
  3:2 one octave up, not 330 Hz). Frequencies already on the grid are fixed
  points; snapping is idempotent; the result reports its offset in cents.
  Non-finite or non-positive frequencies are rejected with a typed error.
- **AC7 — Cents.** A ratio reports its size in cents: 2:1 → 1200 ¢ exactly,
  1:1 → 0 ¢, 3:2 → 701.955 ¢ (within 0.001 ¢).
- **AC8 — Documented public API.** Every public item carries documentation
  with a runnable example (doc tests pass), and the crate builds with all
  four toolchain gates green.

## 4. Constraints & non-goals

- Pure math: no audio I/O, no device access, no allocation in hot paths
  (snapping), no external dependencies (std only).
- Rhythm/beat ratios, Euclidean patterns, time quantization: **R-0002**.
- Tempered/EDO tunings and arbitrary scale imports: Advanced Mode, later
  milestone.
- Choice of *which* grids/sliders the UI exposes: an app concern, not this
  crate's.

## 5. Open questions

None — settled in the decision log below.

## 6. Decision log

Decisions made together (owner + Claude). Append-only.

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-10 | Hand-roll the rational type instead of using `num-rational` | Zero-dependency core; we need domain ops (octave reduction, Tenney height, grid snapping) that the crate would not provide; the rational arithmetic itself is small and fully testable. |
| 2026-06-10 | Complexity = Tenney height, log₂(n·d) | Standard measure in just-intonation theory; monotone in n·d; reproduces the canonical consonance ranking (AC4) with a single formula. |
| 2026-06-10 | `u64` components + checked arithmetic | Plenty of headroom for musical ratios; overflow is a typed error per the constitution ("no unchecked failures in library code"). |
| 2026-06-10 | Owner's "yeah do next" delegates this iteration | Requirement drafted and driven autonomously; owner review lands on the PR. Amendments welcome — this log stays append-only. |

## Changelog

- 2026-06-10 — created, accepted for M1.
