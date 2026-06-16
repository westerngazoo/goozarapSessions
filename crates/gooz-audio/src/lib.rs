//! Real-time audio engine — record a take, play it back.
//!
//! The engine's first heartbeat: moving samples in and out. The device is
//! decoupled behind the [`AudioBackend`] seam, so the same [`Engine`] logic
//! runs on a real device ([`CpalBackend`]) and on a deterministic, in-memory
//! [`VirtualBackend`] — which is what lets CI prove the sample path with no
//! sound card.
//!
//! Engine discipline (see `docs/ARCHITECTURE.md` §6): the audio-callback path
//! ([`Recorder::capture`], [`Player::render`]) only pushes to / pops from
//! preallocated lock-free ring buffers — no allocation, no locking, no I/O.
//!
//! Bounded responsibility: moving samples in real time. The ratio-locked
//! clock/metronome is R-0004; pitch/DSP is R-0005. Realizes R-0003 / SPEC-0003.
//!
//! ```
//! use gooz_audio::{Engine, VirtualBackend};
//!
//! let backend = VirtualBackend::new(48_000, 1, 64);
//! let driver = backend.clone();
//! let mut engine = Engine::new(backend);
//!
//! let signal = vec![0.0, 0.1, 0.2, 0.3, 0.4];
//! engine.start_recording(signal.len())?;
//! driver.feed_input(&signal);
//! let take = engine.stop_recording();
//! engine.start_playback(&take)?;
//! assert_eq!(driver.pull_output(signal.len()), signal);
//! # Ok::<(), gooz_audio::AudioError>(())
//! ```

mod backend;
mod cpal_backend;
mod engine;
mod error;
mod ring;
mod take;

pub use backend::{AudioBackend, AudioStream, VirtualBackend};
pub use cpal_backend::CpalBackend;
pub use engine::Engine;
pub use error::AudioError;
pub use ring::{PlaybackFeed, Player, RecordSink, Recorder, playback_channel, record_channel};
pub use take::Take;
