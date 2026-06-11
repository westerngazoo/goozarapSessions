# goozarapSessions — Architecture

A music-sessions studio in the spirit of Bizarrap's *Music Sessions*: one
artist, one producer-in-a-box, fast creative output. The producer-in-a-box is
this app. The user needs **zero music theory** — music is constructed from
**frequency ratios** and **beat ratios**, voice/mouth sounds become real
instrument parts, and a small **per-song influence model** trained locally
gives each session its own creative direction.

This document is the system design. Individual features still pass the
requirement loop (`CLAUDE.md` §4) before implementation.

## 1. Product pillars

1. **Ratio-first music math.** No scales, no note names, no chord symbols.
   - *Pitch*: intervals are small-integer frequency ratios from the harmonic
     series — 2:1 (octave), 3:2 (fifth), 5:4 (major third), … Consonance
     correlates with ratio simplicity, so the UI can expose a single
     "smooth ↔ tense" control that walks ratio complexity.
   - *Rhythm*: beats are ratios of a bar (1/4, 3/8, …), grooves are Euclidean
     distributions `E(k, n)` (k hits spread over n slots — `E(3,8)` is the
     tresillo behind half of latin/trap), and polyrhythm is just two ratios
     played against each other (3:2, 4:3).
   - Everything the user touches reduces to "pick a ratio," which the app
     renders musically. Math is the interface; sound is the output.

2. **Voice as the universal instrument.** Hum a melody, beatbox a groove, or
   make a "guitar mouth-noise" — the app tracks pitch and onsets, snaps them
   onto the ratio grid, and renders them as a real part: synthesized guitar
   riff with distortion, bass line, drum pattern. Easy Mode is built entirely
   on this loop: *make a sound → hear it back as an instrument → keep or redo*.

3. **Per-song influence models.** A musician writes new music by digesting
   influences. The app mirrors that: the user feeds reference tracks into a
   song/album project, the app extracts features (tempo & ratio profiles,
   timbre embeddings, section structure, lyrical style) and trains **small,
   local** adapters. Generation (beats, riff rendering, lyric suggestions) is
   conditioned on that song's model — so two songs with different influences
   sound and write differently. Models are project-scoped artifacts, trained
   and run on-device.

4. **Rap copilot.** Generate freestyle-able beats from ratio templates +
   influence model; transcribe the rap live (on-device Whisper); suggest
   ending words that (a) rhyme — phoneme-level, multi-syllabic — and (b)
   actually make sense — ranked for semantic coherence with the verse so far;
   coach structure with song templates (bar counts, verse/hook layout).

5. **Two modes.** *Easy Mode* ships first: voice input, ratio sliders,
   templates, one-tap render. *Advanced Mode* (later): musician-facing
   controls — explicit note/MIDI editing, mixer depth, model knobs.

## 2. System overview

```
┌──────────────────────────────────────────────────────────────────┐
│                        apps/gooz-studio                          │
│        Tauri shell — Easy Mode UI, session browser, meters       │
└───────┬──────────────┬──────────────┬──────────────┬─────────────┘
        │              │              │              │
┌───────▼──────┐ ┌─────▼─────┐ ┌──────▼─────┐ ┌──────▼──────┐
│ gooz-session │ │ gooz-model│ │ gooz-lyrics│ │  gooz-synth │
│ project fmt, │ │ influence │ │ rhyme +    │ │ instruments │
│ stems, takes,│ │ models:   │ │ flow engine│ │ K-S guitar, │
│ arrangement, │ │ train +   │ │ G2P, multi-│ │ FM/wavetable│
│ export       │ │ infer     │ │ syl rhyme, │ │ drums, FX   │
│              │ │ (candle)  │ │ templates  │ │ (distortion)│
└───────┬──────┘ └─────┬─────┘ └──────┬─────┘ └──────┬──────┘
        │              │              │              │
┌───────▼──────────────▼──────────────▼──────────────▼─────────────┐
│                          gooz-audio                              │
│   real-time engine: cpal I/O, lock-free graph, transport/clock   │
│            (ratio-locked metronome), record, playback            │
└───────┬──────────────────────────────────────────────┬───────────┘
        │                                              │
┌───────▼───────────────────┐          ┌───────────────▼───────────┐
│         gooz-dsp          │          │        gooz-ratio         │
│ FFT/STFT, pitch (YIN),    │          │ pure math core: frequency │
│ onset detect, filters,    │─────────▶│ ratios, harmonic series,  │
│ waveshaping, stretch      │          │ beat ratios, Euclidean    │
└───────────────────────────┘          │ rhythms, quantization     │
                                       └───────────────────────────┘
```

Dependencies point strictly inward/downward: the app depends on the feature
crates, feature crates depend on the engine, everything may depend on
`gooz-ratio`, and `gooz-ratio` depends on nothing. No cycles, ever.

## 3. Crate breakdown (Cargo workspace)

| Crate | Responsibility | Key dependencies (when implemented) |
|-------|----------------|--------------------------------------|
| `crates/gooz-ratio` | Pure music-math core. Frequency ratios & interval arithmetic, harmonic-series scale construction, beat-ratio grids, Euclidean rhythm generator, polyrhythm composition, snap-to-grid quantization. No I/O, no allocation surprises, fully unit-testable. | none — std only (hand-rolled exact rational; decided in R-0001) |
| `crates/gooz-dsp` | Custom DSP library (Rust-first, ours). STFT/FFT wrappers, YIN/pYIN pitch tracking, spectral-flux onset detection, envelope followers, biquad filters, waveshaping/distortion curves, time-stretch & pitch-shift (phase vocoder). Offline + block-based real-time variants. | `rustfft` |
| `crates/gooz-audio` | Real-time engine. Device I/O (`cpal`), lock-free audio graph (nodes: sampler, synth voice, FX, mixer, meter), transport with ratio-locked clock & metronome, recording into takes, latency compensation. Audio thread is allocation-free; control via SPSC message queues. | `cpal`, `ringbuf` |
| `crates/gooz-synth` | Instrument renderers — the *output* side of voice-to-instrument. Karplus-Strong plucked string (guitar/bass), FM & wavetable voices, drum synthesis (808-style kicks from sine+envelope ratios), sampler. FX chain: distortion (from `gooz-dsp` waveshapers), delay, convolution reverb. | `gooz-dsp` |
| `crates/gooz-session` | The project model. `Song` = stems + takes + arrangement (sections as bar-ratio spans) + ratio/tempo settings + reference to its influence model. Serialization (serde/RON or JSON), WAV/stem export, project directory layout. | `serde`, `hound` |
| `crates/gooz-model` | Local ML. Model registry (one model dir per song/album inside the project), feature-extraction pipeline (tempo & ratio profiles, timbre embeddings, section structure), training of small adapters (DDSP-style timbre decoder; LoRA on a small lyric LM), inference APIs: `timbre_transfer(audio, model)`, `suggest_beat(model)`, `continue_lyrics(model, context)`. On-device Whisper for transcription. | `candle-core`, `candle-transformers`, (`ort` fallback) |
| `crates/gooz-lyrics` | Rhyme & flow engine. Grapheme-to-phoneme (CMUdict + rules; Spanish phonemizer too — Bizarrap sessions are bilingual territory), multi-syllabic rhyme search (assonance/consonance scoring), semantic-coherence ranking via embeddings from `gooz-model`, song-structure templates (bars per section, ending-word targets), syllable/flow counting against the beat grid. | `gooz-model` |
| `apps/gooz-studio` | The desktop app. Tauri: Rust backend commands wrap the crates above; web UI for Easy Mode — big record button, ratio sliders ("smooth↔tense", "sparse↔busy"), instrument picker, session timeline, rap copilot view. | `tauri`, all crates |

## 4. Key pipelines

### 4.1 Voice-to-riff (Easy Mode core loop)

```
mic ──▶ gooz-audio.record(take)
take ──▶ gooz-dsp: YIN pitch track + onset detect ──▶ raw note events
events ──▶ gooz-ratio: snap pitches to harmonic ratio grid,
           snap onsets/durations to beat-ratio grid          ──▶ clean riff
riff ──▶ gooz-synth: render (e.g. Karplus-Strong guitar ▸ distortion ▸ reverb)
     └─▶ OR gooz-model: DDSP timbre transfer for an organic take
render ──▶ gooz-session: new stem on the timeline, loopable
```

Latency budget: tracking + quantize + synth render of a 4-bar take well under
1 s on a laptop; live monitoring of the raw voice is immediate via the engine.

### 4.2 Beat builder

```
user picks tempo (or taps it) + groove template (Euclidean E(k,n) per drum voice)
gooz-ratio expands templates into a bar grid
gooz-synth renders kick/snare/hat voices (or sampler)
influence model (if trained) biases template choice + sound design
└─▶ looping beat on the timeline, each voice's k/n exposed as two sliders
```

### 4.3 Influence model lifecycle

```
ingest: user drops reference tracks into the song project
extract (gooz-model): tempo & beat-ratio profile, ratio/harmony histogram,
        timbre embeddings per instrument-ish band, section structure,
        (if lyrics provided/transcribed) lyrical style features
train:  small adapters, on-device — DDSP timbre decoder(s), LoRA adapter on
        a small local lyric LM, style-conditioning vectors for beat builder
store:  model artifacts live inside the session dir → portable, per-song
use:    beat suggestion, riff rendering bias, timbre transfer, lyric continuation
```

Constraints: training jobs are background tasks with progress UI; never on the
audio thread; artifacts small enough to commit to a session folder (tens of
MB, not GB). Reference audio is used locally only.

### 4.4 Rap copilot

```
beat loop plays (4.2) ──▶ user raps over it
gooz-model: streaming Whisper transcription (on-device)
gooz-lyrics: phonemize the last bar(s) ──▶ multi-syllabic rhyme candidates
gooz-model: embed verse context ──▶ rank candidates by semantic coherence
UI: shows 3–5 ending-word/phrase suggestions a beat ahead; song template
    tracks bar counts and flags "hook coming in 2 bars"
```

## 5. ML strategy

- **Framework: candle** (Rust-native, HuggingFace). Keeps the Rust-first rule
  without FFI into Python. `ort`/ONNX Runtime is the sanctioned fallback for a
  model that candle can't run yet — behind the same `gooz-model` API so
  callers never know.
- **Small and local beats big and remote.** Per-song models are *adapters*,
  not foundation models: DDSP-style decoders for timbre (millions, not
  billions, of params — strong for instrument timbre transfer), LoRA adapters
  on a small (≤3B, quantized) lyric LM, conditioning vectors for beats. A
  laptop/Apple-silicon machine must train them in minutes-to-an-hour.
- **Whisper (candle) for all speech-to-text** — rap transcription, voice memos.
- **Honesty rule:** generation features degrade gracefully — with no influence
  model trained, every pipeline still works from neutral defaults; the model
  only *biases* the math, it never replaces it. This keeps the core app
  deterministic and testable.

## 6. Real-time audio rules (engine discipline)

- The audio callback never allocates, locks, or does file/ML I/O.
- Control plane → audio thread via lock-free SPSC queues; audio thread →
  UI via atomic meters and a ring buffer for waveforms.
- All DSP is block-based (`&[f32]` in/out, power-of-two blocks) so the same
  code serves offline rendering and the live engine.
- Sample rate is engine-global per session; resampling happens at the edges
  (import/export), never mid-graph.

## 7. Testing strategy (per `CLAUDE.md` §5)

- `gooz-ratio` and `gooz-dsp` are pure → exhaustive unit tests, property
  tests where the math invites them (e.g. `E(k,n)` always places exactly k
  hits; quantization is idempotent).
- DSP correctness via golden signals: known sine → YIN must report f₀ within
  tolerance; known click train → onset detector finds every click.
- Engine tested with a virtual (non-device) backend so CI needs no sound card.
- ML pipelines tested at the API seam with tiny fixture models; training
  smoke-tested on a 10-second fixture corpus.
- Each requirement gets e2e coverage (qa agent owns it), e.g. R-hum-to-riff:
  fixture hum WAV in → rendered guitar stem out, note events match golden.

## 8. Risks & deliberate choices

| Risk / choice | Position |
|---------------|----------|
| Pitch tracking on untrained voices is messy | Quantization to the ratio grid is the product answer — snap aggressively in Easy Mode, expose tolerance in Advanced Mode. |
| On-device training time | Adapters only; if a corpus is too big, sample it. Cloud training is explicitly out of scope for now. |
| candle gaps vs PyTorch | `ort` fallback behind `gooz-model`'s API; worst case a model ships as ONNX. |
| Tauri (web UI) vs pure-Rust UI (egui) | Tauri chosen: creative UIs need rich, fast-iterating visuals; the Rust-first rule applies to everything below the UI. Revisit if the IPC boundary hurts. |
| Copyright of reference tracks | Influence models are local-only, trained on audio the user provides, never redistributed by the app. Sessions export only the user's own renders. |

## 9. Repository layout

```
goozarapSessions/
├── CLAUDE.md  project-specifics.md  ROADMAP.md     methodology
├── requirements/   specs/                          the loop's paper trail
├── docs/ARCHITECTURE.md                            this file
├── Cargo.toml                                      workspace root
├── crates/
│   ├── gooz-ratio/    gooz-dsp/    gooz-audio/    gooz-synth/
│   ├── gooz-session/  gooz-model/  gooz-lyrics/
└── apps/
    └── gooz-studio/
```

The workspace is scaffolded with empty, documented crates so the structure is
real and `cargo build/test/clippy/fmt` run green from day one. Per the
constitution, **no feature code lands in any crate until its requirement and
spec are accepted** — the crates' doc comments state their bounded
responsibility and nothing more.
