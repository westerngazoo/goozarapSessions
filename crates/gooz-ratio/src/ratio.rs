//! [`Ratio`] — an exact positive rational and the interval arithmetic on it.

use std::cmp::Ordering;
use std::fmt;

use crate::error::RatioError;

/// Greatest common divisor (binary-free Euclid), used to keep ratios reduced.
fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}

/// An exact, positive frequency ratio held in lowest terms.
///
/// A `Ratio` is the interval between two pitches as a small-integer fraction
/// from the harmonic series — `3:2` is a fifth, `2:1` an octave. Construction
/// reduces to lowest terms, so equal intervals compare, hash, and order
/// identically regardless of how they were spelled.
///
/// ```
/// use gooz_ratio::Ratio;
///
/// let fifth = Ratio::new(6, 4).unwrap(); // spelled un-reduced
/// assert_eq!(fifth, Ratio::new(3, 2).unwrap());
/// assert_eq!(fifth.to_string(), "3:2");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ratio {
    num: u64,
    den: u64,
}

impl Ratio {
    /// The unison, `1:1` — the identity interval.
    pub const UNISON: Ratio = Ratio { num: 1, den: 1 };
    /// The octave, `2:1`.
    pub const OCTAVE: Ratio = Ratio { num: 2, den: 1 };

    /// Builds a ratio `num:den`, reduced to lowest terms.
    ///
    /// Returns [`RatioError::ZeroComponent`] if either component is zero.
    ///
    /// ```
    /// use gooz_ratio::Ratio;
    ///
    /// let r = Ratio::new(4, 6).unwrap();
    /// assert_eq!((r.num(), r.den()), (2, 3));
    /// ```
    pub fn new(num: u64, den: u64) -> Result<Ratio, RatioError> {
        if num == 0 || den == 0 {
            return Err(RatioError::ZeroComponent);
        }
        let g = gcd(num, den);
        Ok(Ratio {
            num: num / g,
            den: den / g,
        })
    }

    /// The (reduced) numerator.
    ///
    /// ```
    /// # use gooz_ratio::Ratio;
    /// assert_eq!(Ratio::new(6, 4).unwrap().num(), 3);
    /// ```
    pub fn num(self) -> u64 {
        self.num
    }

    /// The (reduced) denominator.
    ///
    /// ```
    /// # use gooz_ratio::Ratio;
    /// assert_eq!(Ratio::new(6, 4).unwrap().den(), 2);
    /// ```
    pub fn den(self) -> u64 {
        self.den
    }

    /// Stacks two intervals — exact rational multiplication.
    ///
    /// Cross-cancels common factors before multiplying, so results that are
    /// themselves representable do not spuriously overflow. A genuine overflow
    /// surfaces as [`RatioError::Overflow`].
    ///
    /// ```
    /// use gooz_ratio::Ratio;
    ///
    /// let fifth = Ratio::new(3, 2).unwrap();
    /// let fourth = Ratio::new(4, 3).unwrap();
    /// assert_eq!(fifth.stack(fourth).unwrap(), Ratio::OCTAVE);
    /// ```
    pub fn stack(self, other: Ratio) -> Result<Ratio, RatioError> {
        let ga = gcd(self.num, other.den);
        let gb = gcd(other.num, self.den);
        let num = (self.num / ga)
            .checked_mul(other.num / gb)
            .ok_or(RatioError::Overflow)?;
        let den = (self.den / gb)
            .checked_mul(other.den / ga)
            .ok_or(RatioError::Overflow)?;
        Ok(Ratio { num, den })
    }

    /// Unstacks `other` from `self` — exact rational division (`self ÷ other`).
    ///
    /// ```
    /// use gooz_ratio::Ratio;
    ///
    /// let fourth = Ratio::new(4, 3).unwrap();
    /// assert_eq!(Ratio::OCTAVE.unstack(fourth).unwrap(), Ratio::new(3, 2).unwrap());
    /// ```
    pub fn unstack(self, other: Ratio) -> Result<Ratio, RatioError> {
        self.stack(other.invert())
    }

    /// Inverts the interval (`num:den` becomes `den:num`).
    ///
    /// Cannot fail: a reduced ratio stays reduced when its components swap.
    ///
    /// ```
    /// use gooz_ratio::Ratio;
    ///
    /// let fifth = Ratio::new(3, 2).unwrap();
    /// assert_eq!(fifth.invert(), Ratio::new(2, 3).unwrap());
    /// assert_eq!(fifth.invert().invert(), fifth);
    /// ```
    pub fn invert(self) -> Ratio {
        Ratio {
            num: self.den,
            den: self.num,
        }
    }

    /// Folds the ratio into the unit octave `[1, 2)` (octave equivalence).
    ///
    /// Uses canonical-preserving octave steps, so the result stays in lowest
    /// terms without re-reduction and is idempotent. A ratio whose
    /// octave-reduced form is not `u64`-representable surfaces
    /// [`RatioError::Overflow`].
    ///
    /// ```
    /// use gooz_ratio::Ratio;
    ///
    /// // 6:1 is two octaves above a fifth; it folds back onto 3:2.
    /// assert_eq!(Ratio::new(6, 1).unwrap().reduce_to_octave().unwrap(),
    ///            Ratio::new(3, 2).unwrap());
    /// ```
    pub fn reduce_to_octave(self) -> Result<Ratio, RatioError> {
        let mut num = self.num;
        let mut den = self.den;
        // value >= 2  <=>  floor(num / 2) >= den  (overflow-free test)
        while num / 2 >= den {
            if num.is_multiple_of(2) {
                num /= 2;
            } else {
                den = den.checked_mul(2).ok_or(RatioError::Overflow)?;
            }
        }
        // value < 1  <=>  num < den
        while num < den {
            if den.is_multiple_of(2) {
                den /= 2;
            } else {
                num = num.checked_mul(2).ok_or(RatioError::Overflow)?;
            }
        }
        Ok(Ratio { num, den })
    }

    /// Consonance metric — the Tenney height `log₂(num·den)`.
    ///
    /// Lower is simpler/smoother; this is what a "smooth↔tense" control walks.
    /// It is a metric for ordering and display, not an exact quantity.
    ///
    /// ```
    /// use gooz_ratio::Ratio;
    ///
    /// assert_eq!(Ratio::UNISON.complexity(), 0.0);
    /// assert!(Ratio::new(3, 2).unwrap().complexity()
    ///         < Ratio::new(9, 8).unwrap().complexity());
    /// ```
    pub fn complexity(self) -> f64 {
        (self.num as f64 * self.den as f64).log2()
    }

    /// The interval size in cents (1200 cents to the octave).
    ///
    /// ```
    /// use gooz_ratio::Ratio;
    ///
    /// assert_eq!(Ratio::OCTAVE.cents(), 1200.0);
    /// assert!((Ratio::new(3, 2).unwrap().cents() - 701.955).abs() < 0.001);
    /// ```
    pub fn cents(self) -> f64 {
        1200.0 * (self.num as f64 / self.den as f64).log2()
    }

    /// Maps the ratio to a concrete frequency above `root_hz`.
    ///
    /// Computed as `root_hz * (num / den)`; this exact formula is shared with
    /// [`crate::PitchGrid::snap`] so grid pitches are bitwise fixed points.
    /// Rejects a non-finite or non-positive `root_hz` with
    /// [`RatioError::InvalidFrequency`].
    ///
    /// ```
    /// use gooz_ratio::Ratio;
    ///
    /// assert_eq!(Ratio::new(3, 2).unwrap().to_hz(220.0).unwrap(), 330.0);
    /// ```
    pub fn to_hz(self, root_hz: f64) -> Result<f64, RatioError> {
        if !root_hz.is_finite() || root_hz <= 0.0 {
            return Err(RatioError::InvalidFrequency);
        }
        Ok(root_hz * (self.num as f64 / self.den as f64))
    }
}

impl fmt::Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.num, self.den)
    }
}

impl Ord for Ratio {
    /// Orders by musical size via cross-multiplication in `u128`, which cannot
    /// overflow for `u64` components and agrees with structural equality.
    fn cmp(&self, other: &Self) -> Ordering {
        (self.num as u128 * other.den as u128).cmp(&(other.num as u128 * self.den as u128))
    }
}

impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
