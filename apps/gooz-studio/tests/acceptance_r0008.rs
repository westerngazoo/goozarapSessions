//! Acceptance tests for R-0008 — the hum→riff pipeline (compose R-0005 analysis
//! → R-0006 quantize → R-0007 render into one loopable stem), realized by
//! SPEC-0008.
//!
//! TDD red: this file is written **before** the implementation exists. It is the
//! executable contract that the new `gooz-studio` library surface
//! (`RiffStem`, `RiffOutcome`, `PipelineConfig`, and the `hum_to_riff`
//! function) must satisfy. `gooz-studio` is currently a stub *binary* with no
//! library target, so `gooz_studio::{ … }` does not resolve and this file will
//! not compile. That is the intended red state for loop step 3 — the failure
//! must be "unresolved `gooz_studio` items", not a test typo.
//!
//! **No microphone, no device.** Every "hum" here is synthesized in-process from
//! deterministic helpers (`sine`, `silence`, `concat`) at a fixed sample rate;
//! the grid and tempo are deterministic constructors. The pure pipeline is
//! `samples → riff` with no device I/O (SPEC-0008 §2), so the whole suite is
//! reproducible — exactly the half of AC7 that CI can verify (the by-ear demo is
//! the other half, signed off out of band).
//!
//! One section per acceptance criterion (AC1–AC7); every test fn is prefixed
//! with the criterion id it verifies.
//!
//! ## AC → test mapping
//!
//! | AC  | Verified by                                                          |
//! |-----|----------------------------------------------------------------------|
//! | AC1 | `ac1_two_tone_hum_yields_stem_and_grid_locked_notes`                 |
//! | AC2 | `ac2_stem_is_bar_aligned_and_multi_bar`                              |
//! | AC3 | `ac3_outcome_exposes_transcription_and_quantized_notes`             |
//! | AC4 | `ac4_empty_samples_is_empty_signal_error`,                           |
//! |     | `ac4_zero_sample_rate_is_invalid_sample_rate_error`,                 |
//! |     | `ac4_nan_sample_is_non_finite_error`,                                |
//! |     | `ac4_too_short_buffer_is_window_too_large_error`                     |
//! | AC5 | `ac5_pipeline_is_deterministic`                                      |
//! | AC6 | `ac6_stem_samples_are_bounded_and_finite`                            |
//! | AC7 | verified at QA sign-off (by-ear demo + doc examples + four gates),   |
//! |     | not by a CI test in this file — see the AC7 note below.              |
//!
//! ## API assumptions beyond SPEC-0008 §2/§3
//!
//! SPEC-0008 §2/§3 fix every type, field, function, and error name these tests
//! call; nothing was invented. The notes below pin the exact spelling and
//! semantics relied on, so any mismatch surfaces at the implementation step, not
//! as a test typo:
//!
//! * `RiffStem { samples: Vec<f32>, sample_rate: u32, bars: u32 }` deriving
//!   `Debug, Clone, PartialEq` — verbatim SPEC-0008 §2. Invariant
//!   `bars == 0 ⇔ samples.is_empty()`; `samples.len() == bars · bar_samples`.
//! * `RiffOutcome { stem: RiffStem, notes: Vec<gooz_dsp::QuantizedNote>,
//!   transcription: gooz_dsp::Transcription }` deriving
//!   `Debug, Clone, PartialEq` — SPEC-0008 §2. The whole-outcome `assert_eq!`
//!   in AC5 relies on the `PartialEq` derive (every composed type already
//!   derives it).
//! * `PipelineConfig { analyze: gooz_dsp::Config, subdivision: u32,
//!   render: gooz_synth::RenderConfig }` with `impl Default`
//!   (`analyze: Config::default()`, `subdivision: 2`,
//!   `render: RenderConfig::default()`) — SPEC-0008 §2.
//! * `hum_to_riff(samples: &[f32], sample_rate: u32,
//!   pitch_grid: &gooz_dsp::PitchGrid, tempo: &gooz_dsp::Tempo,
//!   cfg: &PipelineConfig) -> Result<RiffOutcome, gooz_dsp::DspError>` —
//!   SPEC-0008 §2/§3. The only fallible stage is `analyze`, whose error is
//!   `?`-propagated; the same four `DspError` variants therefore surface from
//!   the pipeline (AC4).
//! * `bar_samples = round(tempo.bar_seconds() · sample_rate)` (SPEC-0008 §2
//!   step 4). With `Tempo::new(120.0, 4.0)` (`bar_seconds() == 2.0`) and
//!   `SR == 48_000`, `bar_samples == 96_000` exactly — used by the AC2 length
//!   asserts. `bars = raw.len().div_ceil(bar_samples)`, `samples` padded with
//!   silence to `bars · bar_samples` (padding only — tails preserved).
//! * The hum's two tones (220 Hz, 330 Hz) are *exact grid pitches* of
//!   `PitchGrid::harmonic(220.0, 9)` (`1:1` = 220, `3:2` = 330), so after the
//!   R-0006 snap the quantized `freq_hz` is the exact grid value even if the
//!   YIN median lands a hair off — hence AC1 asserts `freq_hz` with `assert_eq!`
//!   on `220.0` / `330.0` (`f64`, per `QuantizedNote::freq_hz`).
//! * R-0007 let-ring tails are long (the string rings several seconds at the
//!   default decay), so a single note already renders a raw buffer longer than
//!   one 2.0 s bar — guaranteeing the multi-bar (`bars >= 2`, `div_ceil`) path
//!   exercised by AC2.

use gooz_dsp::{Config, DspError, PitchGrid, Tempo};
use gooz_studio::{PipelineConfig, RiffOutcome, RiffStem, hum_to_riff};

/// CD/voice sample rate every synthesized hum is generated at.
const SR: u32 = 48_000;

/// The hummed tones — both exact pitches of the 220-rooted harmonic grid
/// (`1:1` = 220 Hz, `3:2` = 330 Hz), so the snap lands them exactly.
const TONE_A_HZ: f32 = 220.0;
const TONE_B_HZ: f32 = 330.0;

/// Tone length and the silent gap between the two tones (a clear two-note hum).
const TONE_DUR: f64 = 0.40;
const NOTE_GAP: f64 = 0.080;

/// 50 ms silent lead-in so the first attack has a genuine rising edge (the
/// golden-corpus convention the R-0005 onset tests use).
const LEAD_IN: f64 = 0.050;

/// `Tempo::new(120.0, 4.0).bar_seconds() == 2.0 s`, so at `SR` one bar is
/// exactly `2.0 · 48_000 = 96_000` samples — the bar-alignment unit AC2 asserts.
const BAR_SAMPLES: usize = 96_000;

// ---------------------------------------------------------------------------
// Deterministic golden-signal helpers (no device, no rng crate). Mirrors the
// R-0005/R-0006 acceptance harness so the synthesized hum is reproducible.
// ---------------------------------------------------------------------------

/// A pure sine of `freq` Hz for `secs` seconds at `sr`, amplitude ~0.8.
fn sine(freq: f32, secs: f64, sr: u32) -> Vec<f32> {
    let n = (secs * sr as f64).round() as usize;
    let step = std::f64::consts::TAU * freq as f64 / sr as f64;
    (0..n)
        .map(|i| 0.8 * (step * i as f64).sin() as f32)
        .collect()
}

/// `secs` seconds of digital silence at `sr`.
fn silence(secs: f64, sr: u32) -> Vec<f32> {
    vec![0.0; (secs * sr as f64).round() as usize]
}

/// Concatenate signal segments into one buffer (lead-in, tones, gaps).
fn concat(segments: &[&[f32]]) -> Vec<f32> {
    let mut out = Vec::with_capacity(segments.iter().map(|s| s.len()).sum());
    for seg in segments {
        out.extend_from_slice(seg);
    }
    out
}

/// The shared two-tone "hum": lead-in, 220 Hz, gap, 330 Hz — two clear notes at
/// exact grid pitches, the canonical R-0008 input.
fn two_tone_hum() -> Vec<f32> {
    concat(&[
        &silence(LEAD_IN, SR),
        &sine(TONE_A_HZ, TONE_DUR, SR),
        &silence(NOTE_GAP, SR),
        &sine(TONE_B_HZ, TONE_DUR, SR),
    ])
}

/// The pinned 220-rooted harmonic grid (`1:1` = 220, `3:2` = 330 both present).
fn grid() -> PitchGrid {
    PitchGrid::harmonic(220.0, 9).expect("220 Hz / odd-limit 9 is a valid harmonic grid")
}

/// The pinned 120 bpm / 4-beats-per-bar tempo (`bar_seconds() == 2.0 s`).
fn tempo() -> Tempo {
    Tempo::new(120.0, 4.0).expect("120 bpm / 4 beats per bar is a valid tempo")
}

// ---------------------------------------------------------------------------
// AC1 — End-to-end transform.
//
// A synthesized two-tone hum at grid pitches → a non-empty stem plus grid-locked
// notes whose pitches match the hummed tones (snapped to the supplied grid).
// Because the tones are *exact* grid pitches, the snapped freq_hz is the exact
// grid value (220.0 / 330.0) even if the YIN median is a hair off — asserted
// with assert_eq!.
// ---------------------------------------------------------------------------

#[test]
fn ac1_two_tone_hum_yields_stem_and_grid_locked_notes() {
    let outcome = hum_to_riff(
        &two_tone_hum(),
        SR,
        &grid(),
        &tempo(),
        &PipelineConfig::default(),
    )
    .expect("a clean two-tone hum analyses, quantizes, and renders end to end");

    assert!(
        !outcome.stem.samples.is_empty(),
        "the rendered riff is a non-empty buffer"
    );
    assert_eq!(
        outcome.notes.len(),
        2,
        "the two hummed tones become exactly two grid-locked notes: {:?}",
        outcome.notes
    );
    // Notes are time-ordered (R-0005), so [0] is the 220 Hz tone, [1] the 330 Hz.
    assert_eq!(
        outcome.notes[0].freq_hz, 220.0,
        "the first note snaps to the exact grid pitch 220 Hz (the unison)"
    );
    assert_eq!(
        outcome.notes[1].freq_hz, 330.0,
        "the second note snaps to the exact grid pitch 330 Hz (the fifth)"
    );
}

// ---------------------------------------------------------------------------
// AC2 — Loopable (bar-aligned) stem, exercising the multi-bar div_ceil path.
//
// The stem length is exactly a whole number of bars (`bars · bar_samples`),
// `bars >= 1` for a non-empty riff, and the length is a positive multiple of
// `bar_samples`. R-0007's long let-ring tails push the raw render past one
// 2.0 s bar, so `bars >= 2` — covering the `div_ceil` rounding-up path, not just
// the single-bar case.
// ---------------------------------------------------------------------------

#[test]
fn ac2_stem_is_bar_aligned_and_multi_bar() {
    let outcome = hum_to_riff(
        &two_tone_hum(),
        SR,
        &grid(),
        &tempo(),
        &PipelineConfig::default(),
    )
    .expect("the two-tone hum renders a bar-aligned stem");
    let stem = &outcome.stem;

    assert!(stem.bars >= 1, "a non-empty riff spans at least one bar");
    // R-0007 tails ring for several seconds, so the raw render exceeds one 2.0 s
    // bar and div_ceil rounds up past 1 — the multi-bar path SPEC-0008 §6 requires.
    assert!(
        stem.bars >= 2,
        "the long let-ring tails push the riff past one bar (got {} bars)",
        stem.bars
    );
    assert_eq!(
        stem.samples.len(),
        stem.bars as usize * BAR_SAMPLES,
        "the stem length is exactly bars · bar_samples ({} · {BAR_SAMPLES})",
        stem.bars
    );
    assert_eq!(
        stem.samples.len() % BAR_SAMPLES,
        0,
        "the stem length is a whole multiple of one bar, so the loop repeats on the downbeat"
    );
    assert!(
        !stem.samples.is_empty(),
        "the bar-aligned length is a positive multiple of bar_samples"
    );
}

// ---------------------------------------------------------------------------
// AC3 — Returns what it heard.
//
// The outcome exposes both the raw transcription (pitch track frames + onsets)
// and the grid-locked QuantizedNotes, alongside the stem; the quantized count
// reflects the hum (two tones → two notes).
// ---------------------------------------------------------------------------

#[test]
fn ac3_outcome_exposes_transcription_and_quantized_notes() {
    let outcome = hum_to_riff(
        &two_tone_hum(),
        SR,
        &grid(),
        &tempo(),
        &PipelineConfig::default(),
    )
    .expect("the two-tone hum produces a full outcome");

    // The raw transcription is exposed and non-trivial: it heard pitch and attacks.
    assert!(
        !outcome.transcription.notes.is_empty(),
        "the raw transcription carries the note events it heard"
    );
    assert!(
        !outcome.transcription.onsets.is_empty(),
        "the raw transcription exposes the detected onsets"
    );
    assert!(
        !outcome.transcription.pitch_track.frames.is_empty(),
        "the raw transcription exposes the pitch-track frames"
    );

    // The grid-locked notes are exposed and their count reflects the two-tone hum.
    assert_eq!(
        outcome.notes.len(),
        2,
        "the quantized notes reflect the two hummed tones: {:?}",
        outcome.notes
    );
}

// ---------------------------------------------------------------------------
// AC4 — Typed errors / input guard (the deferred R-0007 finiteness guard).
//
// Empty samples, a zero sample rate, a non-finite sample, and a buffer shorter
// than the analysis window each surface a typed DspError propagated from
// analyze — and the pipeline never panics. Validation order (transcribe.rs
// `validate`) is empty → zero-rate → non-finite → window-too-large, so each
// fixture isolates exactly one variant.
// ---------------------------------------------------------------------------

#[test]
fn ac4_empty_samples_is_empty_signal_error() {
    let err = hum_to_riff(&[], SR, &grid(), &tempo(), &PipelineConfig::default())
        .expect_err("empty samples must be rejected, not panic");
    assert_eq!(err, DspError::EmptySignal, "empty samples ⇒ EmptySignal");
}

#[test]
fn ac4_zero_sample_rate_is_invalid_sample_rate_error() {
    let err = hum_to_riff(
        &two_tone_hum(),
        0,
        &grid(),
        &tempo(),
        &PipelineConfig::default(),
    )
    .expect_err("a zero sample rate must be rejected, not panic");
    assert_eq!(
        err,
        DspError::InvalidSampleRate,
        "sample_rate == 0 ⇒ InvalidSampleRate"
    );
}

#[test]
fn ac4_nan_sample_is_non_finite_error() {
    let mut hum = two_tone_hum();
    hum[5_000] = f32::NAN;
    let err = hum_to_riff(&hum, SR, &grid(), &tempo(), &PipelineConfig::default())
        .expect_err("a NaN sample must be rejected, not panic");
    assert_eq!(
        err,
        DspError::NonFiniteSample,
        "a non-finite sample ⇒ NonFiniteSample"
    );
}

#[test]
fn ac4_too_short_buffer_is_window_too_large_error() {
    // A non-empty, finite buffer shorter than the default analysis window
    // (Config::default().window == 2048) cannot fill a single frame, so analyze
    // returns WindowTooLarge — propagated unchanged by the pipeline.
    let window = Config::default().window;
    let short = vec![0.1f32; window - 1];
    let err = hum_to_riff(&short, SR, &grid(), &tempo(), &PipelineConfig::default())
        .expect_err("a buffer shorter than the analysis window must be rejected, not panic");
    assert_eq!(
        err,
        DspError::WindowTooLarge,
        "signal shorter than the window ⇒ WindowTooLarge"
    );
}

// ---------------------------------------------------------------------------
// AC5 — Deterministic.
//
// Every stage is deterministic (R-0005 YIN, R-0006 snap, R-0007 fixed-seed
// pluck), so the same inputs yield an identical RiffOutcome — asserted on the
// whole struct via the PartialEq derive (covers stem samples AND notes AND
// transcription in one comparison).
// ---------------------------------------------------------------------------

#[test]
fn ac5_pipeline_is_deterministic() {
    let hum = two_tone_hum();
    let a: RiffOutcome = hum_to_riff(&hum, SR, &grid(), &tempo(), &PipelineConfig::default())
        .expect("first run succeeds");
    let b: RiffOutcome = hum_to_riff(&hum, SR, &grid(), &tempo(), &PipelineConfig::default())
        .expect("second run on identical inputs succeeds");

    assert_eq!(
        a, b,
        "identical inputs produce an identical outcome (stem samples, notes, and transcription)"
    );
}

// ---------------------------------------------------------------------------
// AC6 — Bounded, clean audio (inherited from the renderer).
//
// Every stem sample is finite and within [-1, 1] (a small epsilon absorbs the
// distortion's full-scale rounding); the silent bar-alignment padding is 0.0,
// trivially in range.
// ---------------------------------------------------------------------------

#[test]
fn ac6_stem_samples_are_bounded_and_finite() {
    let outcome = hum_to_riff(
        &two_tone_hum(),
        SR,
        &grid(),
        &tempo(),
        &PipelineConfig::default(),
    )
    .expect("the two-tone hum renders a bounded stem");

    assert!(
        !outcome.stem.samples.is_empty(),
        "precondition: a non-empty stem to bound-check"
    );
    for (i, &s) in outcome.stem.samples.iter().enumerate() {
        assert!(
            s.is_finite(),
            "stem sample {i} is finite (no NaN/inf), got {s}"
        );
        assert!(
            s.abs() <= 1.0 + 1e-6,
            "stem sample {i} stays within [-1, 1], got {s}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC7 — By-ear demo, documented public API, four toolchain gates.
//
// The `cargo run -p gooz-studio` demo (record ~4 s of humming → hum_to_riff →
// loop-play) is verified by ear on a real machine, not in CI. Every public item
// carries a runnable doc example, and all four gates (cargo build / test /
// clippy -D warnings / fmt --check) are green. These are checked at QA sign-off
// (loop step 7), not by an integration test here. There is intentionally no AC7
// test fn; the pure-pipeline cases above (AC1–AC6) are the CI-testable half of
// AC7 ("the pure pipeline is covered by tests").
// ---------------------------------------------------------------------------

// Compile-time pin: keep the imported public surface referenced even if a future
// edit drops its last in-test use, so a rename surfaces here as a hard error.
#[allow(dead_code)]
fn _type_surface_is_present(o: &RiffOutcome) -> (&RiffStem, &Vec<f32>, u32, u32) {
    (&o.stem, &o.stem.samples, o.stem.sample_rate, o.stem.bars)
}
