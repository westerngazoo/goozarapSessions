//! Spectral-flux onset detection.

use rustfft::FftPlanner;
use rustfft::num_complex::Complex;

use crate::error::DspError;
use crate::transcribe::{Config, Onset, validate};

/// Minimum spacing between onsets, in seconds — one attack is one onset.
const MIN_ONSET_GAP_SECS: f64 = 0.030;

/// Detects note onsets via spectral flux: a Hann-windowed STFT, the sum of
/// positive bin-energy increases between frames (with an implicit zero frame
/// before the first, so an attack at sample 0 registers), then an adaptive
/// peak-pick with a minimum inter-onset gap. Validates input as
/// [`crate::analyze`] does.
///
/// ```
/// use gooz_dsp::{detect_onsets, Config};
///
/// let sr = 48_000;
/// // 50 ms silence, then 0.2 s of 440 Hz: one onset at the tone start.
/// let lead = vec![0.0f32; (0.05 * sr as f64) as usize];
/// let tone: Vec<f32> = (0..(0.2 * sr as f64) as usize)
///     .map(|i| 0.8 * (std::f64::consts::TAU * 440.0 * i as f64 / sr as f64).sin() as f32)
///     .collect();
/// let signal = [lead, tone].concat();
/// let onsets = detect_onsets(&signal, sr, &Config::default()).unwrap();
/// assert_eq!(onsets.len(), 1);
/// ```
pub fn detect_onsets(
    signal: &[f32],
    sample_rate: u32,
    cfg: &Config,
) -> Result<Vec<Onset>, DspError> {
    validate(signal, sample_rate, cfg)?;
    let n = cfg.fft_size.max(2);
    let hop = cfg.hop.max(1);
    if signal.len() < n {
        return Ok(Vec::new());
    }

    let hann: Vec<f32> = (0..n)
        .map(|i| {
            let x = std::f64::consts::TAU * i as f64 / n as f64;
            (0.5 - 0.5 * x.cos()) as f32
        })
        .collect();

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);
    let bins = n / 2;

    // STFT magnitudes (lower half-spectrum) per frame.
    let mut mags: Vec<Vec<f32>> = Vec::new();
    let mut buffer = vec![Complex::new(0.0f32, 0.0); n];
    let mut start = 0usize;
    while start + n <= signal.len() {
        for (i, slot) in buffer.iter_mut().enumerate() {
            *slot = Complex::new(signal[start + i] * hann[i], 0.0);
        }
        fft.process(&mut buffer);
        mags.push(buffer[..bins].iter().map(|c| c.norm()).collect());
        start += hop;
    }

    // Spectral flux with an implicit all-zero frame before the first.
    let flux: Vec<f32> = mags
        .iter()
        .enumerate()
        .map(|(m, frame)| {
            frame
                .iter()
                .enumerate()
                .map(|(k, &mag)| {
                    let prev = if m == 0 { 0.0 } else { mags[m - 1][k] };
                    (mag - prev).max(0.0)
                })
                .sum()
        })
        .collect();

    // Per-frame energy (sum of squared magnitudes, per Parseval — not summed
    // magnitudes, which a broadband transient can inflate). An onset is an
    // energy *increase*; this rejects the edge transient at a note release
    // (tone→silence), which produces positive flux but falling energy.
    let energy: Vec<f32> = mags
        .iter()
        .map(|frame| frame.iter().map(|&m| m * m).sum())
        .collect();

    if flux.is_empty() {
        return Ok(Vec::new());
    }

    // Adaptive threshold over the whole flux: an attack spike towers over the
    // signal's overall flux, while steady-state ripple does not. A local window
    // would track the sustain level too closely and fire on that ripple.
    let count = flux.len() as f32;
    let mean = flux.iter().sum::<f32>() / count;
    let variance = flux.iter().map(|&x| (x - mean) * (x - mean)).sum::<f32>() / count;
    let threshold = mean + cfg.onset_sensitivity * variance.sqrt();

    // Pick frames that clear the threshold and are the maximum within
    // ±onset_window_frames, respecting the minimum inter-onset gap.
    let win = cfg.onset_window_frames;
    let mut onsets = Vec::new();
    let mut last_time = f64::NEG_INFINITY;
    for m in 0..flux.len() {
        if flux[m] <= 0.0 || flux[m] < threshold {
            continue;
        }
        // Reject release transients: the frame must be louder than the one before.
        if m > 0 && energy[m] <= energy[m - 1] {
            continue;
        }
        let lo = m.saturating_sub(win);
        let hi = (m + win).min(flux.len() - 1);
        if !(lo..=hi).all(|i| flux[m] >= flux[i]) {
            continue;
        }
        let time_secs = (m * hop) as f64 / sample_rate as f64;
        if time_secs - last_time >= MIN_ONSET_GAP_SECS {
            onsets.push(Onset {
                time_secs,
                strength: flux[m],
            });
            last_time = time_secs;
        }
    }
    Ok(onsets)
}
