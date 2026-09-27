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

    /// The take as one channel: each frame's channels averaged.
    ///
    /// Everything downstream of a recording — pitch, onsets, tempo, and R-0042's
    /// voice, which is played back as recorded — reads one channel. Handing it
    /// interleaved stereo (`L R L R …`) instead reads twice as many samples as
    /// there are frames: the take analyses and plays **at half speed, an octave
    /// low**, and its followed tempo comes out halved. Most USB microphones and
    /// audio interfaces default to two channels.
    ///
    /// A trailing partial frame is dropped. A mono take comes back unchanged.
    ///
    /// ```
    /// use gooz_audio::Take;
    ///
    /// let stereo = Take::new(vec![0.2, 0.4, -0.6, -0.2], 48_000, 2);
    /// assert_eq!(stereo.mono(), vec![0.3, -0.4]);
    ///
    /// let mono = Take::new(vec![0.1, 0.2, 0.3], 48_000, 1);
    /// assert_eq!(mono.mono(), vec![0.1, 0.2, 0.3]);
    /// ```
    pub fn mono(&self) -> Vec<f32> {
        match self.channels {
            0 => Vec::new(),
            1 => self.samples.clone(),
            n => self
                .samples
                .chunks_exact(n as usize)
                .map(|frame| frame.iter().sum::<f32>() / f32::from(n))
                .collect(),
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
