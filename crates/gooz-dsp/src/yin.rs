//! YIN pitch detection (de Cheveigné & Kawahara) and the pitch track over a
//! whole signal.

use crate::error::DspError;
use crate::transcribe::{Config, PitchFrame, PitchTrack, validate};

/// Generous fixed search range (Hz). Wider than `[f_min, f_max]` so the true
/// fundamental is found even when it is above `f_max` — it is then rejected by
/// the `[f_min, f_max]` gate rather than aliased to an in-range subharmonic.
const SEARCH_F_HI: f32 = 2000.0;
const SEARCH_F_LO: f32 = 50.0;

/// Detects the pitch of one frame: `(Some(f0), confidence)` if voiced, else
/// `(None, confidence)`.
fn detect_pitch(frame: &[f32], sample_rate: u32, cfg: &Config) -> (Option<f32>, f32) {
    let w = frame.len() / 2;
    let sr = sample_rate as f32;
    let tau_max = ((sr / SEARCH_F_LO) as usize).min(w);
    let tau_min = ((sr / SEARCH_F_HI) as usize).max(2);
    if w < 2 || tau_min >= tau_max {
        return (None, 0.0);
    }

    // Difference function d(τ) = Σ_j (x[j] − x[j+τ])², τ in 1..=tau_max.
    let mut diff = vec![0.0f32; tau_max + 1];
    for (tau, slot) in diff.iter_mut().enumerate().skip(1) {
        let mut sum = 0.0f32;
        for j in 0..w {
            let delta = frame[j] - frame[j + tau];
            sum += delta * delta;
        }
        *slot = sum;
    }

    // Cumulative mean normalized difference d'(τ).
    let mut cmnd = vec![1.0f32; tau_max + 1];
    let mut running = 0.0f32;
    for tau in 1..=tau_max {
        running += diff[tau];
        cmnd[tau] = if running > 0.0 {
            diff[tau] * tau as f32 / running
        } else {
            1.0
        };
    }

    // First local minimum below the threshold within the search range, else the
    // global minimum over that range.
    let mut tau_star = None;
    let mut tau = tau_min;
    while tau <= tau_max {
        if cmnd[tau] < cfg.yin_threshold {
            while tau < tau_max && cmnd[tau + 1] < cmnd[tau] {
                tau += 1;
            }
            tau_star = Some(tau);
            break;
        }
        tau += 1;
    }
    let tau_star = tau_star.unwrap_or_else(|| {
        (tau_min..=tau_max)
            .min_by(|&a, &b| cmnd[a].total_cmp(&cmnd[b]))
            .unwrap_or(tau_min)
    });

    let refined = parabolic(&cmnd, tau_star).max(tau_min as f32);
    let f0 = sr / refined;
    let confidence = (1.0 - cmnd[tau_star]).clamp(0.0, 1.0);
    let voiced = cmnd[tau_star] < cfg.yin_threshold && f0 >= cfg.f_min && f0 <= cfg.f_max;
    (voiced.then_some(f0), confidence)
}

/// Sub-sample period via parabolic interpolation around `tau`.
fn parabolic(cmnd: &[f32], tau: usize) -> f32 {
    if tau == 0 || tau + 1 >= cmnd.len() {
        return tau as f32;
    }
    let (a, b, c) = (cmnd[tau - 1], cmnd[tau], cmnd[tau + 1]);
    let denom = a + c - 2.0 * b;
    if denom.abs() < 1e-12 {
        tau as f32
    } else {
        tau as f32 + 0.5 * (a - c) / denom
    }
}

/// Tracks pitch frame-by-frame across a signal (a frame every `cfg.hop`
/// samples). Validates input as [`crate::analyze`] does.
///
/// ```
/// use gooz_dsp::{pitch_track, Config};
///
/// let sr = 48_000;
/// let n = (0.2 * sr as f64) as usize;
/// let signal: Vec<f32> = (0..n)
///     .map(|i| 0.8 * (std::f64::consts::TAU * 220.0 * i as f64 / sr as f64).sin() as f32)
///     .collect();
/// let track = pitch_track(&signal, sr, &Config::default()).unwrap();
/// assert!(track.frames.iter().any(|f| f.f0_hz.is_some()));
/// ```
pub fn pitch_track(signal: &[f32], sample_rate: u32, cfg: &Config) -> Result<PitchTrack, DspError> {
    validate(signal, sample_rate, cfg)?;
    let hop = cfg.hop.max(1);
    let mut frames = Vec::new();
    let mut start = 0usize;
    while start + cfg.window <= signal.len() {
        let frame = &signal[start..start + cfg.window];
        let (f0_hz, confidence) = detect_pitch(frame, sample_rate, cfg);
        let centre = start + cfg.window / 2;
        frames.push(PitchFrame {
            time_secs: centre as f64 / sample_rate as f64,
            f0_hz,
            confidence,
        });
        start += hop;
    }
    Ok(PitchTrack { frames })
}
