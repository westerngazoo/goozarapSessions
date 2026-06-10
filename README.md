# goozarapSessions

A Bizarrap-style music-sessions studio for people with **no music-theory
knowledge**. Music is constructed from math the way sound actually works —
**frequency ratios** for pitch and harmony, **beat ratios & Euclidean
patterns** for rhythm — so "making it sound good" is turning a
smooth↔tense slider, not knowing what a minor seventh is.

## What it does

- **Voice is the instrument.** Hum a melody or make a guitar-ish mouth noise;
  the app tracks pitch and onsets, snaps them onto the ratio grid, and renders
  a real part — e.g. a distorted guitar riff — ready to loop in the session.
- **Per-song influence models.** Like a musician digesting influences: drop
  reference tracks into a song project and the app trains a *small, local*
  model on their tempo/ratio profiles, timbre, structure, and lyrical style.
  Beats, riff rendering, and lyric suggestions are then conditioned per song —
  every session gets its own creative direction. No cloud, no big models.
- **Rap copilot.** Generate freestyle beats from ratio templates, transcribe
  the rap live on-device, and suggest ending words that *rhyme* (multi-syllabic,
  phoneme-level) and *make sense* (semantically ranked against the verse) —
  plus song templates that coach structure bar by bar.
- **Easy Mode first, Advanced Mode later.** Non-musicians get voice + sliders +
  templates; musicians eventually get the full panel.

## How it's built

Rust everywhere — including the audio/DSP libraries, which are ours. A Cargo
workspace of layered crates (deps point strictly inward):

```
apps/gooz-studio      Tauri desktop shell (Easy Mode UI)
crates/gooz-session   project format, stems, arrangement, export
crates/gooz-model     local ML: per-song influence models (candle)
crates/gooz-lyrics    rhyme + flow engine (G2P, multi-syllabic, semantic rank)
crates/gooz-synth     instruments: Karplus-Strong guitar, FM, drums, FX
crates/gooz-audio     real-time engine: cpal I/O, lock-free graph, ratio clock
crates/gooz-dsp       custom DSP: FFT, pitch (YIN), onsets, filters, distortion
crates/gooz-ratio     pure math core: frequency & beat ratios, Euclidean rhythms
```

Full design: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Development

This repo follows a requirement- and spec-driven, test-first methodology —
see [CLAUDE.md](CLAUDE.md) (constitution), [ROADMAP.md](ROADMAP.md)
(milestones & backlog), and `requirements/` + `specs/` (the paper trail).
Nothing is implemented without an accepted requirement and spec.

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```
