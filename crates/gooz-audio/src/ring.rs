//! The real-time-safe sample path: two SPSC ring channels.
//!
//! [`record_channel`] and [`playback_channel`] each split a preallocated
//! lock-free ring into an audio-thread half and a control-thread half. The
//! audio-thread operations ([`Recorder::capture`], [`Player::render`]) are a
//! single `push_slice`/`pop_slice` plus a zero-fill — no heap allocation, no
//! locking, no I/O — so they are safe to call from an audio callback.

use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

/// The audio-thread producer end of a recording ring.
pub struct Recorder {
    prod: HeapProd<f32>,
}

/// The control-thread consumer end of a recording ring.
pub struct RecordSink {
    cons: HeapCons<f32>,
}

/// The control-thread producer end of a playback ring.
pub struct PlaybackFeed {
    prod: HeapProd<f32>,
}

/// The audio-thread consumer end of a playback ring.
pub struct Player {
    cons: HeapCons<f32>,
}

/// Creates a recording ring holding `capacity` samples, returning its
/// audio-thread and control-thread halves.
///
/// ```
/// use gooz_audio::record_channel;
///
/// let (mut recorder, mut sink) = record_channel(8);
/// recorder.capture(&[1.0, 2.0, 3.0]);
/// let mut out = Vec::new();
/// sink.drain(&mut out);
/// assert_eq!(out, vec![1.0, 2.0, 3.0]);
/// ```
pub fn record_channel(capacity: usize) -> (Recorder, RecordSink) {
    let (prod, cons) = HeapRb::<f32>::new(capacity.max(1)).split();
    (Recorder { prod }, RecordSink { cons })
}

/// Creates a playback ring holding `capacity` samples, returning its
/// control-thread and audio-thread halves.
///
/// ```
/// use gooz_audio::playback_channel;
///
/// let (mut feed, mut player) = playback_channel(8);
/// feed.load(&[0.5, -0.5]);
/// let mut out = [0.0; 4];
/// player.render(&mut out);
/// assert_eq!(out, [0.5, -0.5, 0.0, 0.0]); // tail zero-filled
/// ```
pub fn playback_channel(capacity: usize) -> (PlaybackFeed, Player) {
    let (prod, cons) = HeapRb::<f32>::new(capacity.max(1)).split();
    (PlaybackFeed { prod }, Player { cons })
}

impl Recorder {
    /// Pushes input samples into the ring, returning how many were stored.
    /// Audio-thread safe: a single `push_slice`. On overrun the excess is
    /// dropped — it never blocks or allocates.
    pub fn capture(&mut self, input: &[f32]) -> usize {
        self.prod.push_slice(input)
    }
}

impl RecordSink {
    /// Drains all available samples into `out` (control thread). The stack
    /// scratch buffer is fine here — this is not the audio thread.
    pub fn drain(&mut self, out: &mut Vec<f32>) {
        let mut scratch = [0.0f32; 1024];
        loop {
            let popped = self.cons.pop_slice(&mut scratch);
            if popped == 0 {
                break;
            }
            out.extend_from_slice(&scratch[..popped]);
        }
    }
}

impl PlaybackFeed {
    /// Queues samples for playback (control thread), returning how many were
    /// stored.
    pub fn load(&mut self, samples: &[f32]) -> usize {
        self.prod.push_slice(samples)
    }
}

impl Player {
    /// Fills `output` with queued samples, zero-filling any remainder.
    /// Audio-thread safe: a single `pop_slice` plus a zero-fill. On underrun
    /// (including a never-loaded ring) it emits silence — it never blocks or
    /// allocates.
    pub fn render(&mut self, output: &mut [f32]) {
        let filled = self.cons.pop_slice(output);
        output[filled..].fill(0.0);
    }
}
