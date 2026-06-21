//! goozarapSessions studio — the integration / app layer.
//!
//! The only crate that depends on the engine ([`gooz_audio`]), the DSP
//! ([`gooz_dsp`]), and the synth ([`gooz_synth`]) together. It wires them into
//! the signature loop: [`hum_to_riff`] turns a recorded monophonic take into a
//! loopable guitar riff — transcribe (R-0005) → snap to the grids (R-0006) →
//! render (R-0007) → a bar-aligned stem — returning the stem plus what it heard.
//!
//! The pure pipeline lives here in the library (deterministic, no device); the
//! by-ear record→riff→loop demo is the binary (`cargo run -p gooz-studio`). The
//! Tauri Easy Mode shell lands later (R-0013). Realizes R-0008 / SPEC-0008.

mod pipeline;

pub use pipeline::{PipelineConfig, RiffOutcome, RiffStem, hum_to_riff};
