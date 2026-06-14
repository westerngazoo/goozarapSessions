//! The crate's single typed error.

use std::fmt;

/// Every fallible operation in this crate reports one of these.
///
/// The crate never panics on library paths; arithmetic that cannot be
/// completed exactly, or input that is not a valid frequency, surfaces here.
///
/// ```
/// use gooz_ratio::{Ratio, RatioError};
///
/// assert_eq!(Ratio::new(1, 0), Err(RatioError::ZeroComponent));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatioError {
    /// A ratio was constructed with a zero numerator or denominator.
    ZeroComponent,
    /// Exact integer arithmetic exceeded the `u64` representation.
    Overflow,
    /// A frequency was non-finite or non-positive.
    InvalidFrequency,
    /// A pitch grid was constructed with no degrees.
    EmptyGrid,
}

impl fmt::Display for RatioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            RatioError::ZeroComponent => "ratio component must be non-zero",
            RatioError::Overflow => "exact arithmetic overflowed the u64 representation",
            RatioError::InvalidFrequency => "frequency must be finite and positive",
            RatioError::EmptyGrid => "a pitch grid must have at least one degree",
        };
        f.write_str(message)
    }
}

impl std::error::Error for RatioError {}
