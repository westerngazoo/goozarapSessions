# R-0005 — Pitch tracking & onset detection (note transcription)

- **Status:** Accepted
- **Milestone:** M2
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-06-18
- **Depends on:** R-0003 (provides recorded takes the pipeline feeds in; analysis itself is independent)
- **Realized by:** SPEC-0005
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must be able to analyze a recorded monophonic take (a hum, a sung
melody, a beatboxed line) and transcribe it into **note events** — each a start
time, a pitch in Hz, and a duration. It does this by tracking pitch over the
signal with the YIN algorithm and detecting note starts with spectral-flux
onset detection, then segmenting the pitch track at the onsets to assemble the
note events. The pitch track (f0 + confidence per frame) and the raw onsets are
also exposed as intermediate results. This is the first stage of the Easy Mode
hum→riff loop: it turns sound into structured musical material that R-0006 then
snaps onto the ratio grid.

Analysis is **offline** (over a complete recorded buffer); it operates on raw
samples so it sits below the audio engine. Real-time/streaming analysis,
polyphony, and grid quantization are out of scope.

## 2. Rationale

"Voice is the universal instrument" (architecture §1.2): the user makes a sound
and the app turns it into a part. That is impossible without first hearing
*what* they sang — the pitches and where each note begins. YIN is the standard,
robust monophonic pitch tracker (no ML, deterministic); spectral flux is the
standard onset detector. Producing note events here — rather than leaving raw
analyses — gives R-0006 (snap) and R-0008 (the full pipeline) a clean, musical
unit to work with. It must be exact enough that a hum lands on the pitch the
user intended once quantized, and deterministic so it is testable on golden
signals with no microphone.

## 3. Acceptance criteria

- **AC1 — Pitch accuracy (YIN).** For a synthesized steady tone at a known f0 in
  the voice range (e.g. 220, 330, 440 Hz), the detected pitch is within **±1 %**
  (≈17 cents) of the true f0 across the voiced frames.
- **AC2 — Voiced/unvoiced.** Silence and broadband noise are reported as
  *unvoiced* (no f0); a clear tone is reported as *voiced*. Unvoiced frames
  never contribute a pitch to a note event.
- **AC3 — Onset detection.** For a signal of `K` separated bursts (tone or
  click followed by a gap), exactly `K` onsets are detected, each within a small
  tolerance (≈±20 ms) of the true start; a single steady tone produces exactly
  one onset (its start), not a stream of spurious ones.
- **AC4 — Note-event assembly.** A two-tone signal — pitch A then a gap then
  pitch B — transcribes to exactly two note events, in time order, each with the
  correct pitch (within ±1 %), an onset time within ≈±20 ms, and a positive
  duration that does not overlap the next note.
- **AC5 — Range & segmentation.** Pitch detection is confined to a configurable
  `[f_min, f_max]` (default voice range); note events are assembled only from
  voiced segments (a segment with no voiced frames yields no note); events are
  sorted by onset and non-overlapping.
- **AC6 — Typed errors, no panic.** Empty input, a zero/invalid sample rate, or
  a configuration that cannot analyze the input (e.g. analysis window longer
  than the signal) are reported via a typed `DspError`; library paths never
  panic; non-finite input samples do not cause a panic.
- **AC7 — Documented API & gates.** Every public item is documented with a
  runnable example; the analysis is covered by golden-signal tests (no
  microphone); all four toolchain gates are green.

## 4. Constraints & non-goals

- Pure analysis over `&[f32]` + sample rate; **no device I/O** and no dependency
  on `gooz-audio` (dependencies point inward; `gooz-dsp` is below the engine).
- **Offline only.** A streaming/real-time block variant (for live monitoring)
  is a later requirement.
- **Monophonic** voice/melody only; polyphonic transcription is out of scope.
- **No grid quantization** — mapping pitches/onsets onto frequency & beat ratio
  grids is **R-0006**; the rendering of notes into an instrument is R-0007.
- Probabilistic/HMM-smoothed pitch (pYIN) and alternative trackers (SWIPE) are
  possible later refinements; v0 is plain YIN.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-18 | Offline analysis over a complete buffer (no streaming variant yet) | Owner choice — it is exactly what the M2 hum→riff loop needs; streaming pays off only when live monitoring lands. |
| 2026-06-18 | R-0005 assembles note events (onset, pitch, duration), exposing the pitch track + onsets as intermediate results | Owner choice — gives the pipeline a ready musical unit; the raw analyses come for free and aid testing/visualization. |
| 2026-06-18 | YIN for pitch, spectral flux for onsets | The standard, deterministic, no-ML choices for monophonic pitch and onset; reproducible on golden signals. |
| 2026-06-18 | Representative pitch per note = median of its voiced f0 frames | Robust to transient octave errors / glides at note edges; trivially testable. |

## Changelog

- 2026-06-18 — created, accepted for M2.
