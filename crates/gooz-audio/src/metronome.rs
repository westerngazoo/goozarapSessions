//! [`Metronome`] — a real-time-safe render source that plays the transport's
//! click at each beat/subdivision boundary.

use gooz_ratio::Tempo;

use crate::transport::{ClickKind, Transport};

/// Tick length in seconds (~30 ms).
const TICK_SECS: f64 = 0.030;

/// Synthesizes a short percussive "tick": a decaying cosine
/// `amp · cos(2π f t) · (1 − i/n)`. Using cosine gives a sharp attack at the
/// onset (sample 0 is the peak `amp`, not a zero crossing). `n` is clamped to
/// ≥ 1 so the buffer is never empty.
fn tick(sample_rate: u32, freq: f64, amp: f32, secs: f64) -> Vec<f32> {
    let n = ((f64::from(sample_rate) * secs).round() as usize).max(1);
    (0..n)
        .map(|i| {
            let t = i as f64 / f64::from(sample_rate);
            let envelope = 1.0 - (i as f64 / n as f64);
            (amp as f64 * (std::f64::consts::TAU * freq * t).cos() * envelope) as f32
        })
        .collect()
}

/// Plays the metronome click for a [`Transport`], one preallocated tick per
/// boundary, accenting downbeats. Built to be driven from the audio callback:
/// [`Metronome::render`] only reads its preallocated tick buffers and writes the
/// output — no allocation, locking, or I/O.
///
/// ```
/// use gooz_audio::Metronome;
/// use gooz_ratio::Tempo;
///
/// let tempo = Tempo::new(120.0, 4.0).unwrap();
/// let mut metronome = Metronome::new(48_000, &tempo, 2, 1);
/// let mut block = vec![0.0f32; 256];
/// metronome.render(&mut block); // the downbeat accent begins at frame 0
/// assert!(block.iter().any(|&s| s != 0.0));
/// ```
pub struct Metronome {
    transport: Transport,
    channels: u16,
    accent: Vec<f32>,
    beat: Vec<f32>,
    sub: Vec<f32>,
    pos: u64,
    next_index: u64,
    active: Option<(ClickKind, usize)>,
}

impl Metronome {
    /// Builds a metronome for the given format and beat subdivision, synthesizing
    /// the three click voices up front (descending prominence: accent loudest,
    /// subdivision quietest). `channels` is clamped to at least 1.
    pub fn new(sample_rate: u32, tempo: &Tempo, subdivision: u32, channels: u16) -> Metronome {
        Metronome {
            transport: Transport::new(sample_rate, tempo, subdivision),
            channels: channels.max(1),
            accent: tick(sample_rate, 1000.0, 0.9, TICK_SECS),
            beat: tick(sample_rate, 800.0, 0.6, TICK_SECS),
            sub: tick(sample_rate, 800.0, 0.3, TICK_SECS),
            pos: 0,
            next_index: 0,
            active: None,
        }
    }

    /// The transport driving this metronome.
    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    fn voice(&self, kind: ClickKind) -> &[f32] {
        match kind {
            ClickKind::Accent => &self.accent,
            ClickKind::Beat => &self.beat,
            ClickKind::Subdivision => &self.sub,
        }
    }

    /// Fills `output` (interleaved by channel) with the click. Real-time safe:
    /// one frame at a time, firing every boundary at or before the current frame
    /// (so a degenerate config cannot stall) and writing each frame's value to
    /// every channel. Continues a click across calls, so rendering a span in one
    /// block equals rendering it in many frame-aligned blocks.
    pub fn render(&mut self, output: &mut [f32]) {
        for frame in output.chunks_mut(self.channels as usize) {
            while self.transport.boundary_frame(self.next_index) <= self.pos {
                self.active = Some((self.transport.click_kind(self.next_index), 0));
                self.next_index += 1;
            }
            let value = match self.active {
                Some((kind, cursor)) => {
                    let sample = self.voice(kind)[cursor];
                    let next = cursor + 1;
                    self.active = if next < self.voice(kind).len() {
                        Some((kind, next))
                    } else {
                        None
                    };
                    sample
                }
                None => 0.0,
            };
            for out in frame.iter_mut() {
                *out = value;
            }
            self.pos += 1;
        }
    }
}
