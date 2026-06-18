//! The rhythm modules' typed error.

use std::fmt;

/// Every fallible beat-core operation reports one of these.
///
/// Kept separate from [`crate::RatioError`] (the pitch core's error): the two
/// domains barely overlap. Note `EmptyGrid` exists in both types with distinct
/// meanings — here it is a step or pulse count of zero — but they never share a
/// `Result`.
///
/// ```
/// use gooz_ratio::{BarGrid, BeatError};
///
/// assert_eq!(BarGrid::new(0), Err(BeatError::EmptyGrid));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeatError {
    /// A step or pulse count that must be at least one was zero.
    EmptyGrid,
    /// A Euclidean rhythm `E(k, n)` was asked for more onsets than steps.
    TooManyOnsets,
    /// A non-finite bar phase was passed to quantization.
    InvalidPhase,
    /// A tempo or beats-per-bar value was non-finite or non-positive.
    InvalidTempo,
}

impl fmt::Display for BeatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            BeatError::EmptyGrid => "a step or pulse count must be at least one",
            BeatError::TooManyOnsets => "a euclidean rhythm cannot have more onsets than steps",
            BeatError::InvalidPhase => "a bar phase must be finite",
            BeatError::InvalidTempo => "tempo and beats per bar must be finite and positive",
        };
        f.write_str(message)
    }
}

impl std::error::Error for BeatError {}
