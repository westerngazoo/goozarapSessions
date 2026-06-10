//! Instrument renderers — the output side of voice-to-instrument.
//!
//! Karplus-Strong plucked string (guitar/bass), FM and wavetable voices,
//! drum synthesis (808-style kicks built from sine + envelope ratios), and a
//! sampler. FX chain built on `gooz-dsp` primitives: distortion, delay,
//! convolution reverb. A note event quantized by `gooz-ratio` comes in; a
//! rendered instrument part comes out.
//!
//! Bounded responsibility: turning note events into instrument audio. No
//! device I/O, no transport, no ML (timbre transfer lives in `gooz-model`).
//!
//! Implementation lands per accepted requirement + spec (see `ROADMAP.md`).
