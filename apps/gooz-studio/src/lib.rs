//! goozarapSessions studio — the integration / app layer.
//!
//! The only crate that depends on the engine ([`gooz_audio`]), the DSP
//! ([`gooz_dsp`]), and the synth ([`gooz_synth`]) together. It wires them into
//! the signature loop: [`hum_to_riff`] turns a recorded monophonic take into a
//! loopable guitar riff — transcribe (R-0005) → snap to the grids (R-0006) →
//! render (R-0007) → a bar-aligned stem — returning the stem plus what it heard.
//!
//! The pure pipeline lives here in the library (deterministic, no device); the
//! by-ear record→riff→loop demo is the binary (`cargo run -p gooz-studio`).
//! Realizes R-0008 / SPEC-0008.
//!
//! The [`view`] layer ([`demo_riff`], [`riff_from_take`], [`RiffView`]) adapts a
//! pipeline outcome into serializable DTOs for the Easy Mode Tauri shell
//! (R-0013 v0); the shell crate under `src-tauri/` wraps these in commands.

mod pipeline;
mod view;

pub use pipeline::{PipelineConfig, RiffOutcome, RiffStem, hum_to_riff};
pub use view::{NoteView, RiffView, demo_riff, riff_from_take};
