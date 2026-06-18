//! [`Transport`] — the ratio-locked clock: sample-accurate beat/subdivision
//! boundaries derived from a [`gooz_ratio::Tempo`].

use gooz_ratio::Tempo;

/// What a metronome boundary should sound like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickKind {
    /// The first beat of a bar (downbeat) — the accent.
    Accent,
    /// A non-downbeat beat.
    Beat,
    /// A between-beat subdivision.
    Subdivision,
}

/// A sample-accurate beat/subdivision clock.
///
/// Boundaries are computed in **frames** (a frame is one sample per channel; on
/// mono output a frame is a sample) from the index *absolutely*, so they never
/// drift over a long run.
///
/// ```
/// use gooz_audio::{ClickKind, Transport};
/// use gooz_ratio::Tempo;
///
/// let tempo = Tempo::new(120.0, 4.0).unwrap();
/// let transport = Transport::new(48_000, &tempo, 2); // 12000 frames per subdivision
/// assert_eq!(transport.boundary_frame(1), 12_000);
/// assert_eq!(transport.click_kind(0), ClickKind::Accent);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Transport {
    frames_per_sub: f64,
    subdivision: u32,
    beats_per_bar: u32,
}

impl Transport {
    /// Builds a transport from a sample rate, a [`Tempo`], and a beat
    /// subdivision (`1` = beats only, `2` = eighths, …). `subdivision` is
    /// clamped to at least 1.
    pub fn new(sample_rate: u32, tempo: &Tempo, subdivision: u32) -> Transport {
        let subdivision = subdivision.max(1);
        let frames_per_sub =
            f64::from(sample_rate) * tempo.seconds_per_beat() / f64::from(subdivision);
        let beats_per_bar = (tempo.beats_per_bar().round() as i64).max(1) as u32;
        Transport {
            frames_per_sub,
            subdivision,
            beats_per_bar,
        }
    }

    /// The frame at which boundary `index` falls: `round(index · frames_per_sub)`,
    /// computed absolutely (no cumulative drift).
    ///
    /// ```
    /// # use gooz_audio::Transport;
    /// # use gooz_ratio::Tempo;
    /// let transport = Transport::new(48_000, &Tempo::new(120.0, 4.0).unwrap(), 2);
    /// assert_eq!(transport.boundary_frame(0), 0);
    /// assert_eq!(transport.boundary_frame(3), 36_000);
    /// ```
    pub fn boundary_frame(&self, index: u64) -> u64 {
        (index as f64 * self.frames_per_sub).round() as u64
    }

    /// How boundary `index` should sound: an `Accent` on a bar downbeat, a
    /// `Beat` on other beats, a `Subdivision` between beats.
    ///
    /// ```
    /// # use gooz_audio::{ClickKind, Transport};
    /// # use gooz_ratio::Tempo;
    /// let transport = Transport::new(48_000, &Tempo::new(120.0, 4.0).unwrap(), 2);
    /// assert_eq!(transport.click_kind(0), ClickKind::Accent);
    /// assert_eq!(transport.click_kind(1), ClickKind::Subdivision);
    /// assert_eq!(transport.click_kind(2), ClickKind::Beat);
    /// ```
    pub fn click_kind(&self, index: u64) -> ClickKind {
        if !index.is_multiple_of(u64::from(self.subdivision)) {
            return ClickKind::Subdivision;
        }
        let beat = index / u64::from(self.subdivision);
        if beat.is_multiple_of(u64::from(self.beats_per_bar)) {
            ClickKind::Accent
        } else {
            ClickKind::Beat
        }
    }

    /// The beat subdivision (clamped to ≥ 1).
    pub fn subdivision(&self) -> u32 {
        self.subdivision
    }

    /// The beats per bar (rounded to an integer, ≥ 1).
    pub fn beats_per_bar(&self) -> u32 {
        self.beats_per_bar
    }

    /// The number of frames between consecutive subdivision boundaries.
    pub fn frames_per_sub(&self) -> f64 {
        self.frames_per_sub
    }
}
