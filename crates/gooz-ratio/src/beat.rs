//! Bar grids, quantization, polyrhythm, and tempo — positions within a bar.

use crate::beat_error::BeatError;
use crate::math;

/// The even pulse positions `0/count, 1/count, …` as reduced bar fractions.
fn pulse_fractions(count: u32) -> Vec<(u64, u64)> {
    let n = u64::from(count);
    (0..n)
        .map(|i| {
            let g = math::gcd(i, n);
            (i / g, n / g)
        })
        .collect()
}

/// A bar divided into `steps` equal subdivisions, with the downbeat at `0`.
///
/// ```
/// use gooz_ratio::BarGrid;
///
/// let bar = BarGrid::new(8).unwrap();
/// assert_eq!(bar.position(2), (1, 4)); // step 2 of 8 is a quarter of the bar
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BarGrid {
    steps: u32,
}

/// The result of [`BarGrid::quantize`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantizedBeat {
    /// The grid step the phase snapped to, in `0..steps`.
    pub step: u32,
    /// That step's phase (`step / steps`), a fraction of the bar in `[0, 1)`.
    pub phase: f64,
    /// The input relative to the snapped target (`input − snapped`), signed,
    /// in fractions of a bar.
    pub offset: f64,
}

impl BarGrid {
    /// Builds a grid of `steps` equal subdivisions; `0` → [`BeatError::EmptyGrid`].
    ///
    /// ```
    /// # use gooz_ratio::{BarGrid, BeatError};
    /// assert!(BarGrid::new(16).is_ok());
    /// assert_eq!(BarGrid::new(0), Err(BeatError::EmptyGrid));
    /// ```
    pub fn new(steps: u32) -> Result<BarGrid, BeatError> {
        if steps == 0 {
            return Err(BeatError::EmptyGrid);
        }
        Ok(BarGrid { steps })
    }

    /// The number of steps in the bar.
    ///
    /// ```
    /// # use gooz_ratio::BarGrid;
    /// assert_eq!(BarGrid::new(8).unwrap().steps(), 8);
    /// ```
    pub fn steps(&self) -> u32 {
        self.steps
    }

    /// Step `index` as an exact reduced fraction of the bar; the downbeat is
    /// `(0, 1)`. `index` wraps modulo `steps`.
    ///
    /// ```
    /// # use gooz_ratio::BarGrid;
    /// let bar = BarGrid::new(8).unwrap();
    /// assert_eq!(bar.position(0), (0, 1));
    /// assert_eq!(bar.position(3), (3, 8));
    /// ```
    pub fn position(&self, index: u32) -> (u64, u64) {
        let i = u64::from(index % self.steps);
        let n = u64::from(self.steps);
        let g = math::gcd(i, n);
        (i / g, n / g)
    }

    /// Step `index` as a floating-point phase in `[0, 1)`; `index` wraps modulo
    /// `steps`.
    ///
    /// ```
    /// # use gooz_ratio::BarGrid;
    /// assert_eq!(BarGrid::new(4).unwrap().phase(1), 0.25);
    /// ```
    pub fn phase(&self, index: u32) -> f64 {
        f64::from(index % self.steps) / f64::from(self.steps)
    }

    /// Snaps an arbitrary bar phase to the nearest grid step.
    ///
    /// Ties resolve to the earlier step; a phase just below the barline wraps to
    /// step `0`. The snapped step's own phase is a fixed point, so quantizing is
    /// idempotent. A non-finite phase → [`BeatError::InvalidPhase`]; any finite
    /// phase outside `[0, 1)` is still accepted with defined behavior.
    ///
    /// ```
    /// # use gooz_ratio::BarGrid;
    /// let bar = BarGrid::new(8).unwrap();
    /// let q = bar.quantize(0.30).unwrap(); // nearest to step 2 (0.25)
    /// assert_eq!(q.step, 2);
    /// assert!(q.offset > 0.0); // 0.30 sits just after the step
    /// ```
    pub fn quantize(&self, phase: f64) -> Result<QuantizedBeat, BeatError> {
        if !phase.is_finite() {
            return Err(BeatError::InvalidPhase);
        }
        let n = f64::from(self.steps);
        // Nearest step, ties to the earlier step (f64::round ties away from
        // zero — the wrong direction here).
        let nearest = (phase * n - 0.5).ceil() as i64;
        let target = nearest as f64 / n;
        let step = nearest.rem_euclid(i64::from(self.steps)) as u32;
        Ok(QuantizedBeat {
            step,
            phase: f64::from(step) / n,
            offset: phase - target,
        })
    }
}

/// Two pulse streams played against each other on their shared grid (e.g. 3:2).
///
/// ```
/// use gooz_ratio::Polyrhythm;
///
/// let three_two = Polyrhythm::new(3, 2).unwrap();
/// assert_eq!(three_two.grid_steps(), 6); // lcm(3, 2)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Polyrhythm {
    a: u32,
    b: u32,
}

impl Polyrhythm {
    /// Builds an `a`-against-`b` polyrhythm; a zero pulse count →
    /// [`BeatError::EmptyGrid`].
    ///
    /// ```
    /// # use gooz_ratio::{Polyrhythm, BeatError};
    /// assert!(Polyrhythm::new(4, 3).is_ok());
    /// assert_eq!(Polyrhythm::new(3, 0), Err(BeatError::EmptyGrid));
    /// ```
    pub fn new(a: u32, b: u32) -> Result<Polyrhythm, BeatError> {
        if a == 0 || b == 0 {
            return Err(BeatError::EmptyGrid);
        }
        Ok(Polyrhythm { a, b })
    }

    /// The shared grid both streams align on: `lcm(a, b)`.
    ///
    /// ```
    /// # use gooz_ratio::Polyrhythm;
    /// assert_eq!(Polyrhythm::new(4, 3).unwrap().grid_steps(), 12);
    /// ```
    pub fn grid_steps(&self) -> u64 {
        // Both are u32, so the lcm always fits in u64 — the None branch is
        // unreachable here.
        math::lcm(u64::from(self.a), u64::from(self.b))
            .expect("lcm of two u32 values always fits in u64")
    }

    /// The `a`-stream pulses as reduced bar fractions `i/a` for `i` in `0..a`.
    ///
    /// ```
    /// # use gooz_ratio::Polyrhythm;
    /// assert_eq!(Polyrhythm::new(3, 2).unwrap().a_pulses(), vec![(0, 1), (1, 3), (2, 3)]);
    /// ```
    pub fn a_pulses(&self) -> Vec<(u64, u64)> {
        pulse_fractions(self.a)
    }

    /// The `b`-stream pulses as reduced bar fractions `i/b` for `i` in `0..b`.
    ///
    /// ```
    /// # use gooz_ratio::Polyrhythm;
    /// assert_eq!(Polyrhythm::new(3, 2).unwrap().b_pulses(), vec![(0, 1), (1, 2)]);
    /// ```
    pub fn b_pulses(&self) -> Vec<(u64, u64)> {
        pulse_fractions(self.b)
    }
}

/// A tempo: beats per minute and beats per bar, converting bar phases to time.
///
/// ```
/// use gooz_ratio::Tempo;
///
/// let tempo = Tempo::new(120.0, 4.0).unwrap();
/// assert_eq!(tempo.seconds_per_beat(), 0.5);
/// assert_eq!(tempo.bar_seconds(), 2.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tempo {
    bpm: f64,
    beats_per_bar: f64,
}

impl Tempo {
    /// Builds a tempo; a non-finite or non-positive `bpm` or `beats_per_bar` →
    /// [`BeatError::InvalidTempo`].
    ///
    /// ```
    /// # use gooz_ratio::{Tempo, BeatError};
    /// assert!(Tempo::new(92.0, 4.0).is_ok());
    /// assert_eq!(Tempo::new(0.0, 4.0), Err(BeatError::InvalidTempo));
    /// ```
    pub fn new(bpm: f64, beats_per_bar: f64) -> Result<Tempo, BeatError> {
        if !bpm.is_finite() || bpm <= 0.0 || !beats_per_bar.is_finite() || beats_per_bar <= 0.0 {
            return Err(BeatError::InvalidTempo);
        }
        Ok(Tempo { bpm, beats_per_bar })
    }

    /// Seconds per beat: `60 / bpm`.
    ///
    /// ```
    /// # use gooz_ratio::Tempo;
    /// assert_eq!(Tempo::new(120.0, 4.0).unwrap().seconds_per_beat(), 0.5);
    /// ```
    pub fn seconds_per_beat(&self) -> f64 {
        60.0 / self.bpm
    }

    /// Seconds per bar: `beats_per_bar * 60 / bpm`.
    ///
    /// ```
    /// # use gooz_ratio::Tempo;
    /// assert_eq!(Tempo::new(120.0, 3.0).unwrap().bar_seconds(), 1.5);
    /// ```
    pub fn bar_seconds(&self) -> f64 {
        self.beats_per_bar * 60.0 / self.bpm
    }

    /// The wall-clock time of a bar phase: `phase * bar_seconds()`.
    ///
    /// ```
    /// # use gooz_ratio::Tempo;
    /// assert_eq!(Tempo::new(120.0, 4.0).unwrap().step_time(0.5), 1.0);
    /// ```
    pub fn step_time(&self, phase: f64) -> f64 {
        phase * self.bar_seconds()
    }
}
