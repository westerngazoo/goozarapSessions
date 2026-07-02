# SPEC-0012 — Mixdown & export

- **Status:** Implemented
- **Realizes:** R-0012
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-02
- **Depends on:** SPEC-0010 (session), SPEC-0011 (arrangement)
- **Module(s):** `crates/gooz-session`

## 1. Motivation

Realizes R-0012: render a song's arrangement to a master buffer and export it
(and each stem) to WAV — the step that makes a session a deliverable.

## 2. Design

New `export` module in `gooz-session`; adds the `hound` dependency.

```
crates/gooz-session/src/
├── error.rs    + SessionError::Export(String)
├── export.rs   Mixdown, Song::mixdown / export_master / export_stems
└── lib.rs      re-export Mixdown
```

### Bar math

`bar_samples = round(60 / bpm * beats_per_bar * sample_rate)` (`.max(1)`) — the
same bar length the engine and studio use, read from `Song::settings`.

### Mixdown (export.rs)

```rust
pub struct Mixdown { pub sample_rate: u32, pub samples: Vec<f32> }

impl Song {
    pub fn mixdown(&self) -> Result<Mixdown, SessionError>;
}
```

1. `self.validate()?` (R-0011 arrangement validation).
2. Master sample rate = the sample rate shared by every *placed, non-muted*
   stem; a mismatch → `SessionError::Export`. No placements → empty master.
3. `total = arrangement.total_bars(&stems) * bar_samples` (`0` → empty master).
4. For each non-muted placement: starting at `start_bar * bar_samples`, write the
   stem's samples **looped** across `[start, total)`, adding `sample * level`.
5. Clip-safety: if the peak magnitude exceeds `1.0`, scale the whole buffer down
   so the peak is `1.0` (already-safe mixes untouched).

Pure and deterministic: summation order is placement order, looping is index
arithmetic, no RNG.

### Export (export.rs)

```rust
impl Song {
    pub fn export_master(&self, path: impl AsRef<Path>) -> Result<(), SessionError>;
    pub fn export_stems(&self, dir: impl AsRef<Path>) -> Result<Vec<PathBuf>, SessionError>;
}
```

- `export_master` mixes then writes a **mono 16-bit PCM** WAV via `hound`
  (`f32 [-1,1] → i16` = `round(clamp(s) * 32767)`).
- `export_stems` writes `NN-name.wav` per stem into `dir` (created if missing),
  returning the paths. Each stem WAV is the stem's own samples at its rate.
- `hound::Error` / `io::Error` → `SessionError::Export` / `SessionError::Io`.

## 3. Non-goals

Pan, automation, FX, resampling (mismatched rates error), >16-bit / float WAV,
loudness normalization beyond the clip limiter, the UI export button (R-0013).

## 4. Acceptance criteria

Maps to R-0012 AC1–AC8; qa owns `crates/gooz-session/tests/acceptance_r0012.rs`.

- [x] AC1 — mixdown sums placements at bar offsets, looped
- [x] AC2 — muted contributes nothing; level scales
- [x] AC3 — `export_master` WAV reads back with expected frames + rate
- [x] AC4 — `export_stems` writes one WAV per stem, returns paths
- [x] AC5 — master bounded `[-1, 1]`, no NaN/inf
- [x] AC6 — invalid arrangement / rate mismatch / bad path → typed error; empty song → empty master
- [x] AC7 — deterministic mixdown
- [x] AC8 — docs + four gates green

## 5. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | Mixdown is sample summation in `gooz-session`, not via `gooz-audio` | Export is offline file rendering, not real-time; keeping it dependency-light preserves the crate's bounded responsibility. |
| 2026-07-02 | `f32 → i16` at export via `round(clamp * 32767)` | Standard, lossy-but-compatible PCM; the in-session samples stay `f32`. |

## Changelog

- 2026-07-02 — created; accepted alongside R-0012.
