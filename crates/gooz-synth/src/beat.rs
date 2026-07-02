//! Euclidean beat renderer (R-0009).

use gooz_ratio::{Pattern, Tempo};

use crate::drums::{DrumKind, mix_hit};

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
    let bar_samples = ((tempo.bar_seconds() * f64::from(sample_rate)).round() as usize).max(1);
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

fn normalize_peak(buf: &mut [f32]) {
    let peak = buf.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if peak > 0.0 {
        let gain = 1.0 / peak;
        for x in buf.iter_mut() {
            *x *= gain;
        }
    }
}
