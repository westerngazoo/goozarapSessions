//! Pure music-math core — the foundation of "no music knowledge needed".
//!
//! Pitch relationships are small-integer frequency ratios from the harmonic
//! series ([`Ratio`]): `2:1` octave, `3:2` fifth, `5:4` major third, … Their
//! [consonance][Ratio::complexity] orders the "smooth↔tense" control the UI
//! exposes instead of note names. A [`PitchGrid`] roots a set of those ratios
//! at a frequency and [snaps][PitchGrid::snap] an arbitrary pitch (e.g. a
//! tracked vocal) onto the nearest degree in the correct octave.
//!
//! Rhythm is the same idea in time: a [`Pattern`] is a Euclidean rhythm
//! `E(k, n)` (the "sparse↔busy" control), a [`BarGrid`] subdivides a bar and
//! [quantizes][BarGrid::quantize] onto it, [`Polyrhythm`] composes two pulse
//! streams, and [`Tempo`] maps bar phases to wall-clock time.
//!
//! Bounded responsibility: math only. No I/O, no audio, no allocation on the
//! snapping hot path, and no dependencies on any other workspace crate.
//! Realizes R-0001 / SPEC-0001 (pitch) and R-0002 / SPEC-0002 (rhythm).
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

mod beat;
mod beat_error;
mod error;
mod grid;
mod math;
mod ratio;
mod rhythm;

pub use beat::{BarGrid, Polyrhythm, QuantizedBeat, Tempo};
pub use beat_error::BeatError;
pub use error::RatioError;
pub use grid::{PitchGrid, SnappedPitch};
pub use ratio::Ratio;
pub use rhythm::Pattern;
