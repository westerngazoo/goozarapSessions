//! Custom DSP library — Rust-first, ours.
//!
//! STFT/FFT wrappers, YIN/pYIN pitch tracking, spectral-flux onset detection,
//! envelope followers, biquad filters, waveshaping/distortion curves, and
//! time-stretch / pitch-shift (phase vocoder). All processing is block-based
//! (`&[f32]` in/out) so the same code serves offline analysis and the
//! real-time engine.
//!
//! Bounded responsibility: signal analysis & transformation. No device I/O,
//! no scheduling, no ML.
//!
//! Implementation lands per accepted requirement + spec (see `ROADMAP.md`).
