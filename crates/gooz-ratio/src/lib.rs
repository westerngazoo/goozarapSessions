//! Pure music-math core — the foundation of "no music knowledge needed".
//!
//! Pitch relationships are small-integer frequency ratios from the harmonic
//! series (2:1 octave, 3:2 fifth, 5:4 major third, …); rhythm is beat ratios
//! of a bar, Euclidean distributions `E(k, n)`, and polyrhythm as ratio pairs.
//! This crate also owns snap-to-grid quantization for both pitch and time.
//!
//! Bounded responsibility: math only. No I/O, no audio, no allocation on hot
//! paths, no dependencies on any other workspace crate.
//!
//! Implementation lands per accepted requirement + spec (see `ROADMAP.md`).
