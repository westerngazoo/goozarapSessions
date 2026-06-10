# Project Specifics

This is the **single per-project file**. Every other document in this
methodology is generic and identical across all projects — only this file
changes. Fill it in when the project starts; keep it current as these facts
change.

`CLAUDE.md` imports this file, so its contents are always in context.

## Identity

- **Project name:** goozarapSessions
- **One-line description:** A Bizarrap-style music-sessions studio for people
  with no music-theory knowledge — songs are built from frequency ratios and
  beat ratios, voice/mouth sounds become real instrument parts, and a small
  per-song/per-album "influence model" trained locally makes each session
  creatively its own.
- **Owner / final decision authority:** Gustavo Delgadillo (westerngazoo)
- **Repository URL:** https://github.com/westerngazoo/goozarapSessions

## Language & toolchain

The concrete commands referenced by `CLAUDE.md` §6 and by the `architect` and
`qa` agents as merge gates.

- **Primary language / version:** Rust (stable, currently 1.95) — Rust-first
  everywhere, including custom audio/DSP libraries. UI layer via Tauri
  (Rust backend + web frontend) when the app shell lands.
- **Build command:** `cargo build --workspace`
- **Test command:** `cargo test --workspace`
- **Lint command:** `cargo clippy --workspace --all-targets -- -D warnings`
- **Format-check command:** `cargo fmt --all --check`

## Domain notes

Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the full system design.
The non-obvious domain facts:

- **Ratio-first music math.** The app never asks the user for music theory.
  Pitch relationships are small-integer frequency ratios (octave 2:1, fifth
  3:2, …) from the harmonic series; rhythm is beat ratios and Euclidean
  patterns (e.g. `E(3,8)`). Consonance ≈ simplicity of the ratio. This single
  idea is what makes "no music knowledge" possible: the UI exposes
  simple/complex sliders, never note names.
- **Influence models are per-song or per-album, local, and small.** Like a
  musician absorbing influences, the user feeds reference tracks; the app
  extracts features (tempo/ratio profiles, timbre embeddings, structure) and
  trains small local adapters (DDSP-style timbre decoders, LoRA adapters on a
  small lyric LM). No cloud training; a song's model lives inside its session.
- **Voice is the universal input device.** Hum/beatbox → pitch+onset tracking →
  quantize onto the ratio grid → render as a synthesized instrument (e.g.
  Karplus-Strong guitar + distortion) or via timbre transfer. Easy Mode is
  built entirely around this.
- **Two modes.** *Easy Mode* (default): voice-driven, ratio sliders, templates.
  *Advanced Mode* (later milestone): conventional musician-facing controls.
- **Rap copilot.** Live beat + on-device transcription (Whisper via candle);
  rhyme suggestions are phoneme-based (multi-syllabic) and then ranked for
  semantic coherence with the verse so they "actually make sense"; song
  templates give structure (verse/hook bar counts, ending-word targets).
- **Real-time audio discipline.** Anything on the audio callback path is
  allocation-free and lock-free. ML inference and training never run on the
  audio thread.

## Milestone themes

Mirrored into `ROADMAP.md`:

- **M1 — Ratio core & audio engine:** the math foundation and a working
  record/playback engine with a ratio-locked clock.
- **M2 — Easy Mode voice-to-riff:** hum → guitar riff end to end.
- **M3 — Sessions:** project format, arrangement, mixdown/export.
- **M4 — Influence models:** per-song local training + timbre transfer.
- **M5 — Rap copilot:** freestyle beats, live transcription, rhyme assist,
  song templating.
- **M6 — Advanced Mode:** musician-facing depth.
