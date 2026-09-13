//! The input guard every entry point in this crate shares.

use crate::error::DspError;

/// Rejects the three things no analysis or transform can work with: an empty
/// buffer, a zero sample rate, and a non-finite sample — which would poison
/// every sum, sort, and comparison downstream of it.
///
/// Living in one place is the point: the same bad input reports the same error
/// whichever door of the crate it arrives through.
pub(crate) fn input(signal: &[f32], sample_rate: u32) -> Result<(), DspError> {
    if signal.is_empty() {
        return Err(DspError::EmptySignal);
    }
    if sample_rate == 0 {
        return Err(DspError::InvalidSampleRate);
    }
    if signal.iter().any(|sample| !sample.is_finite()) {
        return Err(DspError::NonFiniteSample);
    }
    Ok(())
}
