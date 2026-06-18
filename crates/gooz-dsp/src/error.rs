//! The analysis crate's typed error.

use std::fmt;

/// Every fallible analysis reports one of these. Fieldless and `Copy`, matching
/// the other crates' error style; the library never panics on these paths.
///
/// ```
/// use gooz_dsp::{analyze, Config, DspError};
///
/// assert_eq!(analyze(&[], 48_000, &Config::default()), Err(DspError::EmptySignal));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DspError {
    /// No samples to analyze.
    EmptySignal,
    /// The sample rate was zero.
    InvalidSampleRate,
    /// The analysis window is longer than the signal.
    WindowTooLarge,
    /// The input contains a NaN or infinite sample.
    NonFiniteSample,
}

impl fmt::Display for DspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            DspError::EmptySignal => "the signal has no samples",
            DspError::InvalidSampleRate => "the sample rate must be non-zero",
            DspError::WindowTooLarge => "the analysis window is longer than the signal",
            DspError::NonFiniteSample => {
                "the signal contains a non-finite (NaN or infinite) sample"
            }
        };
        f.write_str(message)
    }
}

impl std::error::Error for DspError {}
