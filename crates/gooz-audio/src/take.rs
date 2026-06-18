//! [`Take`] — a captured block of audio plus its format.

/// Interleaved `f32` samples captured from (or destined for) the engine, with
/// the sample rate and channel count needed to interpret them.
///
/// ```
/// use gooz_audio::Take;
///
/// let take = Take::new(vec![0.0; 48_000], 48_000, 1);
/// assert_eq!(take.frames(), 48_000);
/// assert_eq!(take.duration_secs(), 1.0);
/// ```
#[derive(Debug, Clone)]
pub struct Take {
    samples: Vec<f32>,
    sample_rate: u32,
    channels: u16,
}

impl Take {
    /// Builds a take from interleaved samples and its format. A take minted by
    /// the engine always carries the backend's `channels >= 1`.
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Take {
        Take {
            samples,
            sample_rate,
            channels,
        }
    }

    /// The interleaved samples.
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// The sample rate in Hz.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// The channel count.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// The number of frames (samples per channel). The `channels == 0` guard
    /// returns 0 and is pure defensiveness — engine-minted takes have
    /// `channels >= 1`.
    ///
    /// ```
    /// # use gooz_audio::Take;
    /// assert_eq!(Take::new(vec![0.0; 6], 48_000, 2).frames(), 3);
    /// ```
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / self.channels as usize
        }
    }

    /// The duration in seconds (`frames / sample_rate`).
    ///
    /// ```
    /// # use gooz_audio::Take;
    /// assert_eq!(Take::new(vec![0.0; 24_000], 48_000, 1).duration_secs(), 0.5);
    /// ```
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.frames() as f64 / self.sample_rate as f64
        }
    }

    /// Whether the take has no samples.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}
