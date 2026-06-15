//! [`Pattern`] — Euclidean rhythms and the operations on them.

use crate::beat_error::BeatError;

/// A step pattern: a fixed-length sequence of onsets (`true`) and rests.
///
/// Built by [`Pattern::euclidean`], the maximally-even distribution behind
/// most grooves — the "sparse↔busy" control walks its onset count.
///
/// ```
/// use gooz_ratio::Pattern;
///
/// let tresillo = Pattern::euclidean(3, 8).unwrap();
/// assert_eq!(tresillo.onsets(), vec![0, 3, 6]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    steps: Vec<bool>,
}

impl Pattern {
    /// Builds the Euclidean rhythm `E(onsets, steps)` via Bjorklund's
    /// algorithm: `onsets` onsets spread as evenly as possible over `steps`.
    ///
    /// `steps == 0` → [`BeatError::EmptyGrid`]; `onsets > steps` →
    /// [`BeatError::TooManyOnsets`]; `onsets == 0` is an all-rest pattern;
    /// `onsets == steps` is all onsets. The first step is an onset whenever
    /// `onsets > 0`.
    ///
    /// ```
    /// use gooz_ratio::Pattern;
    ///
    /// let cinquillo = Pattern::euclidean(5, 8).unwrap();
    /// assert_eq!(cinquillo.onsets(), vec![0, 2, 3, 5, 6]);
    /// ```
    pub fn euclidean(onsets: u32, steps: u32) -> Result<Pattern, BeatError> {
        if steps == 0 {
            return Err(BeatError::EmptyGrid);
        }
        if onsets > steps {
            return Err(BeatError::TooManyOnsets);
        }
        if onsets == 0 {
            // Short-circuit: the Bjorklund loop makes no progress with an empty
            // onset pile, so it must never be entered for `onsets == 0`.
            return Ok(Pattern {
                steps: vec![false; steps as usize],
            });
        }

        let mut filled: Vec<Vec<bool>> = (0..onsets).map(|_| vec![true]).collect();
        let mut remainder: Vec<Vec<bool>> = (onsets..steps).map(|_| vec![false]).collect();
        while remainder.len() > 1 {
            let pairs = filled.len().min(remainder.len());
            let mut next: Vec<Vec<bool>> = Vec::with_capacity(pairs);
            for (head, tail) in filled.iter().zip(&remainder) {
                let mut group = head.clone();
                group.extend_from_slice(tail);
                next.push(group);
            }
            let leftover = if filled.len() > pairs {
                filled[pairs..].to_vec()
            } else {
                remainder[pairs..].to_vec()
            };
            filled = next;
            remainder = leftover;
        }

        let steps = filled.into_iter().chain(remainder).flatten().collect();
        Ok(Pattern { steps })
    }

    /// The number of steps in the pattern.
    ///
    /// ```
    /// # use gooz_ratio::Pattern;
    /// assert_eq!(Pattern::euclidean(3, 8).unwrap().len(), 8);
    /// ```
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the pattern has no steps. Never true for a pattern built by
    /// [`Pattern::euclidean`], which always has `steps >= 1`.
    ///
    /// ```
    /// # use gooz_ratio::Pattern;
    /// assert!(!Pattern::euclidean(1, 4).unwrap().is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// How many steps are onsets.
    ///
    /// ```
    /// # use gooz_ratio::Pattern;
    /// assert_eq!(Pattern::euclidean(3, 8).unwrap().onset_count(), 3);
    /// ```
    pub fn onset_count(&self) -> usize {
        self.steps.iter().filter(|&&onset| onset).count()
    }

    /// Whether the step at `index` is an onset; `false` for out-of-range.
    ///
    /// ```
    /// # use gooz_ratio::Pattern;
    /// let p = Pattern::euclidean(3, 8).unwrap();
    /// assert!(p.is_onset(0));
    /// assert!(!p.is_onset(1));
    /// ```
    pub fn is_onset(&self, index: usize) -> bool {
        self.steps.get(index).copied().unwrap_or(false)
    }

    /// The raw step slice, `true` = onset.
    ///
    /// ```
    /// # use gooz_ratio::Pattern;
    /// assert_eq!(Pattern::euclidean(2, 4).unwrap().steps(), &[true, false, true, false]);
    /// ```
    pub fn steps(&self) -> &[bool] {
        &self.steps
    }

    /// The ascending indices of the onsets.
    ///
    /// ```
    /// # use gooz_ratio::Pattern;
    /// assert_eq!(Pattern::euclidean(4, 16).unwrap().onsets(), vec![0, 4, 8, 12]);
    /// ```
    pub fn onsets(&self) -> Vec<usize> {
        self.steps
            .iter()
            .enumerate()
            .filter_map(|(index, &onset)| onset.then_some(index))
            .collect()
    }

    /// Cyclically rotates the pattern: step `s` moves to `(s + by) mod len`.
    ///
    /// `by` is taken modulo the length, so a whole multiple of the length (or
    /// `0`) is the identity and a negative offset rotates the other way.
    /// Length and onset count are preserved.
    ///
    /// ```
    /// # use gooz_ratio::Pattern;
    /// let p = Pattern::euclidean(3, 8).unwrap();        // onsets {0, 3, 6}
    /// assert_eq!(p.rotate(1).onsets(), vec![1, 4, 7]);  // shifted by +1
    /// assert_eq!(p.rotate(8), p);                       // full turn = identity
    /// ```
    pub fn rotate(&self, by: i64) -> Pattern {
        let len = self.steps.len();
        if len == 0 {
            return Pattern { steps: Vec::new() };
        }
        let shift = by.rem_euclid(len as i64) as usize;
        let mut steps = self.steps.clone();
        steps.rotate_right(shift);
        Pattern { steps }
    }
}
