//! [`render_notes`] — render quantized notes into a guitar buffer.

use gooz_dsp::QuantizedNote;

use crate::distortion::Distortion;
use crate::string::KarplusString;

/// Base seed for the per-note pluck excitation (XORed with the note index so
/// each string's noise differs yet the whole render is reproducible).
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// How to render: string decay (ring length) and the distortion FX.
///
/// ```
/// use gooz_synth::{Distortion, RenderConfig};
///
/// let cfg = RenderConfig::default();
/// assert_eq!(cfg.distortion, Distortion::SoftClip);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderConfig {
    /// Karplus-Strong decay factor in `(0, 1)`; higher rings longer.
    pub decay: f32,
    /// The distortion curve.
    pub distortion: Distortion,
    /// Distortion drive amount.
    pub drive: f32,
}

impl Default for RenderConfig {
    fn default() -> RenderConfig {
        RenderConfig {
            decay: 0.996,
            distortion: Distortion::SoftClip,
            drive: 2.0,
        }
    }
}

/// Renders quantized notes into one `f32` buffer: a Karplus-Strong pluck per
/// note (let-ring — each rings its natural decay), mixed, then run through the
/// distortion and normalized so the output never exceeds `[-1, 1]`.
///
/// Total and panic-free: an empty input, a zero sample rate, or input with no
/// renderable notes yields an empty buffer; a note with a non-finite/non-positive
/// frequency is skipped. The render is deterministic for a given input + config.
///
/// ```
/// use gooz_synth::{render_notes, QuantizedNote, Ratio, RenderConfig};
///
/// let notes = vec![QuantizedNote {
///     degree: Ratio::UNISON,
///     octave: 0,
///     freq_hz: 220.0,
///     cents_offset: 0.0,
///     onset_step: 0,
///     onset_secs: 0.0,
///     duration_secs: 0.5,
/// }];
/// let audio = render_notes(&notes, 48_000, &RenderConfig::default());
/// assert!(!audio.is_empty());
/// assert!(audio.iter().all(|s| s.abs() <= 1.0 + 1e-6)); // bounded
/// ```
pub fn render_notes(notes: &[QuantizedNote], sample_rate: u32, cfg: &RenderConfig) -> Vec<f32> {
    if sample_rate == 0 || notes.is_empty() {
        return Vec::new();
    }
    let decay = cfg.decay.clamp(0.5, 0.999_99);
    let mut out: Vec<f32> = Vec::new();
    for (i, note) in notes.iter().enumerate() {
        if !note.freq_hz.is_finite() || note.freq_hz <= 0.0 {
            continue;
        }
        let onset = (note.onset_secs * f64::from(sample_rate)).round().max(0.0) as usize;
        let mut voice = KarplusString::pluck(note.freq_hz, sample_rate, decay, SEED ^ i as u64);
        let end = onset + voice.tail_len();
        if out.len() < end {
            out.resize(end, 0.0);
        }
        for x in &mut out[onset..end] {
            *x += voice.next_sample();
        }
    }
    normalize_peak(&mut out);
    for x in &mut out {
        *x = cfg.distortion.apply(*x, cfg.drive);
    }
    out
}

/// Scales the buffer so its peak magnitude is 1.0 (no-op if already silent), so
/// the distortion sees a full-scale `[-1, 1]` signal.
fn normalize_peak(buf: &mut [f32]) {
    let peak = buf.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if peak > 0.0 {
        let gain = 1.0 / peak;
        for x in buf.iter_mut() {
            *x *= gain;
        }
    }
}
