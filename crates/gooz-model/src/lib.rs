//! Local ML — per-song / per-album influence models.
//!
//! The model registry (one model directory per song, stored inside its
//! session), the feature-extraction pipeline (tempo & beat-ratio profiles,
//! ratio/harmony histograms, timbre embeddings, section structure, lyrical
//! style), on-device training of small adapters (DDSP-style timbre decoders,
//! LoRA on a small lyric LM, beat-conditioning vectors), and inference APIs:
//! timbre transfer, beat suggestion, lyric continuation. On-device Whisper
//! transcription also lives here.
//!
//! Framework: candle (Rust-native), with ONNX Runtime (`ort`) as a fallback
//! behind the same API. Honesty rule: models *bias* the deterministic ratio
//! math, they never replace it — everything works untrained from neutral
//! defaults.
//!
//! Bounded responsibility: ML lifecycle. Never runs on the audio thread.
//!
//! Implementation lands per accepted requirement + spec (see `ROADMAP.md`).
