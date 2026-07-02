# AI music director — describe → music (local-first)

> Research & design direction for a new capability: the user **describes** what
> they want — "a dark trap beat around 140, spooky", "smooth lofi melody, warm",
> "make my voice sound like a broken robot" — and the app produces it. Local,
> on-device, ratio-native. This document seeds milestone **M7** in
> [`ROADMAP.md`](../ROADMAP.md); each requirement still passes the loop
> (`CLAUDE.md` §4) before implementation.

## 1. Principle: the model *biases* the math, it never replaces it

The whole project is ratio-first and deterministic: pitch is small-integer
frequency ratios, rhythm is Euclidean beat ratios, sound is our synths. The AI
layer does **not** generate raw audio from a prompt. It **interprets a
description into musical intent**, and that intent drives the *existing* engine
(ratios + Euclidean beats + synth), optionally coloured by the per-song
influence model (M4).

This keeps the **honesty rule** (ARCHITECTURE §5): with no model and no
description, every pipeline still works from neutral defaults; the description
only *steers* the math. It also keeps generation **on-grid** (musical by
construction) and **small enough to run locally** — the two things a raw
text-to-audio model (MusicGen-scale) gives up.

- **Local-first (owner decision, 2026-07-02):** no cloud. Small/quantized models
  via `candle`, running on the user's machine. An `ort`/ONNX backend behind the
  same `gooz-model` API is the only sanctioned fallback (ARCHITECTURE §5), never
  the default. Reference audio and prompts never leave the device.

## 2. The pipeline

```
"dark trap, ~140, spooky, 808 glide"                 (typed; later: spoken)
        │
        ▼   R-0025  describe → intent  (gooz-model: small local LM / embeddings)
MusicalIntent { tempo, feel(smooth↔tense), genre, structure, energy,
                density, timbre words, mood, key-feel }
        │
        ▼   R-0026  intent → engine params  (preset library, ratio-native)
{ PitchGrid + ratio walk, per-voice E(k,n) beat, synth/FX settings }
        │
        ▼   R-0027  generate  (existing engine + influence model M4)
melody (gooz-ratio + gooz-synth)  +  beat (R-0009)  →  loopable stems
        │
        ▼   R-0028  voice transform  (optional, on a recorded take)
formant/pitch + waveshaping + DDSP timbre transfer  →  "voz distortion"
```

Everything below R-0025 already exists or is planned (ratio core R-0001, beat
builder R-0009, synth R-0007, timbre transfer R-0017). M7 is mostly the
**front half**: turning words into intent, and intent into parameters.

## 3. `MusicalIntent` — the seam

A small, serializable, **inspectable** struct is the contract between the model
and the engine. It is human-readable and editable (the UI can show and tweak it
— no black box), and it degrades: every field has a neutral default.

```
MusicalIntent {
    tempo_bpm: range or point,        // "around 140" → 132..148
    feel: f32,                        // smooth↔tense (ratio complexity target)
    genre: Genre,                     // Trap | Lofi | House | Drill | Free | …
    structure: Vec<Section>,          // intro/verse/hook bar spans
    energy: f32, density: f32,        // drives E(k,n) k/n, velocity, layering
    timbre: Vec<String>,              // "808", "warm", "glassy", "distorted"
    mood: Vec<String>,                // "dark", "spooky", "uplifting"
    key_feel: KeyFeel,                // bright/neutral/dark ratio bias, no note names
}
```

## 4. Genre presets (trap, and beyond)

Presets are **data**, not model weights: a mapping from `genre` + `energy` →
concrete ratio/beat/synth parameters. This makes them reviewable, testable, and
extendable without retraining. Trap, concretely:

- **Tempo feel:** ~130–150 with a half-time backbeat (snare on 3).
- **Hats:** Euclidean rolls — `E(k,16)` with high `k`, plus occasional triplet
  rolls; density from `energy`.
- **Kick + 808:** sparse Euclidean kick, sustained 808 with a pitch glide
  (reuses R-0009's kick voice + a glide envelope).
- **Melody:** sparse, dark ratio walk (minor-third `6:5`, tritone-ish tension
  ratios) over a drone; `feel` picks how tense the ratios go.

"General music" is just the `Free` genre = neutral defaults + the description's
`feel`/`energy` with no strong template.

## 5. Model choices (all local)

| Job | Local approach | Notes |
|-----|----------------|-------|
| Describe → `MusicalIntent` (R-0025) | Small quantized instruct LM (≤3B, candle) doing constrained JSON slot-filling; **or** sentence-embeddings + nearest-preset + keyword slots for a no-LM fallback | Constrained decoding keeps output valid; the embedding path runs on anything |
| Intent → params (R-0026) | Deterministic preset library (pure Rust) | No model; fully testable |
| Generation bias (R-0027) | Influence-model adapters (M4): DDSP timbre, conditioning vectors, beat-choice bias | Degrades to presets with no trained model |
| Voice transform (R-0028) | DSP (formant/pitch shift, waveshaping) + DDSP timbre transfer (R-0017) | "voz distortion"; DSP path needs no model |

Deliberately **not** in scope: raw prompt→waveform generation (MusicGen/AudioLDM
scale). It is large, cloud-shaped, and off-grid — against the project's local +
ratio-first ethos. Revisit only behind the `ort` fallback if ever justified.

## 6. Geometric-algebra ML tie-in

The intent→bias and timbre models are exactly where the
[geometric-algebra / Clifford ML thread](research-directions.md) applies: small,
sample-efficient adapters over the per-song data, with the caveat noted there
(audio symmetries are multiplicative/cyclic, so operate in log-frequency / phase
domains). M7's generation-bias models are candidate hosts for that experiment.

## 7. Testing & honesty

- The **preset library (R-0026)** and **generation (R-0027)** are deterministic
  and unit-tested from a fixed `MusicalIntent` — no model needed in CI.
- The **parser (R-0025)** is tested at the API seam with fixture descriptions →
  expected intent slots (tolerant matching), plus a tiny fixture model.
- With **no model and no description**, the app behaves exactly as today. The AI
  layer is additive; it never gates the core loop.

## 8. Landmarks

- **DDSP** (Engel et al.) — differentiable synthesis; the timbre-transfer basis.
- **RAVE** — real-time neural audio, a local-generation reference.
- **candle** (HuggingFace) — the Rust-native runtime (ARCHITECTURE §5).
- **MusicGen / AudioLDM** — the *contrast*: what we deliberately avoid running
  locally; noted for the `ort` fallback discussion only.
