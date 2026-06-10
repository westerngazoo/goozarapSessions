//! Real-time audio engine.
//!
//! Device I/O (cpal), a lock-free audio graph (sampler, synth voice, FX,
//! mixer, meter nodes), a transport whose clock is locked to the beat-ratio
//! grid from `gooz-ratio`, recording into takes, and playback.
//!
//! Engine discipline (see `docs/ARCHITECTURE.md` §6): the audio callback
//! never allocates, locks, or performs file/ML I/O; control flows in via
//! lock-free SPSC queues, telemetry flows out via atomics and ring buffers.
//!
//! Bounded responsibility: moving samples in real time. No instrument design
//! (that is `gooz-synth`), no project persistence (that is `gooz-session`).
//!
//! Implementation lands per accepted requirement + spec (see `ROADMAP.md`).
