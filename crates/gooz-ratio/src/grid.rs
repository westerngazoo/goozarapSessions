//! [`PitchGrid`] — a rooted set of octave-reduced degrees, and snapping onto it.

use crate::error::RatioError;
use crate::ratio::Ratio;

/// The result of snapping a frequency onto a [`PitchGrid`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SnappedPitch {
    /// The grid degree the frequency snapped to (octave-reduced, in `[1, 2)`).
    pub degree: Ratio,
    /// Whole octaves above the root: `0` ⇔ snapped Hz ∈ `[root, 2·root)`.
    pub octave: i32,
    /// The snapped frequency in Hz, recomputed from `(degree, octave)`.
    pub hz: f64,
    /// The input relative to the snapped pitch (`input − snapped`), in cents.
    pub cents_offset: f64,
}

/// A tuning: a root frequency plus the octave-reduced degrees pitches snap to.
///
/// Degrees are held sorted ascending, deduplicated, and always include
/// [`Ratio::UNISON`] so the root is a valid target.
///
/// ```
/// use gooz_ratio::{PitchGrid, Ratio};
///
/// let grid = PitchGrid::harmonic(220.0, 9).unwrap();
/// assert_eq!(grid.degrees().first(), Some(&Ratio::UNISON));
/// ```
#[derive(Debug, Clone)]
pub struct PitchGrid {
    root_hz: f64,
    degrees: Vec<Ratio>,
}

impl PitchGrid {
    /// Builds a grid from arbitrary ratios: each is octave-reduced, then the
    /// set is deduplicated, sorted ascending, and seeded with `1:1`.
    ///
    /// Rejects a non-finite/non-positive `root_hz`
    /// ([`RatioError::InvalidFrequency`]) or an empty input
    /// ([`RatioError::EmptyGrid`]).
    ///
    /// ```
    /// use gooz_ratio::{PitchGrid, Ratio};
    ///
    /// let grid = PitchGrid::from_ratios(220.0, [Ratio::new(3, 2).unwrap()]).unwrap();
    /// assert_eq!(grid.degrees(), &[Ratio::UNISON, Ratio::new(3, 2).unwrap()]);
    /// ```
    pub fn from_ratios<I>(root_hz: f64, ratios: I) -> Result<PitchGrid, RatioError>
    where
        I: IntoIterator<Item = Ratio>,
    {
        if !root_hz.is_finite() || root_hz <= 0.0 {
            return Err(RatioError::InvalidFrequency);
        }
        let mut degrees = vec![Ratio::UNISON];
        let mut any = false;
        for ratio in ratios {
            any = true;
            degrees.push(ratio.reduce_to_octave()?);
        }
        if !any {
            return Err(RatioError::EmptyGrid);
        }
        degrees.sort();
        degrees.dedup();
        Ok(PitchGrid { root_hz, degrees })
    }

    /// Builds a grid from the harmonic series: the octave-reduced odd harmonics
    /// `1, 3, 5, … ≤ odd_limit`. An even `odd_limit` bounds the same odd set.
    ///
    /// Rejects `odd_limit == 0` ([`RatioError::EmptyGrid`]) and an invalid
    /// root ([`RatioError::InvalidFrequency`]).
    ///
    /// ```
    /// use gooz_ratio::{PitchGrid, Ratio};
    ///
    /// let grid = PitchGrid::harmonic(220.0, 9).unwrap();
    /// assert_eq!(
    ///     grid.degrees(),
    ///     &[
    ///         Ratio::UNISON,
    ///         Ratio::new(9, 8).unwrap(),
    ///         Ratio::new(5, 4).unwrap(),
    ///         Ratio::new(3, 2).unwrap(),
    ///         Ratio::new(7, 4).unwrap(),
    ///     ],
    /// );
    /// ```
    pub fn harmonic(root_hz: f64, odd_limit: u64) -> Result<PitchGrid, RatioError> {
        let mut harmonics = Vec::new();
        for h in (1..=odd_limit).step_by(2) {
            harmonics.push(Ratio::new(h, 1)?);
        }
        PitchGrid::from_ratios(root_hz, harmonics)
    }

    /// The root frequency in Hz.
    ///
    /// ```
    /// # use gooz_ratio::PitchGrid;
    /// assert_eq!(PitchGrid::harmonic(220.0, 9).unwrap().root_hz(), 220.0);
    /// ```
    pub fn root_hz(&self) -> f64 {
        self.root_hz
    }

    /// The grid degrees, sorted ascending and starting at `1:1`.
    ///
    /// ```
    /// # use gooz_ratio::{PitchGrid, Ratio};
    /// let grid = PitchGrid::harmonic(220.0, 1).unwrap();
    /// assert_eq!(grid.degrees(), &[Ratio::UNISON]);
    /// ```
    pub fn degrees(&self) -> &[Ratio] {
        &self.degrees
    }

    /// Snaps an arbitrary frequency onto the nearest grid pitch, in the
    /// correct octave.
    ///
    /// Works in log-frequency, where octaves are unit steps. For each degree it
    /// considers the previous, current, and next octave, choosing the smallest
    /// distance; exact ties resolve to the lower-pitched candidate. The snapped
    /// Hz is recomputed from `(degree, octave)`, so on-grid frequencies are
    /// bitwise fixed points and snapping is idempotent.
    ///
    /// Rejects a non-finite/non-positive input, or one whose ratio to the root
    /// is not finite, with [`RatioError::InvalidFrequency`].
    ///
    /// ```
    /// use gooz_ratio::{PitchGrid, Ratio};
    ///
    /// let grid = PitchGrid::harmonic(220.0, 9).unwrap();
    /// // 660 Hz is a fifth, one octave above the root.
    /// let snapped = grid.snap(660.0).unwrap();
    /// assert_eq!(snapped.degree, Ratio::new(3, 2).unwrap());
    /// assert_eq!(snapped.octave, 1);
    /// assert_eq!(snapped.hz, 660.0);
    /// ```
    pub fn snap(&self, hz: f64) -> Result<SnappedPitch, RatioError> {
        if !hz.is_finite() || hz <= 0.0 {
            return Err(RatioError::InvalidFrequency);
        }
        let t = (hz / self.root_hz).log2();
        if !t.is_finite() {
            return Err(RatioError::InvalidFrequency);
        }
        let octave_floor = t.floor();
        let frac = t - octave_floor;

        let mut best_degree = Ratio::UNISON;
        let mut best_octave_adjust = 0i32;
        let mut best_distance = f64::INFINITY;
        for &degree in &self.degrees {
            let position = (degree.num() as f64 / degree.den() as f64).log2();
            for adjust in [-1.0, 0.0, 1.0] {
                let distance = (frac - (position + adjust)).abs();
                if distance < best_distance {
                    best_distance = distance;
                    best_degree = degree;
                    best_octave_adjust = adjust as i32;
                }
            }
        }

        let octave = octave_floor as i32 + best_octave_adjust;
        // Reuse the one pinned ratio→Hz formula so on-grid pitches stay bitwise
        // fixed points; the root is already validated, so this cannot fail.
        let snapped_hz = best_degree.to_hz(self.root_hz)? * 2f64.powi(octave);
        let cents_offset = 1200.0 * (hz / snapped_hz).log2();

        Ok(SnappedPitch {
            degree: best_degree,
            octave,
            hz: snapped_hz,
            cents_offset,
        })
    }
}
