//! Rhyme & flow engine — the rap copilot's brain.
//!
//! Grapheme-to-phoneme conversion (English via CMUdict-style lexicon +
//! rules; Spanish phonemizer too), multi-syllabic rhyme search with
//! assonance/consonance scoring, semantic-coherence ranking of candidates
//! (embeddings via `gooz-model`) so suggestions actually make sense in the
//! verse, song-structure templates (bars per section, ending-word targets),
//! and syllable/flow counting against the beat grid from `gooz-ratio`.
//!
//! Bounded responsibility: language and structure. No audio, no transcription
//! (that is `gooz-model`'s Whisper), no UI.
//!
//! Implementation lands per accepted requirement + spec (see `ROADMAP.md`).
