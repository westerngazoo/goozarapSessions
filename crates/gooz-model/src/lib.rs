//! Local ML — per-song / per-album influence models.
//!
//! This crate owns the influence-model lifecycle: the **registry** (this
//! requirement, R-0014), then feature extraction (R-0015), on-device training of
//! small adapters (R-0016), and inference — timbre transfer, beat suggestion,
//! lyric continuation (R-0017/R-0018, M5). It will also host on-device Whisper.
//!
//! Today it implements the [`ModelRegistry`]: one directory per model, stored
//! inside a session (`<session>/models/`), each with an inspectable
//! [`ModelManifest`]. It is pure — `serde` + `std::fs`, no ML dependency — until
//! training (R-0016) pulls in candle.
//!
//! Honesty rule (ARCHITECTURE §5): models *bias* the deterministic ratio math,
//! they never replace it — everything works untrained from neutral defaults.
//! Never runs on the audio thread.
//!
//! ```
//! use gooz_model::{ModelKind, ModelRegistry};
//!
//! let dir = std::env::temp_dir().join(format!("gooz_model_doctest_{}", std::process::id()));
//! let reg = ModelRegistry::open(&dir).unwrap();
//! let handle = reg.create("warm guitar", ModelKind::Timbre).unwrap();
//! assert_eq!(handle.id(), "warm-guitar");
//! assert_eq!(reg.list().unwrap().len(), 1);
//! # std::fs::remove_dir_all(&dir).ok();
//! ```

mod error;
mod registry;

pub use error::ModelError;
pub use registry::{MODEL_FORMAT_VERSION, ModelHandle, ModelKind, ModelManifest, ModelRegistry};
