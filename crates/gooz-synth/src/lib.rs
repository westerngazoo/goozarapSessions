//! Instrument renderers — the output side of voice-to-instrument.
//!
//! v0 ([`render_notes`]) turns R-0006's quantized notes into a guitar buffer: a
//! Karplus-Strong plucked string ([per note][crate::render]) tuned to each
//! note's frequency, let to ring its natural decay, mixed, and run through a
//! selectable [`Distortion`] (soft- or hard-clip) FX. Excitation is a fixed-seed
//! PRNG, so a given note list always renders the same buffer.
//!
//! [`build_beat`] (R-0009) turns Euclidean `E(k, n)` specs into a bar-aligned,
//! loopable drum stem (kick / snare / hat) plus the per-voice hit maps for UI
//! sliders.
//!
//! Bounded responsibility: notes → instrument audio; rhythms → drum stems. No
//! device I/O, no transport, no analysis. Realizes R-0007 / SPEC-0007 and
//! R-0009 / SPEC-0009.
//!
//! ```
//! use gooz_synth::{render_notes, QuantizedNote, Ratio, RenderConfig};
//!
//! let notes = vec![QuantizedNote {
//!     degree: Ratio::new(3, 2).unwrap(),
//!     octave: 0,
//!     freq_hz: 330.0,
//!     cents_offset: 0.0,
//!     onset_step: 0,
//!     onset_secs: 0.0,
//!     duration_secs: 0.5,
//! }];
//! let audio = render_notes(&notes, 48_000, &RenderConfig::default());
//! assert!(audio.iter().all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-6));
//! ```

mod beat;
mod distortion;
mod render;
mod string;

pub use beat::{BeatOutcome, BeatVoice, DrumVoiceConfig, build_beat};
pub use distortion::Distortion;
pub use render::{RenderConfig, render_notes};

// Re-exported so callers can construct `render_notes`'s `QuantizedNote` input
// (and read its `degree`) naming only `gooz-synth`.
pub use gooz_dsp::{QuantizedNote, Ratio};
