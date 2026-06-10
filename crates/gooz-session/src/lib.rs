//! The project model.
//!
//! A `Song` is stems, takes, an arrangement (sections as bar-ratio spans),
//! tempo/ratio settings, and a reference to its per-song influence model.
//! This crate owns session serialization, the on-disk project directory
//! layout (including the song's model artifacts), and WAV/stem export.
//!
//! Bounded responsibility: persistence and structure of a session. No audio
//! processing, no rendering, no ML execution.
//!
//! Implementation lands per accepted requirement + spec (see `ROADMAP.md`).
