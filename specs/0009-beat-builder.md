# SPEC-0009 — Beat builder

- **Status:** Accepted
- **Realizes:** R-0009
- **Author:** Jules
- **Created:** 2026-06-28
- **Depends on:** SPEC-0002, SPEC-0007
- **Module(s):** `gooz-synth`

## 1. Motivation

This spec realizes R-0009: turning Euclidean rhythms into a playable drum beat. It defines a pure function that synthesizes a drum loop using three deterministic voices (kick, snare, hat), wraps the decay tails properly over loop boundaries, and returns both the audio stem and the realized patterns for UI/rendering use.

## 2. Design

`build_beat` lives in `gooz-synth` as it relies purely on sound synthesis and rhythm/time ratios.

We introduce three new types:
- `BeatVoice`: An enumeration of the available kit (`Kick`, `Snare`, `Hat`).
- `DrumVoiceConfig`: Represents a single voice configuration within a beat — its `BeatVoice`, and its Euclidean shape `(k, n, rotation)`.
- `BeatOutcome`: Represents the generated beat. Includes the `samples`, `sample_rate`, `bars` count, and a list of `gooz_ratio::Pattern`s (the resolved hit map).

Synthesis implementation details:
Each drum voice is rendered as an isolated function taking sample rate and returning a short `Vec<f32>` containing its sound. We use simple synthesis primitives:
- **Kick**: a sine wave starting at ~150Hz and dropping rapidly to ~50Hz, with an exponential amplitude decay.
- **Snare**: white noise passed through a crude bandpass filter, alongside a small sine "body" (around 200Hz), decaying rapidly.
- **Hat**: high-passed noise with a very short envelope.

The overall `build_beat` algorithm:
1. Validate the tempo. Calculate `bar_samples = tempo.bar_seconds() * sample_rate`. Handle edge cases like `bar_samples == 0`.
2. Generate the Euclidean `Pattern` for each `DrumVoiceConfig` using `Pattern::euclidean(k, n)`. Rotate it by the given `rotation`.
3. If all voices have zero onsets (`k == 0`), return an empty stem and `bars = 0`.
4. Otherwise, initialize a vector `stem` of size `bar_samples`. We assume a single bar output (`bars = 1`) to keep things simple for Euclidean loops.
5. For each voice, iterate over its step indices (`0..n`).
6. If the step is an onset, render the voice audio.
7. The hit offset is `(step as f64 / n as f64) * bar_samples` samples. Mix the voice buffer into `stem`.
8. *Looping & Wrapping*: if the hit's decay tail goes past the end of the `stem` (i.e. `offset + hit_len > bar_samples`), wrap it back to the beginning of `stem` so the loop is seamless.
9. Return the bounded (`[-1, 1]`) `stem`, alongside `bars = 1` and the generated patterns. Normalizing the entire mix guarantees bounds.

## 3. Code outline

```rust
// In crates/gooz-synth/src/beat.rs
use gooz_ratio::{Pattern, Tempo, BeatError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeatVoice { Kick, Snare, Hat }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrumVoiceConfig {
    pub voice: BeatVoice,
    pub k: u32,
    pub n: u32,
    pub rotation: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BeatOutcome {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub bars: u32,
    pub patterns: Vec<Pattern>,
}

pub fn build_beat(
    voices: &[DrumVoiceConfig],
    tempo: &Tempo,
    sample_rate: u32,
) -> Result<BeatOutcome, BeatError> {
    // 1. Calculate bar_samples
    // 2. Map voices to Pattern
    // 3. Render and mix
    // 4. Wrap tails
    // 5. Return outcome
}
```

## 4. Non-goals

- Adding more drum voices beyond Kick, Snare, and Hat.
- Dynamic lengths exceeding 1 bar.
- Sampler/sample-playback integration.
- Integrating UI or influence models.

## 5. Open questions

None.

## 6. Acceptance criteria

- [x] AC1: Correct hit placement via `Pattern::euclidean`.
- [x] AC2: Synthesizes Kick, Snare, and Hat deterministically.
- [x] AC3: `k`, `n`, and `rotation` modify the pattern count and placement correctly.
- [x] AC4: Seamless wrapping of decay tails across `bars * bar_samples` boundary.
- [x] AC5: Returns `BeatOutcome` exposing stem and `patterns`.
- [x] AC6: Deterministic rendering across runs.
- [x] AC7: Samples bounded to `[-1, 1]` without NaN/inf.
- [x] AC8: Docs and a functional demo.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-28 | Limit beat generation to 1 bar loops | Euclidean rhythms inherently define a repeating sequence mapped across a space. Assuming an `E(k, n)` loop represents 1 bar simplifies mixing and aligns perfectly with `gooz-ratio`. |

## Changelog

- _created_
