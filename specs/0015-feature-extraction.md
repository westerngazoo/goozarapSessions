# SPEC-0015 — Ingest & feature extraction

- **Status:** Implemented
- **Realizes:** R-0015
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-02
- **Depends on:** SPEC-0014 (registry), SPEC-0005 (analyze), SPEC-0001 (grid/snap)
- **Module(s):** `crates/gooz-model`

## 1. Motivation

Realizes R-0015: reduce reference audio to a compact, inspectable, ratio-native
`FeatureProfile` and persist it into the model directory — the ingest half of M4,
candle-free.

## 2. Design

```
crates/gooz-model/src/
├── error.rs     + ModelError::Extract(String)
├── features.rs  RatioWeight, FeatureProfile, extract_features + pure helpers
├── registry.rs  + write_features / read_features
└── lib.rs       re-exports
```

Dependencies gain `gooz-dsp` + `gooz-ratio` (analysis + ratio math). Still no
candle.

### Types (features.rs)

```rust
pub const FEATURE_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatioWeight { pub num: u64, pub den: u64, pub weight: f64 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureProfile {
    pub format_version: u32,
    pub sample_rate: u32,
    pub duration_secs: f64,
    pub tempo_bpm: f64,      // 0.0 when < 2 onsets
    pub onset_density: f64,  // onsets per second
    pub rms: f64,            // loudness proxy
    pub brightness: f64,     // zero-crossing rate proxy in [0, 1]
    pub ratios: Vec<RatioWeight>, // grid-degree histogram, weights sum ~1
}
```

### `extract_features` (features.rs)

```rust
pub fn extract_features(
    samples: &[f32],
    sample_rate: u32,
    grid: &gooz_ratio::PitchGrid,
    cfg: &gooz_dsp::Config,
) -> Result<FeatureProfile, ModelError>;
```

1. `t = gooz_dsp::analyze(samples, sample_rate, cfg)` — errors mapped to
   `ModelError::Extract` (empty / zero-rate / non-finite / window-too-large).
2. `duration_secs = samples.len() / sample_rate`.
3. `rms = sqrt(mean(x^2))`; `brightness = zero_crossing_rate(samples)`.
4. Rhythm from `t.onsets`: `onset_density = onsets.len() / duration`;
   `tempo_bpm = estimate_bpm(onset_times)`.
5. Harmony from `t.notes`: snap each `pitch_hz` to `grid`, accumulate
   `duration_secs` into a `(num, den)` bucket, normalize weights to sum 1,
   sort by `(num, den)` for determinism.

Pure helpers (unit-tested directly): `rms`, `zero_crossing_rate`,
`estimate_bpm(&[f64]) -> f64` (median IOI → bpm, `0.0` for < 2 onsets),
`ratio_histogram(notes, grid) -> Vec<RatioWeight>`.

### Registry integration (registry.rs)

```rust
impl ModelRegistry {
    pub fn write_features(&self, id: &str, profile: &FeatureProfile) -> Result<(), ModelError>;
    pub fn read_features(&self, id: &str) -> Result<FeatureProfile, ModelError>;
}
```

`write_features` writes `features.json` into the model dir (pretty JSON) and
records it via `manifest_add_file`; `read_features` reads it back (`NotFound` if
absent). File name constant `FEATURES = "features.json"`.

## 3. Non-goals

Training (R-0016), neural timbre embeddings, polyphonic separation, real
beat-tracking, storing reference audio.

## 4. Acceptance criteria

Maps to R-0015 AC1–AC7; qa owns `crates/gooz-model/tests/acceptance_r0015.rs`.

- [x] AC1 — `extract_features` returns a profile for valid audio
- [x] AC2 — duration/rms/brightness finite
- [x] AC3 — tempo_bpm (0 for < 2 onsets) + onset_density
- [x] AC4 — ratio histogram weights ≥ 0, sum ~1 (empty when no pitch)
- [x] AC5 — `write_features`/`read_features` round-trip via the registry
- [x] AC6 — typed error on bad input; deterministic; no panic
- [x] AC7 — docs + four gates green

## 5. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | Weights are **duration-weighted**, normalized to sum 1 | A distribution over grid degrees is scale-free and directly usable as a bias later. |
| 2026-07-02 | `estimate_bpm` uses the **median** IOI | Robust to a few spurious onsets vs the mean. |
| 2026-07-02 | Analysis errors map to a single `ModelError::Extract` | The caller only needs "extraction failed + why"; the DSP error detail is preserved in the message. |

## Changelog

- 2026-07-02 — created; accepted alongside R-0015.
