# SPEC-0009 — Beat builder

- **Status:** Implemented
- **Realizes:** R-0009
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-02
- **Depends on:** SPEC-0002 (Euclidean patterns, tempo); SPEC-0003 (playback, demo only)
- **Module(s):** `crates/gooz-synth`, `apps/gooz-studio`

## 1. Motivation

Realizes R-0009: render Euclidean drum templates as a bar-aligned, loopable beat
stem — the rhythmic complement to the hum→riff pipeline. Closes M2.

## 2. Design

### `gooz-synth` — drum voices + beat renderer

```
crates/gooz-synth/src/
├── drums.rs   DrumKind, one-shot kick/snare/hat synthesis (deterministic)
├── beat.rs    BeatVoice, render_beat()
└── lib.rs     re-exports + gooz_ratio::Pattern
```

Add `gooz-ratio` as a direct dependency (patterns are rhythm math).

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrumKind { Kick, Snare, HiHat }

#[derive(Debug, Clone, PartialEq)]
pub struct BeatVoice {
    pub kind: DrumKind,
    pub pattern: gooz_ratio::Pattern,
    pub level: f32, // clamped to [0, 1] at render time
}

pub fn render_beat(
    voices: &[BeatVoice],
    tempo: &gooz_ratio::Tempo,
    bars: u32,
    sample_rate: u32,
) -> Vec<f32>;
```

**Placement:** for bar `b` and pattern step `s` (when `pattern.is_onset(s)`), hit at
sample `b·bar_samples + round(s/len · bar_samples)`.

**Mix:** sum one-shots at each hit, peak-normalize to `[-1, 1]`. Empty input guards:
`bars == 0`, `sample_rate == 0`, or `voices.is_empty()` → empty `Vec`.

**One-shots (deterministic, fixed seeds per kind):**

- **Kick** — ~200 ms sine with descending pitch envelope (808-style thump).
- **Snare** — ~150 ms mix of band-limited noise + short body tone.
- **Hi-hat** — ~50 ms high-frequency noise burst with fast decay.

### `gooz-studio` — integration + demo

```
apps/gooz-studio/src/
├── beat.rs       BeatStem, BeatConfig, build_beat()
└── bin/beat.rs   by-ear demo: build_beat → loop-play
```

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BeatStem {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub bars: u32,
}
// invariant: bars == 0 ⇔ samples.is_empty()

#[derive(Debug, Clone, PartialEq)]
pub struct BeatVoiceSpec {
    pub kind: DrumKind,
    pub onsets: u32,   // k
    pub steps: u32,    // n
    pub rotate: i64,   // cyclic offset applied after euclidean()
    pub level: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BeatConfig {
    pub voices: Vec<BeatVoiceSpec>,
    pub bars: u32,
}

impl Default for BeatConfig { /* trap default from R-0009 decision log */ }

pub fn build_beat(
    tempo: &Tempo,
    sample_rate: u32,
    cfg: &BeatConfig,
) -> Result<BeatStem, BeatError>;
```

1. Validate/build each `Pattern::euclidean(k, n)?.rotate(rotate)`.
2. `raw = gooz_synth::render_beat(&voices, tempo, cfg.bars, sample_rate)`.
3. If `cfg.bars == 0` or `sample_rate == 0` → empty stem.
4. Else `BeatStem { samples: raw, sample_rate, bars: cfg.bars }`.

### Demo (`bin/beat.rs`, AC8)

`Tempo::new(92.0, 4.0)`, `BeatConfig::default()` (4 bars), `CpalBackend` loop-play
4× — same channel-adapt approach as R-0008's `main.rs`. Run with
`cargo run -p gooz-studio --bin beat`.

## 3. Non-goals

- Influence-model biasing (R-0018), session save (R-0010), UI (R-0013).
- Sampler/FM drums, per-step velocity, live sequencer.

## 4. Acceptance criteria

Maps to R-0009 AC1–AC8; qa owns:

- `crates/gooz-synth/tests/acceptance_r0009.rs` — pure renderer
- `apps/gooz-studio/tests/acceptance_r0009.rs` — `build_beat` integration

- [x] AC1 — three-voice default template → non-empty bounded stem
- [x] AC2 — `samples.len() == bars · bar_samples`
- [x] AC3 — peaks align with Euclidean onset sample positions
- [x] AC4 — higher `k` → more detected hits per bar
- [x] AC5 — invalid `E(k,n)` → `BeatError`; zero bars/rate → empty stem, no panic
- [x] AC6 — deterministic `assert_eq!` on full buffer
- [x] AC7 — bounded `[-1, 1]`, no NaN/inf
- [x] AC8 — by-ear demo binary; doc examples; four gates green

## 5. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | Different `n` per voice; hits placed by `step/len` phase within the bar | Lets hat run `E(7,16)` while snare runs `E(2,16)` without forcing a shared grid resolution. |
| 2026-07-02 | `build_beat` returns `BeatError` from pattern construction; `render_beat` is infallible on pre-built patterns | Errors originate in `E(k,n)` validation; renderer only guards degenerate inputs. |
| 2026-07-02 | Peak normalize after mix (same discipline as `render_notes`) | Keeps output bounded before any future FX chain. |

## Changelog

- 2026-07-02 — created; accepted alongside R-0009.
