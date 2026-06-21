# SPEC-0008 — Hum-to-riff pipeline

- **Status:** Implemented — QA PASS, architect APPROVE
- **Realizes:** R-0008
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-06-21
- **Depends on:** SPEC-0005, SPEC-0006, SPEC-0007 (analysis/quantize/render); SPEC-0003 (record, demo only)
- **Module(s):** `apps/gooz-studio`

## 1. Motivation

Realizes R-0008: compose R-0005→R-0006→R-0007 into one `hum_to_riff` pipeline
that turns a recorded take into a loopable guitar riff, returning the stem plus
what it heard. The first fully playable expression of the product premise.

## 2. Design

`gooz-studio` becomes a **library + binary**: the pure pipeline lives in the lib
(CI-tested, no device), the device demo in the binary (by ear).

```
apps/gooz-studio/Cargo.toml   deps: gooz-audio, gooz-dsp, gooz-synth (drop the
                              other scaffold deps until their requirements land)
apps/gooz-studio/src/
├── lib.rs       crate docs + pipeline module + re-exports
├── pipeline.rs  RiffStem, RiffOutcome, PipelineConfig, hum_to_riff()
└── main.rs      the by-ear demo: record → hum_to_riff → loop-play
```

Dependency direction: `gooz-studio` is the top of the graph (app layer); it may
depend on every leaf crate. The pure pipeline uses only `gooz-dsp` + `gooz-synth`
(samples in → riff out); the demo adds `gooz-audio`.

### Types (pipeline.rs)

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct RiffStem {
    pub samples: Vec<f32>, // bar-aligned: len == bars · bar_samples; invariant: bars == 0 ⇔ samples.is_empty()
    pub sample_rate: u32,
    pub bars: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RiffOutcome {
    pub stem: RiffStem,
    pub notes: Vec<gooz_dsp::QuantizedNote>,   // grid-locked notes
    pub transcription: gooz_dsp::Transcription, // raw pitch track + onsets
}

#[derive(Debug, Clone, PartialEq)]
pub struct PipelineConfig {
    pub analyze: gooz_dsp::Config,        // R-0005 analysis params
    pub subdivision: u32,                 // beat grid subdivision (R-0006)
    pub render: gooz_synth::RenderConfig, // R-0007 instrument/FX
}
// impl Default: { analyze: Config::default(), subdivision: 2, render: RenderConfig::default() }
```

All composed types already derive `Debug`/`Clone`/`PartialEq`, so the derives
above make `assert_eq!` on a whole `RiffOutcome` the natural AC5 determinism
check.

### `hum_to_riff` (pipeline.rs)

```rust
pub fn hum_to_riff(
    samples: &[f32],
    sample_rate: u32,
    pitch_grid: &gooz_dsp::PitchGrid,
    tempo: &gooz_dsp::Tempo,
    cfg: &PipelineConfig,
) -> Result<RiffOutcome, gooz_dsp::DspError>;
```

1. `transcription = gooz_dsp::analyze(samples, sample_rate, &cfg.analyze)?` —
   propagates `EmptySignal` / `InvalidSampleRate` / `NonFiniteSample` /
   `WindowTooLarge` (AC4, the deferred input guard).
2. `notes = gooz_dsp::quantize_notes(&transcription.notes, pitch_grid, tempo,
   cfg.subdivision)` (R-0006).
3. `raw = gooz_synth::render_notes(&notes, sample_rate, &cfg.render)` (R-0007) —
   bounded `[-1, 1]`, no NaN.
4. Assemble the bar-aligned stem:
   - `bar_samples = (tempo.bar_seconds() · sample_rate as f64).round() as usize`
     (`.max(1)`);
   - if `raw` is empty → `RiffStem { samples: vec![], sample_rate, bars: 0 }`;
   - else `bars = raw.len().div_ceil(bar_samples)` (`≥ 1`), `len = bars ·
     bar_samples`; `samples = raw` zero-padded to `len` (padding only, since
     `len ≥ raw.len()` — tails preserved, AC2).
5. `Ok(RiffOutcome { stem, notes, transcription })`.

Pure and deterministic: every stage is deterministic (R-0005 YIN, R-0006 snap,
R-0007 fixed-seed pluck), so the same inputs yield an identical `RiffOutcome`
(AC5). No panic: the only fallible call (`analyze`) is `?`-propagated;
`bar_samples.max(1)` avoids divide-by-zero; padding is in-bounds.

### Demo (main.rs, AC7)

`CpalBackend::with_defaults()` → record ~4 s into a `Take` → `hum_to_riff(take.
samples(), take.sample_rate(), &grid, &tempo, &cfg)` → play `outcome.stem.samples`
looped a few times through the engine. Concrete demo constants (reproducible):
`grid = PitchGrid::harmonic(220.0, 9)`, `tempo = Tempo::new(92.0, 4.0)`,
`PipelineConfig::default()` (subdivision 2). The three `Result`-returning
constructors (`harmonic`, `Tempo::new`, `with_defaults`) and `hum_to_riff` are
handled with `expect`/`eprintln` in the bin (per §6, a binary may abort with a
justifying message). Channel adaptation reuses R-0003's demo approach. Run with
`cargo run -p gooz-studio`. By ear; not a CI gate.

## 3. Code outline

```rust
// pipeline.rs
pub fn hum_to_riff(
    samples: &[f32], sample_rate: u32,
    pitch_grid: &PitchGrid, tempo: &Tempo, cfg: &PipelineConfig,
) -> Result<RiffOutcome, DspError> {
    let transcription = gooz_dsp::analyze(samples, sample_rate, &cfg.analyze)?;
    let notes = gooz_dsp::quantize_notes(&transcription.notes, pitch_grid, tempo, cfg.subdivision);
    let raw = gooz_synth::render_notes(&notes, sample_rate, &cfg.render);

    let bar_samples = ((tempo.bar_seconds() * sample_rate as f64).round() as usize).max(1);
    let stem = if raw.is_empty() {
        RiffStem { samples: Vec::new(), sample_rate, bars: 0 }
    } else {
        let bars = raw.len().div_ceil(bar_samples);
        let mut samples = raw;
        samples.resize(bars * bar_samples, 0.0);
        RiffStem { samples, sample_rate, bars: bars as u32 }
    };
    Ok(RiffOutcome { stem, notes, transcription })
}
```

## 4. Non-goals

- UI (R-0013), session/stem persistence (R-0010), arrangement.
- Key/tempo detection (caller supplies the grid + tempo).
- Other instruments / the beat builder (R-0009); live/streaming pipeline.

## 5. Open questions

None — settled in the decision log.

## 6. Acceptance criteria

Maps to R-0008 AC1–AC7; qa owns `apps/gooz-studio/tests/acceptance_r0008.rs`
(pure pipeline; no device).

- [x] AC1 — synthesized two-tone hum → non-empty bounded stem + grid-locked notes
      matching the hummed pitches.
- [x] AC2 — stem length == `bars · bar_samples`; `bars ≥ 1` for a non-empty riff;
      length `≥` raw render length (padding only). Exercise a **multi-bar** riff
      (raw length > one bar) so the `div_ceil` path is covered, not just `bars=1`.
- [x] AC3 — outcome exposes the transcription + the quantized notes; counts/
      pitches reflect the hum.
- [x] AC4 — empty / zero-rate / non-finite samples → typed `DspError`; no panic.
- [x] AC5 — deterministic: identical inputs → identical stem samples + notes.
- [x] AC6 — stem bounded in `[-1, 1]`, no NaN/inf.
- [x] AC7 — by-ear demo; doc examples; four gates green.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-21 | `bars = raw.len().div_ceil(bar_samples)`, pad to `bars·bar_samples` | Whole-bar length with the decay tail preserved (padding only); loops on the downbeat. |
| 2026-06-21 | Empty riff → `bars = 0`, empty stem (not a bar of silence) | A hum that yields no notes yields no riff; the caller/UI decides what to do, rather than the pipeline inventing a silent bar. |
| 2026-06-21 | Pure pipeline in `gooz-studio`'s lib; demo in its bin | Keeps the cross-crate orchestration in the app layer while the pure transform stays unit-testable without a device. |
| 2026-06-21 | `RiffStem` invariant: `bars == 0 ⇔ samples.is_empty()` | One flag for downstream (R-0009/R-0010/UI) to branch on "no riff". |
| 2026-06-21 | Architect review (APPROVE) refinements applied: derive `Debug/Clone/PartialEq` on the pipeline types + `impl Default` for `PipelineConfig`; AC2 must exercise a multi-bar riff; pin demo constants; note `notes` stays `gooz_dsp::QuantizedNote` (no dual re-export path) | Findings 1, 2, 3, 5 of the SPEC-0008 review (all minor; design approved as-is). |

## Changelog

- 2026-06-21 — created; accepted alongside R-0008.
