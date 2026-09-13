//! [`Sampler`] — a recording made playable across the ratio grid (R-0039).
//!
//! The structure mirrors [`render_notes`](crate::render_notes) deliberately:
//! same onset placement, same peak normalization, same FX tail. Only the voice
//! differs — a shifted copy of a recording instead of a plucked string.

use gooz_dsp::{QuantizedNote, Ratio, shift_pitch};
use gooz_ratio::RatioError;

use crate::mix::normalize_peak;
use crate::render::RenderConfig;

/// The longest mix this renderer will build, in samples — ten minutes at
/// 192 kHz, the highest rate the project treats as real gear.
///
/// `onset_secs` is caller data, and `1e18 · sample_rate` saturates a `usize`
/// cast into a capacity-overflow abort. A note that would push the mix past
/// this is skipped, like any other note that cannot be rendered.
const MAX_RENDER_SAMPLES: usize = 600 * 192_000;

/// A recording made playable across the ratio grid.
///
/// The recording's **own pitch is the root**: degree `1:1` is the sound exactly
/// as recorded, and every other degree is that sound shifted by a ratio
/// (R-0038). Nothing in this path needs to know what pitch the recording
/// actually has — which is why a knock or a door is as playable as a hum.
///
/// ```
/// use gooz_synth::Sampler;
///
/// let sampler = Sampler::new(vec![0.1, -0.2, 0.3]);
/// assert_eq!(sampler.recording(), &[0.1, -0.2, 0.3]);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Sampler {
    recording: Vec<f32>,
}

impl Sampler {
    /// Wraps a recording. Any buffer is accepted, an empty one included: an
    /// instrument with nothing recorded into it renders silence, not an error.
    pub fn new(recording: Vec<f32>) -> Sampler {
        Sampler { recording }
    }

    /// The recording this instrument plays.
    pub fn recording(&self) -> &[f32] {
        &self.recording
    }
}

/// The ratio a note is played at: its grid degree, stacked with its octave.
///
/// Exact rational arithmetic end to end — `stack`/`unstack` report overflow as
/// a [`RatioError`], so an absurd octave is typed rather than a panic.
fn voice_ratio(note: &QuantizedNote) -> Result<Ratio, RatioError> {
    let mut ratio = note.degree;
    for _ in 0..note.octave.unsigned_abs() {
        ratio = if note.octave > 0 {
            ratio.stack(Ratio::OCTAVE)?
        } else {
            ratio.unstack(Ratio::OCTAVE)?
        };
    }
    Ok(ratio)
}

/// Renders quantized notes through a recording: one shifted copy of the
/// recording per note, mixed at each note's onset, normalized, then run through
/// the distortion.
///
/// Each note plays as a **one-shot** — the shifted copy rings out in full
/// rather than being cut at `duration_secs`, which would put a click at the end
/// of every note. `cfg.decay` is therefore inert here: a recording carries its
/// own decay. `note.freq_hz` is inert too, and deliberately so — under R-0039
/// the recording is the root, so a note's absolute frequency has no meaning for
/// this instrument.
///
/// Total and panic-free. An empty recording, an empty note list, or a zero
/// sample rate yields an empty buffer; a note whose ratio cannot be formed,
/// whose shift is refused, or whose onset would run past
/// [`MAX_RENDER_SAMPLES`] is skipped, because a song with one silent note is a
/// better answer than no song. Deterministic for a given input and config.
///
/// ```
/// use gooz_synth::{QuantizedNote, Ratio, RenderConfig, Sampler, render_sampled_notes};
///
/// let sampler = Sampler::new((0..4_800).map(|i| (i as f32 / 100.0).sin()).collect());
/// let note = QuantizedNote {
///     degree: Ratio::new(3, 2).unwrap(),
///     octave: 0,
///     freq_hz: 0.0,
///     cents_offset: 0.0,
///     onset_step: 0,
///     onset_secs: 0.0,
///     duration_secs: 0.5,
/// };
/// let audio = render_sampled_notes(&sampler, &[note], 48_000, &RenderConfig::default());
/// assert!(!audio.is_empty());
/// assert!(audio.iter().all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-6));
/// ```
pub fn render_sampled_notes(
    sampler: &Sampler,
    notes: &[QuantizedNote],
    sample_rate: u32,
    cfg: &RenderConfig,
) -> Vec<f32> {
    if sample_rate == 0 || notes.is_empty() || sampler.recording.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<f32> = Vec::new();
    for note in notes {
        let Ok(ratio) = voice_ratio(note) else {
            continue;
        };
        let Ok(voice) = shift_pitch(&sampler.recording, sample_rate, ratio) else {
            continue;
        };
        let Some(onset) = onset_sample(note.onset_secs, sample_rate) else {
            continue;
        };
        let Some(end) = onset.checked_add(voice.len()) else {
            continue;
        };
        if end > MAX_RENDER_SAMPLES {
            continue;
        }
        if out.len() < end {
            out.resize(end, 0.0);
        }
        for (mixed, sample) in out[onset..end].iter_mut().zip(&voice) {
            *mixed += *sample;
        }
    }
    normalize_peak(&mut out);
    for sample in &mut out {
        *sample = cfg.distortion.apply(*sample, cfg.drive);
    }
    out
}

/// Where a note starts, in samples, or `None` if that is not a place in a song.
fn onset_sample(onset_secs: f64, sample_rate: u32) -> Option<usize> {
    if !onset_secs.is_finite() || onset_secs < 0.0 {
        return None;
    }
    let onset = (onset_secs * f64::from(sample_rate)).round();
    (onset <= MAX_RENDER_SAMPLES as f64).then_some(onset as usize)
}
