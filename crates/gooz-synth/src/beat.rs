//! Euclidean beat renderer (R-0009).

use gooz_ratio::{Pattern, Tempo};

use crate::drums::{DrumKind, mix_hit};
use crate::mix::normalize_peak;

/// One drum lane: a Euclidean pattern, a kit voice, and a mix level.
#[derive(Debug, Clone, PartialEq)]
pub struct BeatVoice {
    /// The drum sound.
    pub kind: DrumKind,
    /// The `E(k, n)` step pattern for this lane.
    pub pattern: Pattern,
    /// Lane level in `[0, 1]` (clamped at render time).
    pub level: f32,
}

/// The sample offsets at which [`render_beat`] triggers `pattern`'s hits, for
/// `bars` bars of `tempo` at `sample_rate`: bar by bar, step by step, so
/// non-decreasing. Empty when `bars`, `sample_rate` or the pattern is empty.
/// Two offsets can coincide on a bar shorter than its steps.
pub fn pattern_onsets(pattern: &Pattern, tempo: &Tempo, bars: u32, sample_rate: u32) -> Vec<usize> {
    let steps = pattern.len();
    if bars == 0 || sample_rate == 0 || steps == 0 {
        return Vec::new();
    }
    let bar = bar_len(tempo, sample_rate);
    (0..bars as usize)
        .flat_map(|b| {
            (0..steps)
                .filter(|&step| pattern.is_onset(step))
                .map(move |step| b * bar + step_offset(step, steps, bar))
        })
        .collect()
}

/// A bar's length in samples, never zero.
fn bar_len(tempo: &Tempo, sample_rate: u32) -> usize {
    ((tempo.bar_seconds() * f64::from(sample_rate)).round() as usize).max(1)
}

/// Where step `step` of `steps` lands inside a bar of `bar_len` samples:
/// rounded, and kept inside the bar.
fn step_offset(step: usize, steps: usize, bar_len: usize) -> usize {
    let offset = ((step as f64 / steps as f64) * bar_len as f64).round() as usize;
    offset.min(bar_len.saturating_sub(1))
}

/// Renders `voices` into a bar-aligned beat buffer: for each bar, every pattern
/// onset triggers a one-shot at the corresponding sample offset. Returns an
/// empty buffer when `bars == 0`, `sample_rate == 0`, or `voices` is empty.
/// Deterministic and peak-normalized to `[-1, 1]`.
///
/// ```
/// use gooz_ratio::{Pattern, Tempo};
/// use gooz_synth::{render_beat, BeatVoice, DrumKind};
///
/// let tempo = Tempo::new(120.0, 4.0).unwrap();
/// let voices = vec![
///     BeatVoice {
///         kind: DrumKind::Kick,
///         pattern: Pattern::euclidean(4, 16).unwrap(),
///         level: 1.0,
///     },
///     BeatVoice {
///         kind: DrumKind::Snare,
///         pattern: Pattern::euclidean(2, 16).unwrap().rotate(4),
///         level: 0.9,
///     },
///     BeatVoice {
///         kind: DrumKind::HiHat,
///         pattern: Pattern::euclidean(7, 16).unwrap(),
///         level: 0.7,
///     },
/// ];
/// let beat = render_beat(&voices, &tempo, 2, 48_000);
/// assert!(!beat.is_empty());
/// assert_eq!(beat.len(), 2 * (tempo.bar_seconds() * 48_000.0).round() as usize);
/// ```
pub fn render_beat(voices: &[BeatVoice], tempo: &Tempo, bars: u32, sample_rate: u32) -> Vec<f32> {
    if bars == 0 || sample_rate == 0 || voices.is_empty() {
        return Vec::new();
    }
    let bar_samples = bar_len(tempo, sample_rate);
    let total = bar_samples * bars as usize;
    let mut out = vec![0.0f32; total];

    for bar in 0..bars {
        let bar_start = bar as usize * bar_samples;
        for voice in voices {
            let len = voice.pattern.len();
            if len == 0 {
                continue;
            }
            for step in 0..len {
                if !voice.pattern.is_onset(step) {
                    continue;
                }
                let offset = bar_start + step_offset(step, len, bar_samples);
                mix_hit(voice.kind, sample_rate, voice.level, &mut out, offset);
            }
        }
    }

    normalize_peak(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DrumKind;

    /// `render_beat` as it was before the hit-offset arithmetic moved into
    /// `pattern_onsets` (SPEC-0033 §2.2): the reference the refactor must match
    /// bit for bit.
    fn render_beat_before_refactor(
        voices: &[BeatVoice],
        tempo: &Tempo,
        bars: u32,
        sample_rate: u32,
    ) -> Vec<f32> {
        let bar_samples = ((tempo.bar_seconds() * f64::from(sample_rate)).round() as usize).max(1);
        let mut out = vec![0.0f32; bar_samples * bars as usize];
        for bar in 0..bars {
            let bar_start = bar as usize * bar_samples;
            for voice in voices {
                let len = voice.pattern.len();
                for step in (0..len).filter(|&step| voice.pattern.is_onset(step)) {
                    let offset_in_bar =
                        ((step as f64 / len as f64) * bar_samples as f64).round() as usize;
                    let offset = bar_start + offset_in_bar.min(bar_samples.saturating_sub(1));
                    mix_hit(voice.kind, sample_rate, voice.level, &mut out, offset);
                }
            }
        }
        normalize_peak(&mut out);
        out
    }

    /// Three lanes of different step counts, rotated, whose tails ring across
    /// bar lines, over three bars.
    #[test]
    fn render_beat_is_unchanged_by_the_onset_refactor() {
        let tempo = Tempo::new(180.0, 4.0).expect("valid tempo");
        let lane = |kind, k, n, rotate, level| BeatVoice {
            kind,
            pattern: Pattern::euclidean(k, n)
                .expect("valid E(k, n)")
                .rotate(rotate),
            level,
        };
        let voices = [
            lane(DrumKind::Kick, 5, 16, 0, 1.0),
            lane(DrumKind::Snare, 3, 8, 3, 0.9),
            lane(DrumKind::HiHat, 7, 12, 5, 0.6),
        ];
        let now = render_beat(&voices, &tempo, 3, 48_000);
        let before = render_beat_before_refactor(&voices, &tempo, 3, 48_000);
        assert_eq!(now.len(), before.len());
        let differing = now
            .iter()
            .zip(&before)
            .filter(|(a, b)| a.to_bits() != b.to_bits())
            .count();
        assert_eq!(differing, 0, "samples that changed bits");
    }
}
