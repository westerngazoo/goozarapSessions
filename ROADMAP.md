# Roadmap

The single source of truth for what is being built and in what order — for the
project named in `project-specifics.md`. Milestones group requirements; each
requirement is realized by one or more specs. Nothing moves without passing the
requirement loop in [`CLAUDE.md`](CLAUDE.md) §4.

## Status legend

`Backlog` → `Discussing` → `Spec'd` → `In progress` → `In review` → `Done`

## Milestones

### M0 — Foundation  ·  *in progress*

Adopt the methodology and prepare the repository.

| Item | Status |
|------|--------|
| Methodology files in place (`CLAUDE.md`, `requirements/`, `specs/`, agents) | Done |
| `project-specifics.md` filled in | Done |
| Toolchain chosen and recorded (Rust workspace, gates green) | Done |
| Architecture written (`docs/ARCHITECTURE.md`) and workspace scaffolded | Done |
| First requirement discussed (R-0001) | Done |

### M1 — Ratio core & audio engine  ·  *complete*

The math foundation and a working engine: by the end of M1 the app can keep a
ratio-locked beat and record/play the user's voice.

| Req | Capability | Spec | Status |
|-----|------------|------|--------|
| R-0001 | Frequency-ratio core: interval arithmetic, harmonic-series grids, ratio-complexity ("smooth↔tense") ordering | SPEC-0001 | Done |
| R-0002 | Beat-ratio core: bar grids, Euclidean rhythms `E(k,n)`, polyrhythm composition, time quantization | SPEC-0002 | Done |
| R-0003 | Audio engine v0: device I/O, lock-free graph, record a take / play it back | SPEC-0003 | Done |
| R-0004 | Ratio-locked transport: metronome and click track driven by the beat grid | SPEC-0004 | Done |

### M2 — Easy Mode voice-to-riff

The signature loop: hum → distorted guitar riff, end to end.

| Req | Capability | Spec | Status |
|-----|------------|------|--------|
| R-0005 | Pitch tracking (YIN) + onset detection over a recorded take | SPEC-0005 | Done |
| R-0006 | Snap-to-grid: quantize tracked pitches/onsets onto frequency & beat ratio grids | SPEC-0006 | Done |
| R-0007 | Instrument render v0: Karplus-Strong guitar + distortion FX chain | SPEC-0007 | Backlog |
| R-0008 | Hum-to-riff pipeline: record → track → quantize → render → loopable stem | SPEC-0008 | Backlog |
| R-0009 | Beat builder: Euclidean drum templates with k/n sliders, synthesized kit | SPEC-0009 | Backlog |

### M3 — Sessions

A song is a real, savable, exportable thing.

| Req | Capability | Spec | Status |
|-----|------------|------|--------|
| R-0010 | Session format: song = stems + takes + arrangement + settings, save/load | SPEC-0010 | Backlog |
| R-0011 | Arrangement: sections as bar spans, loop regions, stem mute/level | SPEC-0011 | Backlog |
| R-0012 | Mixdown & export: WAV master + per-stem export | SPEC-0012 | Backlog |
| R-0013 | Studio shell v0: Tauri app wrapping M1–M3 (record button, sliders, timeline) | SPEC-0013 | Backlog |

### M4 — Influence models

The per-song creative brain, trained locally.

| Req | Capability | Spec | Status |
|-----|------------|------|--------|
| R-0014 | Model registry: per-song/per-album model dirs inside the session | SPEC-0014 | Backlog |
| R-0015 | Ingest & feature extraction: tempo/ratio profiles, timbre embeddings, structure from reference tracks | SPEC-0015 | Backlog |
| R-0016 | On-device training of adapters (DDSP timbre decoder first) with progress UI | SPEC-0016 | Backlog |
| R-0017 | Timbre transfer: render a hummed take through a trained timbre model | SPEC-0017 | Backlog |
| R-0018 | Model-biased beat builder: influence model conditions template & sound choices | SPEC-0018 | Backlog |

### M5 — Rap copilot

Freestyle support: beat, ears, and a rhyme brain.

| Req | Capability | Spec | Status |
|-----|------------|------|--------|
| R-0019 | Freestyle beat mode: one-tap looping beats from templates + influence model | SPEC-0019 | Backlog |
| R-0020 | Live transcription: on-device Whisper (candle) streaming over the beat | SPEC-0020 | Backlog |
| R-0021 | Rhyme engine: G2P + multi-syllabic rhyme search (EN + ES) | SPEC-0021 | Backlog |
| R-0022 | Semantic ranking: suggestions ordered by coherence with the verse so far | SPEC-0022 | Backlog |
| R-0023 | Song templating: structure coach (bar counts, hook cues, ending-word targets) | SPEC-0023 | Backlog |
| R-0024 | Lyric writing assist: influence-model LoRA continuation/suggestions | SPEC-0024 | Backlog |

### M6 — Advanced Mode

> Musician-facing depth: explicit note/MIDI editing, mixer depth, model knobs,
> tolerance controls on quantization. Requirements to be drafted once M2's
> Easy Mode loop has real-world feedback.

## Sequencing rules

- A requirement enters `Discussing` only when every requirement it depends on is
  `Done`.
- Requirement and spec ids are 4-digit and shared in spirit: `R-0001` is
  realized by `SPEC-0001` unless a requirement needs several specs.
- This file is updated by the orchestrator whenever a requirement changes state.

## Current focus

**M1 is complete and merged to `main`**, and **M2 is underway** — `R-0001`
(frequency ratios), `R-0002` (beat ratios), `R-0003` (audio engine v0), `R-0004`
(ratio-locked metronome), and `R-0005` (pitch tracking + onset detection) are
all implemented, QA-signed-off (PASS), architect-approved, and merged. The
ratio core (pitch + rhythm) lives in `gooz-ratio`; `gooz-audio` records a take,
plays it back, and runs a sample-accurate metronome driven by the beat grid;
`gooz-dsp` transcribes a recorded take into note events. The math drives the
sound, and the app can hear what you hummed.

M2 is half-built: `gooz-dsp` now hears a take (**R-0005**) and snaps it onto the
ratio grids (**R-0006** — pitch, onset, and duration all grid-locked). Next is
**R-0007 — instrument render v0**: turn quantized notes into audio with a
Karplus-Strong plucked string + a distortion FX chain (`gooz-synth`), the
*output* side of voice-to-instrument, en route to the full hum→riff pipeline
(R-0008).
