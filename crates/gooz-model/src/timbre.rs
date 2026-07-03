//! On-device DDSP-style timbre decoder: extract a harmonic target and train a
//! small candle model to reproduce it.

use std::f64::consts::TAU;
use std::path::Path;

use candle_core::{D, Device, Tensor, Var};
use candle_nn::{AdamW, Linear, Module, Optimizer, ParamsAdamW, VarMap};

use gooz_dsp::{Config, analyze};

use crate::error::ModelError;

/// Training hyper-parameters for the timbre decoder.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrainConfig {
    /// Number of harmonics the decoder models.
    pub n_harmonics: usize,
    /// Hidden-layer width.
    pub hidden: usize,
    /// Training epochs.
    pub epochs: usize,
    /// AdamW learning rate.
    pub lr: f64,
    /// RNG seed for reproducible initialization.
    pub seed: u64,
}

impl Default for TrainConfig {
    fn default() -> TrainConfig {
        TrainConfig {
            n_harmonics: 8,
            hidden: 16,
            epochs: 200,
            lr: 0.05,
            seed: 0xDD5F,
        }
    }
}

/// One training step's outcome.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrainProgress {
    /// Zero-based epoch index.
    pub epoch: usize,
    /// Mean-squared error at this epoch.
    pub loss: f32,
}

/// Measures the magnitude of frequency `freq_hz` in `samples` via the Goertzel
/// algorithm (a single-bin DFT). Returns `0.0` for degenerate input.
fn goertzel(samples: &[f32], sample_rate: u32, freq_hz: f64) -> f64 {
    if samples.is_empty() || sample_rate == 0 || freq_hz <= 0.0 {
        return 0.0;
    }
    let omega = TAU * freq_hz / f64::from(sample_rate);
    let coeff = 2.0 * omega.cos();
    let (mut s_prev, mut s_prev2) = (0.0f64, 0.0f64);
    for &x in samples {
        let s = f64::from(x) + coeff * s_prev - s_prev2;
        s_prev2 = s_prev;
        s_prev = s;
    }
    let power = s_prev2 * s_prev2 + s_prev * s_prev - coeff * s_prev * s_prev2;
    power.max(0.0).sqrt() / samples.len() as f64
}

/// A uniform distribution of length `n` (`n == 0` → empty).
fn uniform(n: usize) -> Vec<f32> {
    if n == 0 {
        return Vec::new();
    }
    vec![1.0 / n as f32; n]
}

/// Normalizes a non-negative vector to sum 1, or returns uniform if it sums to 0.
fn normalize(mut v: Vec<f32>) -> Vec<f32> {
    let sum: f32 = v.iter().sum();
    if sum <= 0.0 {
        return uniform(v.len());
    }
    for x in &mut v {
        *x /= sum;
    }
    v
}

/// Extracts a normalized harmonic-amplitude target (the timbre) from a reference
/// recording: the magnitudes at `f0, 2·f0, …, n·f0` for the reference's dominant
/// pitch. A recording with no detectable pitch yields a uniform target.
pub fn extract_timbre_target(samples: &[f32], sample_rate: u32, n_harmonics: usize) -> Vec<f32> {
    if n_harmonics == 0 {
        return Vec::new();
    }
    let Ok(t) = analyze(samples, sample_rate, &Config::default()) else {
        return uniform(n_harmonics);
    };
    let mut pitches: Vec<f64> = t
        .notes
        .iter()
        .map(|n| f64::from(n.pitch_hz))
        .filter(|p| p.is_finite() && *p > 0.0)
        .collect();
    if pitches.is_empty() {
        return uniform(n_harmonics);
    }
    pitches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let f0 = pitches[pitches.len() / 2];

    let mags: Vec<f32> = (1..=n_harmonics)
        .map(|k| goertzel(samples, sample_rate, f0 * k as f64) as f32)
        .collect();
    normalize(mags)
}

/// A small MLP timbre decoder: a constant control maps through `1 → hidden →
/// n_harmonics` and a softmax to a harmonic-amplitude distribution.
pub struct TimbreDecoder {
    varmap: VarMap,
    l1: Linear,
    l2: Linear,
    device: Device,
}

impl TimbreDecoder {
    /// Builds an (untrained) decoder for `cfg` with deterministic seeded weights.
    ///
    /// candle's CPU device has no `set_seed`, so the layers are constructed
    /// directly from a seeded LCG as trainable [`Var`]s and registered by name in
    /// the [`VarMap`] (so the optimizer, save, and load all see them). Biases start
    /// at zero, weights use a LeCun-style uniform scale.
    fn build(cfg: &TrainConfig) -> Result<TimbreDecoder, ModelError> {
        let device = Device::Cpu;
        let varmap = VarMap::new();

        let mut state = cfg.seed | 1;
        let mut draw = |scale: f32| -> f32 {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (((state >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0) * scale
        };
        let w1_scale = 1.0f32; // fan_in = 1
        let w2_scale = (1.0 / cfg.hidden as f32).sqrt();
        let w1: Vec<f32> = (0..cfg.hidden).map(|_| draw(w1_scale)).collect();
        let w2: Vec<f32> = (0..cfg.n_harmonics * cfg.hidden)
            .map(|_| draw(w2_scale))
            .collect();

        let w1 = Var::from_vec(w1, (cfg.hidden, 1), &device).map_err(train_err)?;
        let b1 = Var::from_vec(vec![0.0f32; cfg.hidden], cfg.hidden, &device).map_err(train_err)?;
        let w2 = Var::from_vec(w2, (cfg.n_harmonics, cfg.hidden), &device).map_err(train_err)?;
        let b2 = Var::from_vec(vec![0.0f32; cfg.n_harmonics], cfg.n_harmonics, &device)
            .map_err(train_err)?;

        // The layers must reference the *same* tensors that live in the VarMap
        // (the optimizer steps `varmap.all_vars()`, and autograd keys gradients by
        // tensor id), so insert the vars and read the layer tensors back out.
        let (l1, l2) = {
            let mut data = varmap
                .data()
                .lock()
                .map_err(|_| ModelError::Train("varmap lock poisoned".into()))?;
            data.insert("l1.weight".into(), w1);
            data.insert("l1.bias".into(), b1);
            data.insert("l2.weight".into(), w2);
            data.insert("l2.bias".into(), b2);
            let l1 = Linear::new(
                data["l1.weight"].as_tensor().clone(),
                Some(data["l1.bias"].as_tensor().clone()),
            );
            let l2 = Linear::new(
                data["l2.weight"].as_tensor().clone(),
                Some(data["l2.bias"].as_tensor().clone()),
            );
            (l1, l2)
        };
        Ok(TimbreDecoder {
            varmap,
            l1,
            l2,
            device,
        })
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor, ModelError> {
        let h = self.l1.forward(x).map_err(train_err)?;
        let h = h.relu().map_err(train_err)?;
        let logits = self.l2.forward(&h).map_err(train_err)?;
        softmax(&logits)
    }

    /// The decoder's current harmonic-amplitude distribution (sums to 1).
    pub fn harmonics(&self) -> Result<Vec<f32>, ModelError> {
        let x = Tensor::new(&[[1.0f32]], &self.device).map_err(train_err)?;
        let y = self.forward(&x)?;
        let row = y.get(0).map_err(train_err)?;
        row.to_vec1::<f32>().map_err(train_err)
    }

    /// Saves the trained weights to `path` (safetensors).
    pub(crate) fn save(&self, path: &Path) -> Result<(), ModelError> {
        self.varmap.save(path).map_err(train_err)
    }

    /// Loads weights from `path` into a decoder shaped for `cfg`.
    pub(crate) fn load(path: &Path, cfg: &TrainConfig) -> Result<TimbreDecoder, ModelError> {
        let mut decoder = TimbreDecoder::build(cfg)?;
        decoder.varmap.load(path).map_err(train_err)?;
        Ok(decoder)
    }
}

/// Trains a timbre decoder to reproduce `target` (a harmonic-amplitude vector),
/// reporting per-epoch progress via `progress` and returning the decoder plus the
/// loss history. Deterministic for a fixed `cfg.seed`; CPU-only.
pub fn train_timbre(
    target: &[f32],
    cfg: &TrainConfig,
    progress: &mut dyn FnMut(TrainProgress),
) -> Result<(TimbreDecoder, Vec<TrainProgress>), ModelError> {
    if target.len() != cfg.n_harmonics {
        return Err(ModelError::Train(format!(
            "target length {} != n_harmonics {}",
            target.len(),
            cfg.n_harmonics
        )));
    }
    let decoder = TimbreDecoder::build(cfg)?;
    let device = &decoder.device;
    let x = Tensor::new(&[[1.0f32]], device).map_err(train_err)?;
    let normalized = normalize(target.to_vec());
    let y = Tensor::from_vec(normalized, (1, cfg.n_harmonics), device).map_err(train_err)?;

    let params = ParamsAdamW {
        lr: cfg.lr,
        ..Default::default()
    };
    let mut opt = AdamW::new(decoder.varmap.all_vars(), params).map_err(train_err)?;

    let mut history = Vec::with_capacity(cfg.epochs);
    for epoch in 0..cfg.epochs {
        let pred = decoder.forward(&x)?;
        let loss = candle_nn::loss::mse(&pred, &y).map_err(train_err)?;
        opt.backward_step(&loss).map_err(train_err)?;
        let value = loss.to_scalar::<f32>().map_err(train_err)?;
        let step = TrainProgress { epoch, loss: value };
        progress(step);
        history.push(step);
    }
    Ok((decoder, history))
}

/// A differentiable last-dim softmax (candle's `ops::softmax_last_dim` has no
/// backward, so we build it from `exp`/`sum`/`div`).
fn softmax(x: &Tensor) -> Result<Tensor, ModelError> {
    let max = x.max_keepdim(D::Minus1).map_err(train_err)?;
    let shifted = x.broadcast_sub(&max).map_err(train_err)?;
    let e = shifted.exp().map_err(train_err)?;
    let sum = e.sum_keepdim(D::Minus1).map_err(train_err)?;
    e.broadcast_div(&sum).map_err(train_err)
}

fn train_err(e: candle_core::Error) -> ModelError {
    ModelError::Train(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goertzel_peaks_at_the_tone_frequency() {
        let sr = 48_000u32;
        let n = 4096;
        let tone: Vec<f32> = (0..n)
            .map(|i| (TAU * 440.0 * i as f64 / f64::from(sr)).sin() as f32)
            .collect();
        let at_440 = goertzel(&tone, sr, 440.0);
        let at_880 = goertzel(&tone, sr, 880.0);
        assert!(at_440 > at_880 * 5.0, "energy concentrates at 440 Hz");
    }

    #[test]
    fn softmax_is_differentiable_and_normalizes() {
        let device = Device::Cpu;
        let w = Var::from_vec(vec![0.5f32, -0.3, 0.2, 0.1], (1, 4), &device).unwrap();
        let out = softmax(w.as_tensor()).unwrap();
        let sum = out.sum_all().unwrap().to_scalar::<f32>().unwrap();
        assert!((sum - 1.0).abs() < 1e-5, "softmax sums to 1");
        // Gradient must flow through our softmax (candle's has no backward).
        let loss = out.sqr().unwrap().sum_all().unwrap();
        let grads = loss.backward().unwrap();
        assert!(grads.get(&w).is_some(), "softmax must be differentiable");
    }

    #[test]
    fn normalize_sums_to_one_and_uniform_on_zero() {
        let n = normalize(vec![1.0, 3.0]);
        assert!((n.iter().sum::<f32>() - 1.0).abs() < 1e-6);
        assert_eq!(normalize(vec![0.0, 0.0]), vec![0.5, 0.5]);
    }
}
