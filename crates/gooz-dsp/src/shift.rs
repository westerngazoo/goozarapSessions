//! Ratio-native pitch shifting (R-0038 / SPEC-0038).
//!
//! Moves a recorded buffer to another pitch by a [`Ratio`] — "a fifth up" is
//! `3:2`, never a semitone count, so the whole engine keeps speaking one
//! language. This is the primitive that lets *any* recording become an
//! instrument across the grid (R-0039).
//!
//! The method is **varispeed**: the buffer is resampled, so pitch scales by the
//! ratio and length scales by its inverse — classic sampler behaviour, and the
//! desired sound for the one-shot samples that motivated this. Holding length
//! constant needs a phase vocoder, which smears exactly the transients a
//! percussive sample is made of; that becomes its own requirement the day a
//! whole riff must be transposed without changing tempo.

use crate::error::DspError;
use gooz_ratio::Ratio;

/// Shifts `signal` up or down by `ratio`.
///
/// The returned buffer sounds at `ratio` times the input's pitch. Its length
/// scales by the inverse: shifting up returns a shorter buffer, down a longer
/// one.
///
/// `Ratio` is positive and non-zero by construction (R-0001), so the shift
/// factor cannot be zero, negative, or NaN — the type removes that error class
/// before it reaches the DSP.
///
/// # Errors
///
/// [`DspError::EmptySignal`] for an empty buffer,
/// [`DspError::InvalidSampleRate`] for a zero rate, and
/// [`DspError::NonFiniteSample`] if any sample is NaN or infinite.
///
/// ```
/// use gooz_dsp::{Ratio, shift_pitch};
///
/// let sr = 48_000;
/// let sine: Vec<f32> = (0..sr as usize)
///     .map(|i| (std::f64::consts::TAU * 220.0 * i as f64 / f64::from(sr)).sin() as f32)
///     .collect();
///
/// // Up a fifth: shorter buffer, higher pitch.
/// let fifth = shift_pitch(&sine, sr, Ratio::new(3, 2).unwrap()).unwrap();
/// assert!(fifth.len() < sine.len());
///
/// // Unison is an exact identity.
/// assert_eq!(shift_pitch(&sine, sr, Ratio::UNISON).unwrap(), sine);
/// ```
pub fn shift_pitch(signal: &[f32], sample_rate: u32, ratio: Ratio) -> Result<Vec<f32>, DspError> {
    validate(signal, sample_rate)?;

    let step = ratio.num() as f64 / ratio.den() as f64;
    if step == 1.0 {
        // Exact identity, not merely close: every read would land on an integer
        // index anyway, and returning the buffer says so unambiguously.
        return Ok(signal.to_vec());
    }

    let out_len = (signal.len() as f64 / step).ceil() as usize;
    Ok((0..out_len)
        .map(|i| sample_at(signal, i as f64 * step))
        .collect())
}

/// Reads the signal at a fractional index, interpolating linearly between the
/// two neighbouring samples and holding the value at the far edge.
///
/// Linear interpolation is exact at integer positions and its error is a gentle
/// high-frequency roll-off rather than the aliasing artifacts that would be
/// audible on a percussive sample. Because `|lerp(a, b)| <= max(|a|, |b|)`, a
/// bounded input cannot produce an unbounded output.
fn sample_at(signal: &[f32], pos: f64) -> f32 {
    let floor = pos.floor();
    let index = floor as usize;
    let Some(&left) = signal.get(index) else {
        // Past the end (possible on the final sample when out_len rounds up).
        return *signal.last().unwrap_or(&0.0);
    };
    let right = signal.get(index + 1).copied().unwrap_or(left);
    let fraction = (pos - floor) as f32;
    left + (right - left) * fraction
}

/// The crate's standard input guard, shared by every analysis entry point.
fn validate(signal: &[f32], sample_rate: u32) -> Result<(), DspError> {
    if sample_rate == 0 {
        return Err(DspError::InvalidSampleRate);
    }
    if signal.is_empty() {
        return Err(DspError::EmptySignal);
    }
    if signal.iter().any(|s| !s.is_finite()) {
        return Err(DspError::NonFiniteSample);
    }
    Ok(())
}
