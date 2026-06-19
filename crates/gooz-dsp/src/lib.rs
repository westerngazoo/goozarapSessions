//! Custom DSP library — analysis of recorded audio.
//!
//! Turns a recorded monophonic take (a hum, a sung line, a beatboxed groove)
//! into **note events** via [`analyze`]: YIN pitch tracking ([`pitch_track`])
//! and spectral-flux onset detection ([`detect_onsets`]), with the pitch track
//! and onsets exposed as intermediate results. This is the first analysis stage
//! of the Easy Mode hum→riff loop (R-0006 then snaps the notes onto the ratio
//! grid).
//!
//! Bounded responsibility: signal analysis over `&[f32]` + sample rate. No
//! device I/O, no scheduling — it sits below the audio engine. Offline only;
//! monophonic. Realizes R-0005 / SPEC-0005.
//!
//! ```
//! use gooz_dsp::{analyze, Config};
//!
//! let sr = 48_000;
//! // 50 ms silence, then 0.3 s of 330 Hz.
//! let lead = vec![0.0f32; (0.05 * sr as f64) as usize];
//! let tone: Vec<f32> = (0..(0.3 * sr as f64) as usize)
//!     .map(|i| 0.8 * (std::f64::consts::TAU * 330.0 * i as f64 / sr as f64).sin() as f32)
//!     .collect();
//! let signal = [lead, tone].concat();
//!
//! let t = analyze(&signal, sr, &Config::default())?;
//! assert_eq!(t.notes.len(), 1);
//! assert!((t.notes[0].pitch_hz - 330.0).abs() / 330.0 < 0.01); // within 1%
//! # Ok::<(), gooz_dsp::DspError>(())
//! ```

mod error;
mod onset;
mod quantize;
mod transcribe;
mod yin;

pub use error::DspError;
pub use onset::detect_onsets;
pub use quantize::{QuantizedNote, quantize_notes};
pub use transcribe::{Config, NoteEvent, Onset, PitchFrame, PitchTrack, Transcription, analyze};
pub use yin::pitch_track;

// Re-exported so callers can name the grid/result types from `gooz-dsp` alone.
pub use gooz_ratio::{PitchGrid, Ratio, Tempo};
