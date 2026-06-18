# R-0002 — Beat-ratio core

- **Status:** Accepted
- **Milestone:** M1
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-06-15
- **Depends on:** R-0001 (shares the `gooz-ratio` crate; reuses its exact `Ratio`)
- **Realized by:** SPEC-0002
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must have an exact beat-ratio core: the rhythmic counterpart to
the frequency-ratio core, and the math behind the "sparse↔busy" control. It
must divide a bar into equal steps and address them as exact fractions of the
bar (the downbeat is position 0); generate Euclidean rhythms `E(k, n)` that
distribute `k` onsets as evenly as possible across `n` steps (the family that
underlies most latin/trap grooves, e.g. the tresillo `E(3, 8)`); rotate those
patterns; compose two pulse streams into a polyrhythm on their shared grid
(e.g. 3-against-2); quantize an arbitrary position within a bar onto the
nearest grid step; and convert grid positions to wall-clock time given a
tempo. It is rhythm only — pitch is R-0001.

## 2. Rationale

Rhythm is the other half of "no music knowledge": the user never sets a time
signature or reads notation — they move a "sparse↔busy" slider that walks the
onset count of a Euclidean pattern, and the app renders a groove. The beat
builder (R-0009), the ratio-locked transport (R-0004), and voice-onset
quantization (R-0006) all stand on this core, so — like R-0001 — it must be
exact (no drift when subdividing or composing grids), deterministic, and
dependency-free, so everything above it is testable.

## 3. Acceptance criteria

- **AC1 — Euclidean distribution.** `E(k, n)` returns a length-`n` pattern with
  exactly `k` onsets, placed by Bjorklund's algorithm (maximally even). It is
  deterministic and matches known patterns: `E(3, 8)` has onsets at steps
  `{0, 3, 6}` (tresillo); `E(5, 8)` at `{0, 2, 3, 5, 6}` (cinquillo); `E(4, 16)`
  at `{0, 4, 8, 12}`. The first step is always an onset when `k > 0`.
- **AC2 — Euclidean boundaries.** `k = 0` yields all rests; `k = n` yields all
  onsets; `k > n` is rejected with a typed error; `n = 0` is rejected with a
  typed error. The library never panics.
- **AC3 — Rotation.** Rotating a pattern preserves its onset count and length;
  rotating by `0` or by a whole multiple of `n` is the identity; rotation is
  well-defined for any integer offset (negative rotates the other way).
- **AC4 — Bar grid.** A bar divided into `n` steps exposes `n` positions; step
  `i` is the exact fraction `i/n` of the bar with the downbeat at `0`;
  positions are strictly ascending in `[0, 1)`; `n = 0` is a typed error.
- **AC5 — Time quantization.** Snapping an arbitrary bar phase (a real number,
  conventionally in `[0, 1)`) returns the nearest step on an `n`-step grid,
  with wrap-around: a phase just below the barline snaps to step `0`. The
  snapped step's own phase is a fixed point; snapping is idempotent; the
  result reports the signed offset (input − snapped) in fractions of a bar;
  exact ties resolve deterministically to the earlier step. Non-finite input
  is rejected with a typed error.
- **AC6 — Polyrhythm.** Composing pulse streams `a:b` places them on the
  `lcm(a, b)`-step grid: `a` evenly spaced pulses and `b` evenly spaced
  pulses. For `3:2` the `a`-pulses fall at bar fractions `{0, 1/3, 2/3}` and
  the `b`-pulses at `{0, 1/2}`; positions are exact. `a` or `b` of `0` is a
  typed error.
- **AC7 — Tempo mapping.** Given a tempo in BPM, one beat lasts `60 / BPM`
  seconds and a bar of `beats_per_bar` beats lasts `beats_per_bar · 60 / BPM`
  seconds; a grid step at phase `p` occurs at `p · bar_seconds`. A non-finite
  or non-positive BPM (or beats-per-bar) is rejected with a typed error;
  `120 BPM` gives `0.5 s` per beat, exactly.
- **AC8 — Documented public API.** Every public item carries documentation with
  a runnable example (doc tests pass), and the crate builds with all four
  toolchain gates green.

## 4. Constraints & non-goals

- Pure math: no audio, no scheduling/clock thread (that is `gooz-audio`,
  R-0004), no allocation on the quantization hot path, std-only.
- Frequency/pitch ratios: **R-0001** (already in `gooz-ratio`).
- Swing/groove humanization, tempo curves, and non-isochronous meters:
  Advanced Mode, later milestone.
- Mapping a recorded voice's onsets onto the grid: **R-0006** (consumes this
  core); this requirement only provides the grid + quantizer.

## 5. Open questions

None — settled in the decision log below.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-15 | Beat positions are integer step indices on an `n`-step grid, exposed as exact `i/n` bar fractions — not `Ratio` | `Ratio` (R-0001) forbids a zero numerator, but the downbeat is position 0; an integer step grid represents 0 exactly and keeps composition (lcm) exact. |
| 2026-06-15 | Euclidean generation via Bjorklund's algorithm | The canonical maximally-even construction; reproduces the standard `E(k,n)` patterns the groove templates expect. |
| 2026-06-15 | Quantization works in normalized bar phase `[0,1)`; tempo→seconds is a separate concern | Keeps the grid tempo-independent and pure; mirrors R-0001's split of exact ratio vs `to_hz`. |
| 2026-06-15 | Owner delegated this iteration ("lets go next dev task… no pressure") | Requirement drafted and driven autonomously toward a reviewable PR; owner review lands on the PR. Decision log is append-only — amendments welcome. |

## Changelog

- 2026-06-15 — created, accepted for M1.
