# R-0008 — Hum-to-riff pipeline

- **Status:** Accepted
- **Milestone:** M2
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-06-21
- **Depends on:** R-0005 (transcribe), R-0006 (quantize), R-0007 (render); R-0003 (record, for the demo)
- **Realized by:** SPEC-0008
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must wire the analysis, quantization, and synthesis stages into one
pipeline: given a recorded monophonic take (samples + sample rate) and a musical
context (a frequency grid and a tempo + subdivision), it transcribes the take
(R-0005), snaps the notes onto the grids (R-0006), renders them as a guitar
(R-0007), and assembles a **loopable stem** — a rendered buffer whose length is a
whole number of bars so it repeats cleanly on the beat. The pipeline returns the
stem **plus what it heard** (the raw transcription and the grid-locked notes), so
the app can show "you hummed → these notes." A by-ear demo records a few seconds
of humming and plays the resulting riff in a loop. This is the first time the
product premise — *hum something, get a distorted guitar riff* — is fully
playable end to end.

It is the integration layer: it lives where the engine, the DSP, and the synth
all meet. It is not the UI (R-0013) and does not save the stem (R-0010); it
turns a recording into a looping riff.

## 2. Rationale

Every piece of the hum→riff chain already exists and is tested in isolation
(R-0005 hears, R-0006 snaps, R-0007 renders). Until they are wired together the
product does not actually *do* its core thing. This requirement is the payoff:
it makes the premise real and gives the owner something to play with. Returning
the intermediate results (not just audio) is what lets the dark-neon UI later
display the detected pitches and the snapped grid — and lets this stage be
tested precisely (assert the notes, not just "non-silent"). A bar-aligned stem
is what "loop" means musically: a riff you can lay down and build on.

## 3. Acceptance criteria

- **AC1 — End-to-end transform.** Given a synthesized monophonic "hum" (tones at
  grid pitches with gaps), the pipeline returns a non-empty, bounded stem and a
  set of grid-locked notes whose pitches match the hummed tones (snapped to the
  supplied grid) — i.e. analysis → quantize → render compose correctly.
- **AC2 — Loopable (bar-aligned) stem.** The stem's length is exactly a whole
  number of bars (`bars · bar_samples`, `bar_samples` derived from the tempo and
  sample rate); `bars ≥ 1` for a non-empty riff; the length is `≥` the raw
  rendered length (padding only — the note tails are preserved, the pad is
  silence), so looping repeats on the downbeat.
- **AC3 — Returns what it heard.** The result exposes both the raw transcription
  (pitch track + onsets) and the grid-locked `QuantizedNote`s, in addition to the
  stem; the notes' count and pitches reflect the hum.
- **AC4 — Typed errors / input guard.** Empty samples, a zero sample rate, or
  non-finite samples are reported as a typed error (propagated from analysis);
  the pipeline never panics on bad input. (This is the onset/finiteness guard
  deferred from R-0007.)
- **AC5 — Deterministic.** The same input (samples + context + config) produces
  an identical result — identical stem samples and identical notes.
- **AC6 — Bounded, clean audio.** The stem samples are within `[-1, 1]` and
  contain no NaN/inf (inherited from the renderer).
- **AC7 — Demo, docs, gates.** A `cargo run` demo records ~4 s of humming and
  plays back the looped riff on a real machine (verified by ear; not a CI gate).
  Every public item is documented; the pure pipeline is covered by tests; all
  four toolchain gates are green.

## 4. Constraints & non-goals

- The pure pipeline (`samples → riff`) is offline, deterministic, and depends on
  `gooz-dsp` + `gooz-synth`; the demo additionally uses `gooz-audio` to record
  and play. It lives in the integration crate (`gooz-studio`), the only place
  that depends on the engine and the synth.
- Caller supplies the **grid (root) and tempo** — no key or tempo detection
  (later features), consistent with R-0006.
- **No UI** (R-0013), **no session/stem persistence** (R-0010), no arrangement.
- One instrument (the R-0007 guitar); the **beat builder** is R-0009.
- Offline (consumes a finished recording); a live/streaming pipeline is later.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-21 | The stem is **bar-aligned** (length padded to a whole number of bars) | Owner choice — that is what a loop is musically; it repeats cleanly on the downbeat. |
| 2026-06-21 | The pipeline returns the stem **plus** the transcription and the quantized notes | Owner choice — feeds the UI's "here's what I heard" and makes the stage precisely testable; the intermediate data is nearly free. |
| 2026-06-21 | Lives in `gooz-studio` (made lib + bin): pure pipeline in the lib, device demo in the bin | It is the integration layer (the only crate depending on `gooz-audio` + `gooz-synth`); the pure pipeline stays CI-testable, the demo is by-ear. |
| 2026-06-21 | `hum_to_riff` returns `Result` (propagating the analysis error) | Honours the rich success shape while surfacing real input failures (empty/zero-rate/non-finite) — the deferred R-0007 input guard. |

## Changelog

- 2026-06-21 — created, accepted for M2.
