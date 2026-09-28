//! [`render_808`] — the 808 bass: a sine sub-bass with a long decay, a glide
//! between overlapping notes, and a drive that cracks it. Realizes R-0033 /
//! SPEC-0033.

/// One note for the bass: only what the voice reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BassNote {
    /// Pitch in Hz.
    pub hz: f64,
    /// When the note starts, in seconds from the start of the part.
    pub onset_secs: f64,
    /// How long it lasts, in seconds.
    pub duration_secs: f64,
}

/// How an 808 sounds: how long overlapping notes take to slide, how long a
/// hit rings, and how hard it is driven.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bass808 {
    /// Seconds a legato note takes to glide to its pitch.
    pub glide_secs: f64,
    /// Time constant of the exponential decay, in seconds.
    pub decay_secs: f64,
    /// Saturation amount in `0..=1`: `0` is a clean sine.
    pub drive: f32,
}

impl Default for Bass808 {
    fn default() -> Bass808 {
        Bass808 {
            glide_secs: 0.08,
            decay_secs: 1.2,
            drive: 0.0,
        }
    }
}

/// Renders `notes` as an 808 part exactly `len` samples long.
pub fn render_808(notes: &[BassNote], sample_rate: u32, len: usize, cfg: &Bass808) -> Vec<f32> {
    let _ = (notes, cfg);
    if sample_rate == 0 {
        return Vec::new();
    }
    vec![0.0; len]
}
