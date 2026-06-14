//! Pure music-math core — the foundation of "no music knowledge needed".
//!
//! Pitch relationships are small-integer frequency ratios from the harmonic
//! series ([`Ratio`]): `2:1` octave, `3:2` fifth, `5:4` major third, … Their
//! [consonance][Ratio::complexity] orders the "smooth↔tense" control the UI
//! exposes instead of note names. A [`PitchGrid`] roots a set of those ratios
//! at a frequency and [snaps][PitchGrid::snap] an arbitrary pitch (e.g. a
//! tracked vocal) onto the nearest degree in the correct octave.
//!
//! Bounded responsibility: math only. No I/O, no audio, no allocation on the
//! snapping hot path, and no dependencies on any other workspace crate.
//! Realizes R-0001 / SPEC-0001; rhythm and beat ratios are R-0002.
//!
//! ```
//! use gooz_ratio::{PitchGrid, Ratio};
//!
//! // A fifth stacked on a fourth is an octave — exactly.
//! let fifth = Ratio::new(3, 2)?;
//! let fourth = Ratio::new(4, 3)?;
//! assert_eq!(fifth.stack(fourth)?, Ratio::OCTAVE);
//!
//! // Snap a slightly-sharp A onto a harmonic grid rooted at 220 Hz.
//! let grid = PitchGrid::harmonic(220.0, 9)?;
//! let snapped = grid.snap(445.0)?;
//! assert_eq!(snapped.degree, Ratio::UNISON);
//! assert_eq!(snapped.octave, 1);
//! assert_eq!(snapped.hz, 440.0);
//! # Ok::<(), gooz_ratio::RatioError>(())
//! ```

mod error;
mod grid;
mod ratio;

pub use error::RatioError;
pub use grid::{PitchGrid, SnappedPitch};
pub use ratio::Ratio;
