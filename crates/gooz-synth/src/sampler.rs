//! [`Sampler`] — a recording made playable across the ratio grid (R-0039).
//!
//! The structure mirrors [`render_notes`](crate::render_notes) deliberately:
//! same onset placement, same peak normalization, same FX tail. Only the voice
//! differs — a shifted copy of a recording instead of a plucked string.

use gooz_dsp::{DspError, QuantizedNote, max_output_samples, shift_pitch};
use gooz_ratio::{Ratio, RatioError};

use crate::mix::normalize_peak;
use crate::render::RenderConfig;

/// A recording made playable across the ratio grid.
///
/// The recording's **own pitch is the root**: at [`root_octave`] the degree
/// `1:1` is the sound exactly as recorded, and every other degree is that sound
/// shifted by a ratio (R-0038). Nothing here needs to know what pitch the
/// recording actually has — which is why a knock or a door is as playable as a
/// hum.
///
/// # Why `root_octave` exists
///
/// A [`QuantizedNote`]'s `octave` counts octaves **above the pitch grid's
/// root**, and that root is a per-song, user-settable setting. Without this
/// field the sampler would silently mean "the grid's root pitch *is* the
/// recording's pitch": the same melody quantized against a 440 Hz root plays
/// the recording unshifted, and against a 55 Hz root plays it three octaves up
/// and four times shorter. `root_octave` names the octave the recording sits
/// at, so the caller says where its instrument lives instead of inheriting it
/// from an unrelated setting.
///
/// [`root_octave`]: Sampler::root_octave
///
/// ```
/// use gooz_synth::Sampler;
///
/// let sampler = Sampler::new(vec![0.1, -0.2, 0.3]).expect("a finite recording");
/// assert_eq!(sampler.recording(), &[0.1, -0.2, 0.3]);
/// assert_eq!(sampler.root_octave(), 0);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Sampler {
    recording: Vec<f32>,
    root_octave: i32,
}

impl Sampler {
    /// Wraps a recording, rooted at octave 0.
    ///
    /// An empty buffer is accepted: an instrument with nothing recorded into it
    /// renders silence, not an error.
    ///
    /// # Errors
    ///
    /// [`DspError::NonFiniteSample`] if any sample is NaN or infinite, and
    /// [`DspError::SampleOutOfRange`] if any lies outside `[-1, 1]`.
    ///
    /// Both are checked once, here, rather than per note. A single NaN makes
    /// every shift of the recording fail, which silences the entire part with
    /// no indication of why; and a sample far outside audio's range overflows
    /// the mix the moment two notes overlap, which normalization then turns
    /// into NaN. Rejecting both where the take can still be re-recorded is the
    /// honest place, and it gives this type the invariant its renderer relies
    /// on: **a `Sampler` holds audio**.
    ///
    /// These are the only failures caught here. A recording can still be
    /// refused *later*, per note, by the shift itself — see
    /// [`render_sampled_notes`].
    pub fn new(recording: Vec<f32>) -> Result<Sampler, DspError> {
        if recording.iter().any(|sample| !sample.is_finite()) {
            return Err(DspError::NonFiniteSample);
        }
        if recording.iter().any(|sample| sample.abs() > 1.0) {
            return Err(DspError::SampleOutOfRange);
        }
        Ok(Sampler {
            recording,
            root_octave: 0,
        })
    }

    /// Sets the octave at which degree `1:1` plays the recording unshifted.
    pub fn rooted_at_octave(self, root_octave: i32) -> Sampler {
        Sampler {
            root_octave,
            ..self
        }
    }

    /// The recording this instrument plays.
    pub fn recording(&self) -> &[f32] {
        &self.recording
    }

    /// The octave at which degree `1:1` plays the recording unshifted.
    pub fn root_octave(&self) -> i32 {
        self.root_octave
    }
}

/// The ratio a note is played at: its grid degree, stacked with its octave
/// **relative to the sampler's root octave**.
///
/// Exact rational arithmetic end to end — `stack`/`unstack` report overflow as
/// a [`RatioError`], so an absurd octave is typed rather than a panic.
fn voice_ratio(note: &QuantizedNote, root_octave: i32) -> Result<Ratio, RatioError> {
    let Some(octaves) = note.octave.checked_sub(root_octave) else {
        return Err(RatioError::Overflow);
    };
    let mut ratio = note.degree;
    for _ in 0..octaves.unsigned_abs() {
        ratio = if octaves > 0 {
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
/// [`max_output_samples`] is skipped, because a song with one silent note is a
/// better answer than no song. Deterministic for a given input and config.
///
/// ```
/// use gooz_synth::{QuantizedNote, Ratio, RenderConfig, Sampler, render_sampled_notes};
///
/// let sampler = Sampler::new((0..4_800).map(|i| (i as f32 / 100.0).sin()).collect())?;
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
/// # Ok::<(), gooz_dsp::DspError>(())
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
        let Ok(ratio) = voice_ratio(note, sampler.root_octave) else {
            continue;
        };
        // Cheap checks first: a note that cannot be placed must not pay for a
        // resample it will never use.
        let Some(onset) = onset_sample(note.onset_secs, sample_rate) else {
            continue;
        };
        let Ok(voice) = shift_pitch(&sampler.recording, sample_rate, ratio) else {
            continue;
        };
        let Some(end) = onset.checked_add(voice.len()) else {
            continue;
        };
        if end > max_output_samples(sample_rate) {
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
///
/// `onset_secs` is caller data, and `1e18 · sample_rate` saturates a `usize`
/// cast into a capacity-overflow abort — the same float-cast trap that bit
/// `shift_pitch` (R-0038) and that `render_notes` still carries (#71).
fn onset_sample(onset_secs: f64, sample_rate: u32) -> Option<usize> {
    if !onset_secs.is_finite() || onset_secs < 0.0 {
        return None;
    }
    let onset = (onset_secs * f64::from(sample_rate)).round();
    (onset <= max_output_samples(sample_rate) as f64).then_some(onset as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(octave: i32) -> QuantizedNote {
        QuantizedNote {
            degree: Ratio::UNISON,
            octave,
            freq_hz: 220.0,
            cents_offset: 0.0,
            onset_step: 0,
            onset_secs: 0.0,
            duration_secs: 0.5,
        }
    }

    #[test]
    fn an_octave_too_high_to_form_is_a_typed_overflow() {
        // 64 octaves is where `num` reaches 2^64. AC4 asks for a typed error,
        // and nothing else in the suite can observe the type.
        assert_eq!(voice_ratio(&note(64), 0), Err(RatioError::Overflow));
        assert_eq!(voice_ratio(&note(-64), 0), Err(RatioError::Overflow));
    }

    #[test]
    fn the_root_octave_is_what_the_note_is_measured_against() {
        // A note three octaves above the grid root, played by an instrument
        // that also sits three octaves up, is unshifted.
        assert_eq!(voice_ratio(&note(3), 3), Ok(Ratio::UNISON));
        assert_eq!(voice_ratio(&note(3), 2), Ok(Ratio::OCTAVE));
    }

    #[test]
    fn an_octave_difference_that_overflows_i32_is_typed_too() {
        assert_eq!(voice_ratio(&note(i32::MIN), 1), Err(RatioError::Overflow));
    }
}
