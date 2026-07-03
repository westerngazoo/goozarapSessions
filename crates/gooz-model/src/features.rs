//! Reference-audio feature extraction: a compact, ratio-native profile.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use gooz_dsp::{Config, NoteEvent, analyze};
use gooz_ratio::PitchGrid;

use crate::error::ModelError;

/// The feature profile format version.
pub const FEATURE_FORMAT_VERSION: u32 = 1;

/// One grid degree's share of the reference's harmony, weighted by how long it
/// sounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatioWeight {
    /// Degree ratio numerator.
    pub num: u64,
    /// Degree ratio denominator.
    pub den: u64,
    /// Normalized weight in `[0, 1]`; the weights sum to `1.0` (± ε).
    pub weight: f64,
}

/// A compact, inspectable description of a reference recording — the raw material
/// an influence model trains on. Ratio-native and derived-only (never stores the
/// reference audio).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureProfile {
    /// The profile format version.
    pub format_version: u32,
    /// Sample rate of the analyzed reference, in Hz.
    pub sample_rate: u32,
    /// Reference duration, in seconds.
    pub duration_secs: f64,
    /// Estimated tempo in BPM (`0.0` when fewer than two onsets were found).
    pub tempo_bpm: f64,
    /// Onsets per second.
    pub onset_density: f64,
    /// Loudness proxy: RMS amplitude.
    pub rms: f64,
    /// Brightness proxy: zero-crossing rate in `[0, 1]`.
    pub brightness: f64,
    /// Harmony as a histogram over grid degrees (weights sum to ~1).
    pub ratios: Vec<RatioWeight>,
}

/// Reduces a reference recording to a [`FeatureProfile`]: overall stats, a
/// rhythm profile, and a ratio/harmony histogram against `grid`.
///
/// Analysis errors (empty / zero-rate / non-finite / too-short input) are
/// propagated as [`ModelError::Extract`]. Deterministic; never panics.
pub fn extract_features(
    samples: &[f32],
    sample_rate: u32,
    grid: &PitchGrid,
    cfg: &Config,
) -> Result<FeatureProfile, ModelError> {
    let t = analyze(samples, sample_rate, cfg).map_err(|e| ModelError::Extract(e.to_string()))?;

    let duration_secs = if sample_rate == 0 {
        0.0
    } else {
        samples.len() as f64 / f64::from(sample_rate)
    };
    let onset_times: Vec<f64> = t.onsets.iter().map(|o| o.time_secs).collect();
    let onset_density = if duration_secs > 0.0 {
        onset_times.len() as f64 / duration_secs
    } else {
        0.0
    };

    Ok(FeatureProfile {
        format_version: FEATURE_FORMAT_VERSION,
        sample_rate,
        duration_secs,
        tempo_bpm: estimate_bpm(&onset_times),
        onset_density,
        rms: rms(samples),
        brightness: zero_crossing_rate(samples),
        ratios: ratio_histogram(&t.notes, grid),
    })
}

/// RMS amplitude of the signal (`0.0` for empty input).
pub(crate) fn rms(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f64 = samples.iter().map(|&x| f64::from(x) * f64::from(x)).sum();
    (sum_sq / samples.len() as f64).sqrt()
}

/// Fraction of adjacent sample pairs that change sign, in `[0, 1]` — a coarse
/// brightness proxy (`0.0` for fewer than two samples).
pub(crate) fn zero_crossing_rate(samples: &[f32]) -> f64 {
    if samples.len() < 2 {
        return 0.0;
    }
    let crossings = samples
        .windows(2)
        .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
        .count();
    crossings as f64 / (samples.len() - 1) as f64
}

/// Estimates tempo from onset times: `60 / median(inter-onset interval)`.
/// Returns `0.0` for fewer than two onsets or a non-positive median interval.
pub(crate) fn estimate_bpm(onset_times: &[f64]) -> f64 {
    if onset_times.len() < 2 {
        return 0.0;
    }
    let mut iois: Vec<f64> = onset_times.windows(2).map(|w| w[1] - w[0]).collect();
    iois.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = iois.len() / 2;
    let median = if iois.len().is_multiple_of(2) {
        (iois[mid - 1] + iois[mid]) / 2.0
    } else {
        iois[mid]
    };
    if median > 0.0 { 60.0 / median } else { 0.0 }
}

/// Builds the duration-weighted, grid-snapped ratio histogram, normalized to sum
/// `1.0`. Notes whose pitch is non-positive/non-finite (unsnappable) are skipped;
/// an empty result means no usable pitch was found.
pub(crate) fn ratio_histogram(notes: &[NoteEvent], grid: &PitchGrid) -> Vec<RatioWeight> {
    let mut buckets: BTreeMap<(u64, u64), f64> = BTreeMap::new();
    let mut total = 0.0f64;
    for note in notes {
        let Ok(snapped) = grid.snap(f64::from(note.pitch_hz)) else {
            continue;
        };
        let weight = note.duration_secs.max(0.0);
        if weight <= 0.0 {
            continue;
        }
        *buckets
            .entry((snapped.degree.num(), snapped.degree.den()))
            .or_insert(0.0) += weight;
        total += weight;
    }
    if total <= 0.0 {
        return Vec::new();
    }
    buckets
        .into_iter()
        .map(|((num, den), w)| RatioWeight {
            num,
            den,
            weight: w / total,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_of_silence_is_zero_and_of_dc_is_one() {
        assert_eq!(rms(&[]), 0.0);
        assert_eq!(rms(&[0.0, 0.0, 0.0]), 0.0);
        assert!((rms(&[1.0, -1.0, 1.0, -1.0]) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn zcr_counts_sign_changes() {
        assert_eq!(zero_crossing_rate(&[0.5]), 0.0);
        // +,-,+,- → 3 crossings over 3 pairs = 1.0
        assert!((zero_crossing_rate(&[1.0, -1.0, 1.0, -1.0]) - 1.0).abs() < 1e-12);
        // all positive → 0 crossings
        assert_eq!(zero_crossing_rate(&[0.1, 0.2, 0.3]), 0.0);
    }

    #[test]
    fn estimate_bpm_from_regular_onsets() {
        // Onsets every 0.5 s → 120 BPM.
        let onsets = [0.0, 0.5, 1.0, 1.5, 2.0];
        assert!((estimate_bpm(&onsets) - 120.0).abs() < 1e-9);
        // Fewer than two onsets → 0.
        assert_eq!(estimate_bpm(&[0.3]), 0.0);
        assert_eq!(estimate_bpm(&[]), 0.0);
    }
}
