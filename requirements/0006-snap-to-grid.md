# R-0006 — Snap-to-grid (quantize notes onto the ratio grids)

- **Status:** Accepted
- **Milestone:** M2
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-06-18
- **Depends on:** R-0001 (`PitchGrid`), R-0002 (`Tempo`/beat grid), R-0005 (note events)
- **Realized by:** SPEC-0006
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must quantize the note events produced by R-0005 onto the ratio
grids: each note's **pitch** snaps to the nearest degree of a caller-supplied
frequency grid (the right ratio in the right octave), and each note's **timing**
— its start and its end — snaps to the nearest step of the beat grid derived
from a caller-supplied tempo and subdivision. The result is a list of
fully grid-locked notes, each carrying its snapped grid pitch (degree, octave,
frequency), how far the original pitch was from the grid (in cents), and its
snapped onset and duration in whole grid steps. This is the step that turns a
loose hum into something that lands on the grid — "make it sound right."

It uses the existing exact math: `gooz-ratio`'s `PitchGrid::snap` for pitch and
its beat math (`Tempo`) for time. It does not detect the key or the tempo; the
caller provides both. Rendering the quantized notes into an instrument is
R-0007; the full record→riff pipeline is R-0008.

## 2. Rationale

A hum is never exactly in tune or exactly on the beat. The whole "no music
knowledge" promise depends on the app fixing that automatically: the user makes
an approximate sound and it comes back locked to the grid. R-0005 hears *what*
was sung; R-0006 decides *where on the grid it belongs*. Snapping pitch, onset,
and duration all to the grid is what makes a wobbly hum read as a played-in
riff. Keeping the grids caller-supplied (root + tempo) keeps this step exact,
deterministic, and testable, and defers key/tempo detection to later features.

## 3. Acceptance criteria

- **AC1 — Pitch snaps to the nearest grid degree, correct octave.** Given a
  `PitchGrid` and a note whose pitch is near a degree, the quantized note's
  pitch is that degree's exact grid frequency in the correct octave (e.g. on a
  grid rooted at 220 Hz, a note hummed at 446 Hz snaps to 440 Hz — the unison
  one octave up). The grid degree (ratio) and octave are reported.
- **AC2 — Cents offset reports how far the hum was.** Each quantized note
  reports the signed distance of its original pitch from the snapped grid pitch
  in cents (input − snapped): a sharp hum is positive, a flat hum negative,
  an on-pitch hum ≈ 0.
- **AC3 — Onset snaps to the nearest beat-grid step.** With a tempo and
  subdivision giving a step duration `step_secs`, a note's onset snaps to the
  nearest whole step (`round(onset / step_secs)`); the snapped onset time and
  the step index are reported. A note starting at `t = 0` snaps to step 0.
- **AC4 — Duration snaps to whole steps, ≥ 1 step.** A note's end also snaps to
  the grid; the snapped duration is `(end_step − onset_step)` steps and is
  always at least one step (a note never collapses to zero length).
- **AC5 — Order and count preserved.** Quantizing a sequence of valid notes
  yields the same number of notes, in onset order; a non-finite or non-positive
  input pitch is skipped (never panics), and empty input yields empty output.
- **AC6 — Pure & exact.** Quantization is deterministic and allocation-simple,
  reuses `gooz-ratio`'s exact snap/grid math, depends only inward
  (`gooz-dsp → gooz-ratio`), and library paths never panic.
- **AC7 — Documented API & gates.** Every public item is documented with a
  runnable example; the behaviour is covered by tests; all four toolchain gates
  are green.

## 4. Constraints & non-goals

- Operates on R-0005's note events + a caller-supplied `PitchGrid` and `Tempo`
  (+ subdivision); **no key detection, no tempo detection** (later features).
- Assumes the take's `t = 0` is the grid origin (the downbeat) — i.e. the user
  recorded against the metronome; phase/tempo alignment detection is later.
- **No rendering** — turning quantized notes into instrument audio is R-0007;
  the end-to-end record→riff pipeline is R-0008.
- Monophonic, offline (consumes a finished `Transcription`).
- Snapping is nearest-step/nearest-degree; tolerance controls and humanize/swing
  are Advanced Mode.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-18 | Fully grid-lock each note: snap pitch, onset, **and** duration (end snapped to the grid, duration = whole steps) | Owner choice — locks a loose hum to the grid so it reads as a played-in riff. |
| 2026-06-18 | The caller supplies the pitch grid root (and the tempo); R-0006 does not infer the key | Owner choice — deterministic and simple for v0; key/tempo detection is a later feature with its own heuristics and tests. |
| 2026-06-18 | Lives in `gooz-dsp` (re-adds the `gooz-ratio` dependency dropped in R-0005) | It consumes R-0005's note events (`gooz-dsp`) and `gooz-ratio`'s grids; `gooz-dsp → gooz-ratio` is the correct inward edge. |
| 2026-06-18 | A note with a non-finite/non-positive pitch is skipped, not an error | Such a note cannot occur from R-0005 (voiced notes are positive); skipping keeps the API total and panic-free without a spurious error path. |

## Changelog

- 2026-06-18 — created, accepted for M2.
