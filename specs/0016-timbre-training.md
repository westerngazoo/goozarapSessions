# SPEC-0016 — On-device training (DDSP timbre decoder)

- **Status:** Implemented
- **Realizes:** R-0016
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-03
- **Depends on:** SPEC-0014 (registry), SPEC-0015 (features), SPEC-0005 (analyze)
- **Module(s):** `crates/gooz-model`

## 1. Motivation

Realizes R-0016: the first on-device learned adapter — a small candle timbre
decoder trained locally from a reference's harmonic profile, with progress and
save/load into the registry. Proves the training seam for all later adapters.

## 2. Design

```
crates/gooz-model/src/
├── error.rs     + ModelError::Train(String)
├── timbre.rs    harmonic target, TimbreDecoder, TrainConfig, TrainProgress, train_timbre
├── registry.rs  + save_timbre / load_timbre
└── lib.rs       re-exports
```

Dependencies gain `candle-core` + `candle-nn` (CPU).

### Harmonic target (timbre.rs)

```rust
pub fn extract_timbre_target(samples: &[f32], sample_rate: u32, n_harmonics: usize) -> Vec<f32>;
```

- Find the reference's dominant pitch `f0` (median of `analyze`'s voiced note
  pitches; if none → return a uniform vector of length `n_harmonics`).
- For `k in 1..=n_harmonics`, measure the magnitude at `k·f0` with the **Goertzel**
  algorithm over the signal.
- Normalize the magnitudes to sum 1 (uniform if all zero).

### Decoder + training (timbre.rs)

```rust
pub struct TrainConfig { pub n_harmonics: usize, pub hidden: usize, pub epochs: usize, pub lr: f64, pub seed: u64 }
// Default: { n_harmonics: 8, hidden: 16, epochs: 200, lr: 0.05, seed: 0xDDSP }

pub struct TrainProgress { pub epoch: usize, pub loss: f32 }

pub struct TimbreDecoder { /* VarMap + a 1→hidden→n_harmonics MLP */ }

pub fn train_timbre(
    target: &[f32],
    cfg: &TrainConfig,
    progress: &mut dyn FnMut(TrainProgress),
) -> Result<(TimbreDecoder, Vec<TrainProgress>), ModelError>;
```

- CPU device, `device.set_seed(cfg.seed)` before building the `VarMap` so the
  linear-layer init is deterministic.
- Model: `linear(1, hidden)` → ReLU → `linear(hidden, n_harmonics)` →
  `softmax_last_dim` (a valid distribution).
- Input is a constant control `x = [[1.0]]`; target is the normalized vector.
- Loss = MSE(pred, target); optimizer = AdamW(lr). Each epoch: forward,
  `backward_step`, record `TrainProgress` (via callback + history).
- `TimbreDecoder::harmonics() -> Vec<f32>` runs the forward pass at `x = 1.0`.

Candle errors map to `ModelError::Train`.

### Registry integration (registry.rs)

```rust
impl ModelRegistry {
    pub fn save_timbre(&self, id: &str, decoder: &TimbreDecoder) -> Result<(), ModelError>;
    pub fn load_timbre(&self, id: &str, cfg: &TrainConfig) -> Result<TimbreDecoder, ModelError>;
}
```

`save_timbre` writes `timbre.safetensors` (candle `VarMap::save`) into the model
dir and records it via `manifest_add_file`. `load_timbre` rebuilds the MLP for
`cfg` then `VarMap::load`s the weights (`NotFound` if absent). File constant
`TIMBRE = "timbre.safetensors"`.

## 3. Non-goals

Timbre transfer/rendering (R-0017), time-varying DDSP control curves + noise
branch, GPU/`ort`, LoRA (R-0024), a training UI.

## 4. Acceptance criteria

Maps to R-0016 AC1–AC8; qa owns `crates/gooz-model/tests/acceptance_r0016.rs`.

- [x] AC1 — `extract_timbre_target` → normalized harmonics (uniform when no pitch)
- [x] AC2 — `train_timbre` trains on CPU for `epochs`
- [x] AC3 — per-epoch progress via callback + history; loss decreases
- [x] AC4 — trained harmonics approximate the target on a fixture
- [x] AC5 — `save_timbre`/`load_timbre` reproduce the harmonics
- [x] AC6 — deterministic with a fixed seed
- [x] AC7 — typed `ModelError`, no panic
- [x] AC8 — docs + four gates green

## 5. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-03 | Constant control input (`x = 1.0`) in v0 | The decoder learns a static timbre; time-varying control (loudness/f0 curves) is a later DDSP refinement. |
| 2026-07-03 | Goertzel (not a full FFT) for harmonic magnitudes | Only a handful of exact harmonic bins are needed; Goertzel is allocation-light and dependency-free. |
| 2026-07-03 | Softmax output | Guarantees the decoder emits a valid, comparable distribution against the normalized target. |

## Changelog

- 2026-07-03 — created; accepted alongside R-0016.
