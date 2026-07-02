//! The project model.
//!
//! A [`Song`] is its [`Settings`] (tempo + grid), its [`Take`]s (recorded
//! audio), and its [`Stem`]s (rendered loopable parts — riffs and beats), plus
//! a `model_ref` reserved for its per-song influence model (M4). The song
//! serializes losslessly to JSON and back ([`Song::save`] / [`Song::load`]).
//!
//! Bounded responsibility: the persistence and structure of a session. No audio
//! processing, no rendering, no ML execution. Realizes R-0010 / SPEC-0010;
//! arrangement (R-0011) and mixdown/export (R-0012) build on this model.
//!
//! ```
//! use gooz_session::{Settings, Song, Stem, StemKind};
//!
//! let settings = Settings { bpm: 92.0, beats_per_bar: 4.0, root_hz: 220.0, odd_limit: 9 };
//! let song = Song::new("session 001", settings).with_stem(Stem {
//!     name: "riff".into(),
//!     kind: StemKind::Riff,
//!     sample_rate: 48_000,
//!     bars: 2,
//!     samples: vec![0.0, 0.1, -0.1],
//! });
//! let json = song.to_json().unwrap();
//! assert_eq!(Song::from_json(&json).unwrap(), song);
//! ```

mod arrangement;
mod error;
mod export;
mod model;

pub use arrangement::{Arrangement, LoopRegion, Section, StemPlacement};
pub use error::SessionError;
pub use export::Mixdown;
pub use model::{FORMAT_VERSION, Settings, Song, Stem, StemKind, Take};
