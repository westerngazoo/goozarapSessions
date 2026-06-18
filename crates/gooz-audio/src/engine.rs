//! [`Engine`] — the transport: start/stop recording and playback over a backend.

use crate::backend::{AudioBackend, AudioStream};
use crate::error::AudioError;
use crate::metronome::Metronome;
use crate::ring::{playback_channel, record_channel};
use crate::take::Take;

/// Drives recording and playback over any [`AudioBackend`].
///
/// Sequential record-then-play (v0): recording captures input into a [`Take`];
/// playback streams a take to the output. Holds the open streams so dropping or
/// stopping the engine stops the audio.
///
/// `Engine<B>` is `!Send` (it holds an [`AudioStream`], whose resource is
/// `!Send` for cpal) — an accepted v0 constraint; control is single-threaded.
///
/// ```
/// use gooz_audio::{Engine, Take, VirtualBackend};
///
/// let backend = VirtualBackend::new(48_000, 1, 64);
/// let driver = backend.clone();
/// let mut engine = Engine::new(backend);
///
/// let signal = vec![0.1, 0.2, 0.3, 0.4];
/// engine.start_recording(signal.len()).unwrap();
/// driver.feed_input(&signal);
/// let take = engine.stop_recording();
/// assert_eq!(take.samples(), signal.as_slice());
///
/// engine.start_playback(&take).unwrap();
/// assert_eq!(driver.pull_output(signal.len()), signal);
/// ```
pub struct Engine<B: AudioBackend> {
    backend: B,
    recording: Option<(crate::ring::RecordSink, AudioStream)>,
    playback: Option<AudioStream>,
    metronome: Option<AudioStream>,
}

impl<B: AudioBackend> Engine<B> {
    /// Wraps a backend in an idle engine.
    pub fn new(backend: B) -> Engine<B> {
        Engine {
            backend,
            recording: None,
            playback: None,
            metronome: None,
        }
    }

    /// The underlying backend.
    pub fn backend(&self) -> &B {
        &self.backend
    }

    /// Whether a recording is in progress.
    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    /// Whether a playback is in progress.
    pub fn is_playing(&self) -> bool {
        self.playback.is_some()
    }

    /// Whether the metronome is running.
    pub fn is_metronome_running(&self) -> bool {
        self.metronome.is_some()
    }

    /// Starts capturing input into a ring sized for `capacity_frames` frames.
    ///
    /// A second `start_recording` while already recording is a no-op returning
    /// `Ok(())` — the in-flight capture is preserved. Propagates the backend's
    /// `AudioError`; introduces no new error variants.
    pub fn start_recording(&mut self, capacity_frames: usize) -> Result<(), AudioError> {
        if self.recording.is_some() {
            return Ok(());
        }
        let capacity = capacity_frames * self.backend.channels().max(1) as usize;
        let (mut recorder, sink) = record_channel(capacity);
        let stream = self.backend.open_input(Box::new(move |data: &[f32]| {
            recorder.capture(data);
        }))?;
        self.recording = Some((sink, stream));
        Ok(())
    }

    /// Stops recording and returns the captured [`Take`], stamped with the
    /// backend's sample rate and channel count. If not recording, returns a
    /// well-formed empty take with those same values.
    pub fn stop_recording(&mut self) -> Take {
        let mut samples = Vec::new();
        if let Some((mut sink, stream)) = self.recording.take() {
            drop(stream); // stop input first, then drain what was captured
            sink.drain(&mut samples);
        }
        Take::new(samples, self.backend.sample_rate(), self.backend.channels())
    }

    /// Starts playing `take` to the output. A `start_playback` while already
    /// playing replaces the current playback: the old output stream is dropped
    /// (stopping it) and a new one opened. Propagates the backend's `AudioError`.
    pub fn start_playback(&mut self, take: &Take) -> Result<(), AudioError> {
        // Take playback and the metronome share the single device output; drop
        // both current sources first so the replacement owns the output slot.
        self.metronome = None;
        self.playback = None;
        let (mut feed, mut player) = playback_channel(take.samples().len());
        feed.load(take.samples());
        let stream = self
            .backend
            .open_output(Box::new(move |output: &mut [f32]| {
                player.render(output);
            }))?;
        self.playback = Some(stream);
        Ok(())
    }

    /// Starts the ratio-locked metronome on a continuous output stream. The
    /// metronome and take playback share the single device output, so starting
    /// the metronome stops any take playback (and any running metronome).
    /// Propagates the backend's `AudioError`.
    pub fn start_metronome(&mut self, mut metronome: Metronome) -> Result<(), AudioError> {
        self.playback = None;
        self.metronome = None;
        let stream = self
            .backend
            .open_output(Box::new(move |output: &mut [f32]| {
                metronome.render(output);
            }))?;
        self.metronome = Some(stream);
        Ok(())
    }

    /// Stops any recording, playback, and metronome, returning to idle.
    pub fn stop(&mut self) {
        self.recording = None;
        self.playback = None;
        self.metronome = None;
    }
}
