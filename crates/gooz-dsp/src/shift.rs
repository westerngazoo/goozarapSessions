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
//!
//! **Known limitation — upward shifts alias.** Reading faster than the source
//! is decimation, and there is no pre-filter, so content above `SR / (2·ratio)`
//! folds back down instead of disappearing: a 15 kHz tone shifted `2:1` at
//! 48 kHz returns at 18 kHz, not 30 kHz. Classic sampler behaviour, and
//! inaudible on the one-shots this exists to serve (R-0039), but it is a real
//! bound on the shift, not an artifact-free operation. A decimation pre-filter
//! is a later requirement; an acceptance test pins today's folding, so adding
//! one will be a deliberate change rather than a silent one.

use crate::error::DspError;
use gooz_ratio::Ratio;

/// The longest output this function will produce, in seconds.
///
/// Varispeed multiplies length without bound — `1:10_000` turns one second into
/// nearly three hours, and `Ratio` guarantees positive and non-zero but *not*
/// bounded. Past some size the request stopped being musical and became a way
/// to exhaust memory, so it is refused as a typed error rather than allocated.
/// This is also what makes `sample_rate` load-bearing: the resampling itself is
/// dimensionless, but "too long" is a duration, and a duration needs a rate.
const MAX_OUTPUT_SECS: u64 = 600;

/// The highest sample rate this project treats as real gear (192 kHz studio).
///
/// [`MAX_OUTPUT_SECS`] on its own is a *duration*, and the caller picks the
/// rate — so an absurd rate widens it without limit: ten minutes at
/// `u32::MAX` Hz is about ten terabytes, which the allocator will happily
/// reserve lazily and then thrash the machine filling. Clamping the rate makes
/// the duration cap an absolute ceiling on the allocation too.
const MAX_SANE_SAMPLE_RATE: u64 = 192_000;

/// Shifts `signal` up or down by `ratio`.
///
/// The returned buffer sounds at `ratio` times the input's pitch. Its length
/// scales by the inverse: shifting up returns a shorter buffer, down a longer
/// one.
///
/// `Ratio` is positive and non-zero by construction (R-0001), so the shift
/// factor cannot be zero, negative, or NaN — the type removes that error class
/// before it reaches the DSP. It is not *bounded*, though, so an extreme ratio
/// is refused rather than allocated (see [`MAX_OUTPUT_SECS`]).
///
/// Resampling is dimensionless, so `sample_rate` does not change which samples
/// are read; it is validated for consistency with the rest of the crate, and it
/// is what turns the output-length cap into a duration.
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
    crate::validate::input(signal, sample_rate)?;

    if ratio == Ratio::UNISON {
        // Not a correctness guard — the general path already returns these exact
        // bytes, because every read lands on an integer index and interpolates
        // by zero. It skips the arithmetic and says "identity" out loud.
        return Ok(signal.to_vec());
    }

    let out_len = output_len(signal.len(), sample_rate, ratio)?;
    let step = ratio.num() as f64 / ratio.den() as f64;
    Ok((0..out_len)
        .map(|i| sample_at(signal, i as f64 * step))
        .collect())
}

/// How many samples reading `input_len` at `ratio` produces.
///
/// This is `ceil(input_len / (num/den))`, but computed as
/// `ceil(input_len · den / num)` **in integers**: the float round-trip
/// overshoots for some ratios — `5 / (1/49)` evaluates to `245.00000000000003`,
/// whose ceiling is 246 — which appends a duplicated final sample.
///
/// # Errors
///
/// [`DspError::OutputTooLong`] if the count exceeds [`MAX_OUTPUT_SECS`] at
/// this sample rate, capped by [`MAX_SANE_SAMPLE_RATE`].
fn output_len(input_len: usize, sample_rate: u32, ratio: Ratio) -> Result<usize, DspError> {
    // u128 so the intermediate cannot overflow into a false rejection: for a
    // ratio like `u64::MAX : u64::MAX - 1` — very nearly unison — `len · den`
    // leaves u64 long before the division cancels it back down.
    let stretched = input_len as u128 * u128::from(ratio.den());
    let out_len = stretched.div_ceil(u128::from(ratio.num()));
    let limit = MAX_OUTPUT_SECS * u64::from(sample_rate).min(MAX_SANE_SAMPLE_RATE);
    if out_len > u128::from(limit) {
        return Err(DspError::OutputTooLong);
    }
    Ok(out_len as usize)
}

/// Reads the signal at a fractional index, interpolating linearly between the
/// two neighbouring samples and holding the value at the far edge.
///
/// Linear interpolation is exact at integer positions, and its error is a
/// gentle high-frequency roll-off rather than an audible artifact.
///
/// The weighted form `left·(1−f) + right·f` is deliberate, not stylistic. The
/// tidier `left + (right − left)·f` overflows f32 when the two neighbours are
/// more than `f32::MAX` apart, and `inf · 0.0` is NaN — so a *finite* input
/// came back non-finite, even at an integer position where the read is
/// supposed to be exact. `|lerp(a, b)| <= max(|a|, |b|)` is true in ℝ but was
/// false in f32; each term of the weighted form is bounded by its own sample,
/// so it holds in both.
fn sample_at(signal: &[f32], pos: f64) -> f32 {
    let floor = pos.floor();
    let index = floor as usize;
    let Some(&left) = signal.get(index) else {
        // Past the end — reachable on the final reads when the length rounds
        // up. Hold the last sample rather than inventing silence.
        return *signal
            .last()
            .expect("validate::input has already rejected an empty signal");
    };
    let right = signal.get(index + 1).copied().unwrap_or(left);
    let fraction = (pos - floor) as f32;
    left * (1.0 - fraction) + right * fraction
}
