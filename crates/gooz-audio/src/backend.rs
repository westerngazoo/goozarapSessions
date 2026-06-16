//! The device seam: the [`AudioBackend`] trait, the [`AudioStream`] handle, and
//! the deterministic [`VirtualBackend`] used for tests.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::error::AudioError;

/// A capture callback: invoked with each incoming block of input samples.
type CaptureCb = Box<dyn FnMut(&[f32]) + Send>;
/// A render callback: invoked to fill each outgoing block of output samples.
type RenderCb = Box<dyn FnMut(&mut [f32]) + Send>;

type CaptureSlot = Arc<Mutex<Option<CaptureCb>>>;
type RenderSlot = Arc<Mutex<Option<RenderCb>>>;

/// Locks a mutex, recovering the guard even if a previous holder panicked, so
/// no audio path can be brought down by lock poisoning.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// An opaque handle that keeps an open stream alive and **stops it when
/// dropped**. It holds its resource as `Box<dyn Any>` so it does not require
/// `Send` — cpal's `Stream` is `!Send` on some platforms.
pub struct AudioStream {
    // RAII only: the boxed value's `Drop` stops the stream (cpal) or clears the
    // backend's callback slot (virtual). Never read directly.
    #[allow(dead_code)]
    keep: Box<dyn std::any::Any>,
}

impl AudioStream {
    pub(crate) fn new(keep: impl std::any::Any) -> AudioStream {
        AudioStream {
            keep: Box::new(keep),
        }
    }
}

/// A source of input/output streams. The engine is generic over this seam, so
/// the same engine logic runs on a real device ([`crate::CpalBackend`]) and on
/// the deterministic [`VirtualBackend`].
pub trait AudioBackend {
    /// The stream sample rate in Hz.
    fn sample_rate(&self) -> u32;
    /// The stream channel count.
    fn channels(&self) -> u16;
    /// Opens an input stream that calls `capture` with each incoming block.
    fn open_input(&self, capture: CaptureCb) -> Result<AudioStream, AudioError>;
    /// Opens an output stream that calls `render` to fill each outgoing block.
    fn open_output(&self, render: RenderCb) -> Result<AudioStream, AudioError>;
}

/// Clears a virtual backend's callback slot when the [`AudioStream`] is dropped,
/// modelling "stream stopped".
struct SlotGuard<T: Send + 'static> {
    slot: Arc<Mutex<Option<T>>>,
}

impl<T: Send + 'static> Drop for SlotGuard<T> {
    fn drop(&mut self) {
        *lock(&self.slot) = None;
    }
}

/// A deterministic, in-memory backend with no device — what CI uses.
///
/// `open_input`/`open_output` store the callback in a shared slot;
/// [`VirtualBackend::feed_input`] and [`VirtualBackend::pull_output`] drive
/// those callbacks synchronously, so record/playback is fully repeatable
/// without hardware. `Clone` shares the slots, letting a test keep a driver
/// handle after the engine takes ownership of the backend.
///
/// ```
/// use gooz_audio::{AudioBackend, VirtualBackend, record_channel};
///
/// let backend = VirtualBackend::new(48_000, 1, 64);
/// let (mut recorder, mut sink) = record_channel(64);
/// let _stream = backend
///     .open_input(Box::new(move |data: &[f32]| { recorder.capture(data); }))
///     .unwrap();
/// backend.feed_input(&[0.1, 0.2, 0.3]);
/// let mut out = Vec::new();
/// sink.drain(&mut out);
/// assert_eq!(out, vec![0.1, 0.2, 0.3]);
/// ```
#[derive(Clone)]
pub struct VirtualBackend {
    sample_rate: u32,
    channels: u16,
    block: usize,
    input: CaptureSlot,
    output: RenderSlot,
}

impl VirtualBackend {
    /// Builds a virtual backend at the given format, driving callbacks in
    /// `block`-sample chunks.
    pub fn new(sample_rate: u32, channels: u16, block: usize) -> VirtualBackend {
        VirtualBackend {
            sample_rate,
            channels,
            block: block.max(1),
            input: Arc::new(Mutex::new(None)),
            output: Arc::new(Mutex::new(None)),
        }
    }

    /// Drives the registered capture callback with `signal`, in `block`-sample
    /// chunks. A no-op if no input stream is open.
    pub fn feed_input(&self, signal: &[f32]) {
        let mut slot = lock(&self.input);
        if let Some(capture) = slot.as_mut() {
            for chunk in signal.chunks(self.block) {
                capture(chunk);
            }
        }
    }

    /// Drives the registered render callback to produce `frames` of output,
    /// returning `frames * channels` samples. Returns zeros when no output
    /// stream is open (silence when nothing is loaded to play).
    pub fn pull_output(&self, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; frames * self.channels as usize];
        let mut slot = lock(&self.output);
        if let Some(render) = slot.as_mut() {
            for chunk in out.chunks_mut(self.block) {
                render(chunk);
            }
        }
        out
    }
}

impl AudioBackend for VirtualBackend {
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn open_input(&self, capture: CaptureCb) -> Result<AudioStream, AudioError> {
        *lock(&self.input) = Some(capture);
        Ok(AudioStream::new(SlotGuard {
            slot: self.input.clone(),
        }))
    }

    fn open_output(&self, render: RenderCb) -> Result<AudioStream, AudioError> {
        *lock(&self.output) = Some(render);
        Ok(AudioStream::new(SlotGuard {
            slot: self.output.clone(),
        }))
    }
}
