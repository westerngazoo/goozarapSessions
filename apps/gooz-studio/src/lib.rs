//! goozarapSessions studio — the integration / app layer.
//!
//! The only crate that depends on the engine ([`gooz_audio`]), the DSP
//! ([`gooz_dsp`]), and the synth ([`gooz_synth`]) together. It wires them into
//! the signature loops:
//!
//! - [`hum_to_riff`] — recorded hum → loopable guitar riff (R-0008)
//! - [`build_beat`] — Euclidean drum templates → loopable beat (R-0009)
//!
//! The pure pipelines live here in the library (deterministic, no device); the
//! by-ear demos are binaries (`cargo run -p gooz-studio` for hum→riff,
//! `cargo run -p gooz-studio --bin beat` for the beat builder).
//!
//! The [`view`] layer ([`demo_riff`], [`riff_from_take`], [`RiffView`]) adapts a
//! pipeline outcome into serializable DTOs for the Easy Mode Tauri shell
//! (R-0013 v0); the shell crate under `src-tauri/` wraps these in commands.

mod beat;
mod pipeline;
mod view;

pub use beat::{BeatConfig, BeatStem, BeatVoiceSpec, build_beat};
pub use gooz_synth::{DrumKind, Pattern};
pub use pipeline::{PipelineConfig, RiffOutcome, RiffStem, hum_to_riff};
pub use view::{NoteView, RiffView, demo_riff, riff_from_take};
