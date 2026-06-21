//! Acceptance tests for R-0007 — instrument render v0 (Karplus-Strong guitar +
//! distortion), realized by SPEC-0007.
//!
//! TDD red: this file is written **before** the implementation exists. It is the
//! executable contract that the new `gooz-synth` modules (`Distortion`,
//! `RenderConfig`, `render_notes`) and the re-exports they depend on must
//! satisfy. Until those items land, the crate does not name these symbols and
//! this file will not compile — the intended red state for loop step 3.
//!
//! **No microphone, no device, no ears.** Every `QuantizedNote` here is a
//! hand-constructed literal, `render_notes` is a pure offline function, and the
//! pluck excitation is a fixed-seed LCG (SPEC-0007 §2), so every assertion is
//! reproducible. Pitch is verified by autocorrelation, decay by windowed energy,
//! distortion by the exact waveshaper math — never by listening.
//!
//! One section per acceptance criterion (AC1–AC7); every test fn is prefixed
//! with the criterion id it verifies.
//!
//! ## AC → test mapping
//!
//! | AC  | Verified by                                                          |
//! |-----|----------------------------------------------------------------------|
//! | AC1 | `ac1_single_note_is_in_tune`                                         |
//! | AC2 | `ac2_plucked_note_decays`                                            |
//! | AC3 | `ac3_silence_before_onset_then_sound`                                |
//! | AC4 | `ac4_notes_let_ring_and_sum`                                         |
//! | AC5 | `ac5_softclip_clean_at_low_drive`,                                   |
//! |     | `ac5_softclip_saturates_and_is_bounded`,                             |
//! |     | `ac5_hardclip_identity_at_drive_one`,                                |
//! |     | `ac5_hardclip_clamps_and_is_bounded`,                                |
//! |     | `ac5_both_modes_keep_output_bounded`,                                |
//! |     | `ac5_softclip_and_hardclip_render_differently`,                      |
//! |     | `ac5_rendered_buffer_is_bounded`                                     |
//! | AC6 | `ac6_render_is_deterministic`,                                       |
//! |     | `ac6_empty_input_yields_empty_buffer`,                               |
//! |     | `ac6_zero_sample_rate_yields_empty_buffer`,                          |
//! |     | `ac6_all_skipped_notes_yield_empty_buffer`,                          |
//! |     | `ac6_non_positive_frequency_is_skipped`,                             |
//! |     | `ac6_non_finite_frequency_is_skipped`,                               |
//! |     | `ac6_rendered_buffer_has_no_nan_or_inf`                              |
//! | AC7 | verified at QA sign-off (doc examples + clippy + fmt + build),       |
//! |     | not by a CI test in this file — see the AC7 note below.              |
//!
//! ## API assumptions (flagged for the implementation step)
//!
//! SPEC-0007 §2/§3 fix every type, field, and function these tests call. Two
//! re-export requirements are surfaced here because the public `render_notes`
//! signature is otherwise un-callable by an external crate:
//!
//! * `render_notes(notes: &[QuantizedNote], …)` takes a slice of `gooz-dsp`'s
//!   `QuantizedNote`. A test in `gooz-synth/tests/` can only name `gooz-synth`'s
//!   public surface plus `gooz-synth`'s own deps — it cannot reach through to
//!   `gooz-dsp` unless `gooz-synth` **re-exports** the input type. So this suite
//!   assumes `gooz_synth::QuantizedNote` exists (re-exported from `gooz-dsp`).
//! * Building a `QuantizedNote` literal needs its `degree: Ratio` field, so this
//!   suite also assumes `gooz_synth::Ratio` is re-exported (`Ratio::UNISON`).
//!
//!   If either re-export is missing the test will not compile — that is a valid
//!   red signal for a real API-completeness gap: without these re-exports no
//!   external caller can construct the input to the public `render_notes`.
//!
//! * `Distortion` is `#[derive(Debug, Clone, Copy, PartialEq)]` with
//!   `fn apply(self, x: f32, drive: f32) -> f32` (SPEC-0007 §2). `RenderConfig`
//!   has public fields `decay: f32, distortion: Distortion, drive: f32` and a
//!   `Default` of `{ decay: 0.996, distortion: SoftClip, drive: 2.0 }`.

use gooz_synth::{Distortion, QuantizedNote, Ratio, RenderConfig, render_notes};

// ---------------------------------------------------------------------------
// Shared harness — sample rate and a QuantizedNote literal builder.
// ---------------------------------------------------------------------------

/// The pinned render sample rate (48 kHz, the project's working rate).
const SR: u32 = 48_000;

/// A bound on numeric noise for "is this sample silent" checks.
const SILENCE_EPS: f32 = 1e-6;

/// A bound for "is this sample inside [-1, 1]" checks (a hair of slack for the
/// tanh / clamp arithmetic at the extremes).
const BOUND_EPS: f32 = 1e-6;

/// Build a `QuantizedNote` at a given frequency, onset, and duration.
///
/// Only `freq_hz`, `onset_secs`, and `duration_secs` matter to the renderer
/// (SPEC-0007 §2 reads `freq_hz` for tuning and `onset_secs` for placement; the
/// grid `duration_secs` is *not* the render length — let-ring uses the natural
/// decay tail). The pitch-class fields are filled with valid placeholders.
fn note(freq_hz: f64, onset_secs: f64, duration_secs: f64) -> QuantizedNote {
    QuantizedNote {
        degree: Ratio::UNISON,
        octave: 0,
        freq_hz,
        cents_offset: 0.0,
        onset_step: 0,
        onset_secs,
        duration_secs,
    }
}

/// `RMS` energy proxy over a window — sum of squares (finite, non-negative).
fn window_energy(buf: &[f32], range: std::ops::Range<usize>) -> f64 {
    buf[range].iter().map(|&x| (x as f64) * (x as f64)).sum()
}

/// Autocorrelation of `buf[region]` at integer `lag`: Σ s[i]·s[i+lag].
///
/// A periodic signal of period `p` peaks at `lag == p` (and its multiples). Used
/// to recover the rendered fundamental's period without an FFT.
fn autocorr(samples: &[f32], lag: usize) -> f64 {
    let n = samples.len();
    if lag >= n {
        return 0.0;
    }
    (0..n - lag)
        .map(|i| (samples[i] as f64) * (samples[i + lag] as f64))
        .sum()
}

/// The lag in `min_lag..=max_lag` that maximizes autocorrelation of `samples`
/// — i.e. the estimated fundamental period in samples.
fn dominant_period(samples: &[f32], min_lag: usize, max_lag: usize) -> usize {
    let mut best_lag = min_lag;
    let mut best = f64::NEG_INFINITY;
    for lag in min_lag..=max_lag {
        let r = autocorr(samples, lag);
        if r > best {
            best = r;
            best_lag = lag;
        }
    }
    best_lag
}

// ---------------------------------------------------------------------------
// AC1 — In tune. A single note in the integer-tuned band renders a tone whose
// fundamental period (by autocorrelation) is ≈ sample_rate / f within ~1 %.
// Pinned at 440 Hz: n = round(48000 / 440) = 109 samples.
// ---------------------------------------------------------------------------

#[test]
fn ac1_single_note_is_in_tune() {
    // 440 Hz sits safely inside the integer-tuned band (n = 109 ≫ 48). Render it
    // alone, then estimate the period over a steady, post-attack region.
    let buf = render_notes(&[note(440.0, 0.0, 0.5)], SR, &RenderConfig::default());
    assert!(
        buf.len() >= 10_000,
        "a 0.5 s-ish pluck at 48 kHz rings out well past 10k samples, got {}",
        buf.len()
    );

    // A steady region away from the very first transient.
    let region = &buf[2_000..10_000];

    // Period candidates around 109: 1:1 grid frequency = 48000/440 = 109.09…
    let expected = (SR as f64 / 440.0).round() as usize; // 109
    let lag = dominant_period(region, 20, 400);

    let tolerance = 2; // ±2 samples ≈ ±1.8 % of 109 (within the ~1 % AC band)
    assert!(
        lag.abs_diff(expected) <= tolerance,
        "autocorrelation period {lag} must be ≈ sample_rate/f = {expected} (±{tolerance}); \
         the string is tuned to 440 Hz"
    );
}

// ---------------------------------------------------------------------------
// AC2 — Plucked decay. Within one rendered note, energy in a late window is
// strictly less than in an early window (a pluck that rings down, not a drone).
// ---------------------------------------------------------------------------

#[test]
fn ac2_plucked_note_decays() {
    let buf = render_notes(&[note(440.0, 0.0, 0.5)], SR, &RenderConfig::default());
    assert!(
        buf.len() >= 22_000,
        "need at least 22k samples to compare early vs late windows, got {}",
        buf.len()
    );

    let early = window_energy(&buf, 200..2_000);
    let late = window_energy(&buf, 20_000..22_000);

    assert!(
        early.is_finite() && late.is_finite(),
        "window energies must be finite: early={early}, late={late}"
    );
    assert!(
        early > 0.0,
        "the early window must carry the pluck's energy, got {early}"
    );
    assert!(
        late < early,
        "a plucked string decays: late energy {late} must be strictly less than \
         early energy {early}"
    );
}

// ---------------------------------------------------------------------------
// AC3 — Onset placement. Output before the first note's onset sample is silence;
// the note produces non-zero output from its onset. Onset 0.25 s → sample 12000.
// ---------------------------------------------------------------------------

#[test]
fn ac3_silence_before_onset_then_sound() {
    let onset_secs = 0.25;
    let onset_sample = (onset_secs * SR as f64).round() as usize; // 12_000
    let buf = render_notes(
        &[note(440.0, onset_secs, 0.5)],
        SR,
        &RenderConfig::default(),
    );
    assert!(
        buf.len() > onset_sample,
        "buffer must extend past the onset sample {onset_sample}, got {}",
        buf.len()
    );

    // Everything strictly before the onset is silence.
    for (i, &x) in buf[..onset_sample].iter().enumerate() {
        assert!(
            x.abs() < SILENCE_EPS,
            "sample {i} before onset {onset_sample} must be silent, got {x}"
        );
    }

    // There is real energy at/after the onset.
    let after = window_energy(&buf, onset_sample..buf.len());
    assert!(
        after > 0.0,
        "the note must produce non-zero output from its onset onward, got energy {after}"
    );
}

// ---------------------------------------------------------------------------
// AC4 — Let-ring mix. Multiple notes mix into one buffer; each rings its natural
// decay past its grid duration; tails sum. Note A: 220 Hz, onset 0.0,
// duration 0.1 (grid end = 4800 samples). Note B: 330 Hz, onset 0.3 (14400).
// ---------------------------------------------------------------------------

#[test]
fn ac4_notes_let_ring_and_sum() {
    let a = note(220.0, 0.0, 0.1); // grid end = 0.1 s = 4800 samples
    let b = note(330.0, 0.3, 0.1); // onset = 0.3 s = 14400 samples
    let a_grid_end = (0.1 * SR as f64).round() as usize; // 4_800
    let b_onset = (0.3 * SR as f64).round() as usize; // 14_400

    let buf = render_notes(&[a, b], SR, &RenderConfig::default());

    // (a) Note A rings PAST its grid duration: there is energy in a window that
    // begins after A's grid end (4800) and ends before B enters (14400). This is
    // the "let ring" property — the render length is the decay tail, not the grid
    // duration.
    assert!(
        6_000 > a_grid_end && 8_000 < b_onset,
        "precondition: window [6000..8000] is past A's grid end ({a_grid_end}) and \
         before B's onset ({b_onset})"
    );
    let a_tail_energy = window_energy(&buf, 6_000..8_000);
    assert!(
        a_tail_energy > 0.0,
        "note A must still ring in [6000..8000], past its grid duration end \
         ({a_grid_end}); got energy {a_tail_energy}"
    );

    // (b) The buffer spans the second note: it extends past B's onset (and B
    // itself rings out a tail beyond that).
    assert!(
        buf.len() > b_onset,
        "buffer length {} must span the last note's onset {b_onset} plus its tail",
        buf.len()
    );

    // (c) The total length is well beyond B's onset — B's decay tail is included.
    assert!(
        buf.len() > b_onset + 1_000,
        "buffer length {} must extend well past B's onset {b_onset} (B's let-ring tail)",
        buf.len()
    );
}

// ---------------------------------------------------------------------------
// AC5 — Distortion FX. Both modes available; each alters the signal; higher
// drive saturates more; output bounded in [-1, 1]. SoftClip ≈ identity at low
// drive; HardClip is exactly identity at drive = 1.0. The rendered buffer is
// bounded. Distortion::apply is tested directly; render-level bounds via
// render_notes.
// ---------------------------------------------------------------------------

#[test]
fn ac5_softclip_clean_at_low_drive() {
    // SoftClip tanh(d·x)/tanh(d) → x as d → 0. At a tiny drive it is ≈ identity.
    let y = Distortion::SoftClip.apply(0.5, 0.001);
    assert!(
        (y - 0.5).abs() < 1e-2,
        "SoftClip at very low drive ≈ identity: apply(0.5, 0.001) = {y} ≈ 0.5"
    );
}

#[test]
fn ac5_softclip_saturates_and_is_bounded() {
    // At a high drive SoftClip compresses toward ±1: a mid-level input is pushed
    // UP (saturating), differs from the input, and stays bounded.
    let saturated = Distortion::SoftClip.apply(0.5, 8.0);
    assert!(
        saturated > 0.5,
        "high-drive SoftClip pushes 0.5 up toward 1.0 (saturation), got {saturated}"
    );
    assert!(
        saturated <= 1.0 + BOUND_EPS,
        "SoftClip output stays within [-1, 1], got {saturated}"
    );

    // A near-full-scale input clearly differs from itself once driven hard.
    let driven = Distortion::SoftClip.apply(0.8, 8.0);
    assert!(
        (driven - 0.8).abs() > 1e-3,
        "high-drive SoftClip changes the signal: apply(0.8, 8.0) = {driven} != 0.8"
    );

    // Bounded across the whole input range at a representative drive.
    for k in -10..=10 {
        let x = k as f32 / 10.0; // -1.0 ..= 1.0
        let y = Distortion::SoftClip.apply(x, 2.0);
        assert!(
            y.abs() <= 1.0 + BOUND_EPS,
            "SoftClip(x={x}, drive=2.0) = {y} must lie within [-1, 1]"
        );
        assert!(
            y.is_finite(),
            "SoftClip(x={x}, drive=2.0) = {y} must be finite"
        );
    }
}

#[test]
fn ac5_hardclip_identity_at_drive_one() {
    // HardClip is (d·x).clamp(-1, 1); at d = 1.0 it is exactly the identity for
    // |x| <= 1. These are exact in f32, so assert_eq! on pinned values.
    assert_eq!(
        Distortion::HardClip.apply(0.5, 1.0),
        0.5,
        "HardClip is identity at drive 1.0"
    );
    assert_eq!(
        Distortion::HardClip.apply(-0.5, 1.0),
        -0.5,
        "HardClip is identity at drive 1.0 (negative)"
    );
    assert_eq!(
        Distortion::HardClip.apply(0.0, 1.0),
        0.0,
        "HardClip is identity at drive 1.0 (zero)"
    );
    assert_eq!(
        Distortion::HardClip.apply(1.0, 1.0),
        1.0,
        "HardClip is identity at drive 1.0 (positive edge)"
    );
    assert_eq!(
        Distortion::HardClip.apply(-1.0, 1.0),
        -1.0,
        "HardClip is identity at drive 1.0 (negative edge)"
    );
}

#[test]
fn ac5_hardclip_clamps_and_is_bounded() {
    // Higher drive clamps: 0.5 · 4.0 = 2.0 → clamped to 1.0.
    assert_eq!(
        Distortion::HardClip.apply(0.5, 4.0),
        1.0,
        "HardClip at drive 4.0 clamps 0.5 (→ 2.0) to 1.0"
    );
    assert_eq!(
        Distortion::HardClip.apply(-0.5, 4.0),
        -1.0,
        "HardClip at drive 4.0 clamps -0.5 (→ -2.0) to -1.0"
    );

    // Higher drive saturates more than drive 1.0 for a mid input (identity there).
    let low = Distortion::HardClip.apply(0.5, 1.0);
    let high = Distortion::HardClip.apply(0.5, 4.0);
    assert!(
        high > low,
        "higher drive saturates more: HardClip(0.5, 4.0)={high} > HardClip(0.5, 1.0)={low}"
    );
}

#[test]
fn ac5_both_modes_keep_output_bounded() {
    // For |x| <= 1, both modes keep |output| <= 1 across several drives.
    let drives = [0.5_f32, 1.0, 2.0, 4.0, 8.0];
    for mode in [Distortion::SoftClip, Distortion::HardClip] {
        for &drive in &drives {
            for k in -10..=10 {
                let x = k as f32 / 10.0; // -1.0 ..= 1.0
                let y = mode.apply(x, drive);
                assert!(
                    y.is_finite(),
                    "{mode:?}.apply(x={x}, drive={drive}) = {y} must be finite"
                );
                assert!(
                    y.abs() <= 1.0 + BOUND_EPS,
                    "{mode:?}.apply(x={x}, drive={drive}) = {y} must lie within [-1, 1]"
                );
            }
        }
    }
}

#[test]
fn ac5_softclip_and_hardclip_render_differently() {
    // The same notes rendered through SoftClip vs HardClip yield different
    // buffers — both modes are wired into render_notes and actually shape it.
    let notes = [note(220.0, 0.0, 0.3), note(330.0, 0.2, 0.3)];
    let soft = render_notes(
        &notes,
        SR,
        &RenderConfig {
            decay: 0.996,
            distortion: Distortion::SoftClip,
            drive: 4.0,
        },
    );
    let hard = render_notes(
        &notes,
        SR,
        &RenderConfig {
            decay: 0.996,
            distortion: Distortion::HardClip,
            drive: 4.0,
        },
    );

    assert_eq!(
        soft.len(),
        hard.len(),
        "the two distortion modes do not change the render length"
    );
    assert!(
        soft != hard,
        "SoftClip and HardClip must shape the same notes into different buffers"
    );
}

#[test]
fn ac5_rendered_buffer_is_bounded() {
    // The final rendered buffer never exceeds [-1, 1] (SPEC-0007 §2 step 3
    // normalizes to full scale before the bounded waveshaper). Check both modes.
    for mode in [Distortion::SoftClip, Distortion::HardClip] {
        let buf = render_notes(
            &[note(110.0, 0.0, 0.5), note(220.0, 0.1, 0.5)],
            SR,
            &RenderConfig {
                decay: 0.996,
                distortion: mode,
                drive: 6.0,
            },
        );
        for (i, &x) in buf.iter().enumerate() {
            assert!(
                x.abs() <= 1.0 + BOUND_EPS,
                "{mode:?}: rendered sample {i} = {x} must lie within [-1, 1]"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// AC6 — Deterministic & robust. Identical input → identical buffer; empty input
// or zero sample rate → empty; all-skipped → empty; bad-frequency notes skipped;
// no NaN/inf; library paths never panic.
// ---------------------------------------------------------------------------

#[test]
fn ac6_render_is_deterministic() {
    // Fixed-seed excitation (SPEC-0007 §2) ⇒ the same notes + config render the
    // exact same buffer, sample-for-sample. Not a frozen golden — a re-render
    // equality (SPEC-0007 §6 AC6).
    let notes = [
        note(220.0, 0.0, 0.3),
        note(330.0, 0.25, 0.3),
        note(440.0, 0.5, 0.3),
    ];
    let cfg = RenderConfig::default();
    let a = render_notes(&notes, SR, &cfg);
    let b = render_notes(&notes, SR, &cfg);

    assert_eq!(
        a, b,
        "render_notes is deterministic: identical input → identical buffer"
    );
}

#[test]
fn ac6_empty_input_yields_empty_buffer() {
    let buf = render_notes(&[], SR, &RenderConfig::default());
    assert!(buf.is_empty(), "empty note slice renders an empty buffer");
}

#[test]
fn ac6_zero_sample_rate_yields_empty_buffer() {
    let buf = render_notes(&[note(440.0, 0.0, 0.5)], 0, &RenderConfig::default());
    assert!(buf.is_empty(), "a zero sample rate renders an empty buffer");
}

#[test]
fn ac6_all_skipped_notes_yield_empty_buffer() {
    // A non-empty slice where every note is skipped (bad frequency) yields an
    // empty buffer (SPEC-0007 §2 step 1).
    let buf = render_notes(&[note(0.0, 0.0, 0.5)], SR, &RenderConfig::default());
    assert!(
        buf.is_empty(),
        "a slice whose only note is skipped renders an empty buffer, got len {}",
        buf.len()
    );
}

#[test]
fn ac6_non_positive_frequency_is_skipped() {
    // Zero and negative frequencies cannot tune a string → skipped, never a
    // panic. A valid note alongside them still renders.
    let bad_zero = render_notes(&[note(0.0, 0.0, 0.5)], SR, &RenderConfig::default());
    assert!(bad_zero.is_empty(), "a 0 Hz note is skipped → empty buffer");

    let bad_negative = render_notes(&[note(-110.0, 0.0, 0.5)], SR, &RenderConfig::default());
    assert!(
        bad_negative.is_empty(),
        "a negative-frequency note is skipped → empty buffer"
    );

    // Mixed: the valid note survives, the bad ones are dropped.
    let mixed = render_notes(
        &[
            note(0.0, 0.0, 0.5),
            note(440.0, 0.0, 0.5),
            note(-1.0, 0.0, 0.5),
        ],
        SR,
        &RenderConfig::default(),
    );
    assert!(
        !mixed.is_empty(),
        "the single valid note among bad-frequency notes still renders"
    );
}

#[test]
fn ac6_non_finite_frequency_is_skipped() {
    // NaN, +inf, and -inf frequencies are non-finite → skipped, never a panic.
    let nan = render_notes(&[note(f64::NAN, 0.0, 0.5)], SR, &RenderConfig::default());
    assert!(
        nan.is_empty(),
        "a NaN-frequency note is skipped → empty buffer"
    );

    let pos_inf = render_notes(
        &[note(f64::INFINITY, 0.0, 0.5)],
        SR,
        &RenderConfig::default(),
    );
    assert!(
        pos_inf.is_empty(),
        "a +inf-frequency note is skipped → empty buffer"
    );

    let neg_inf = render_notes(
        &[note(f64::NEG_INFINITY, 0.0, 0.5)],
        SR,
        &RenderConfig::default(),
    );
    assert!(
        neg_inf.is_empty(),
        "a -inf-frequency note is skipped → empty buffer"
    );
}

#[test]
fn ac6_rendered_buffer_has_no_nan_or_inf() {
    // No rendered sample is NaN or inf — inputs are finite and every op is finite
    // (SPEC-0007 §2 step 5).
    let notes = [
        note(110.0, 0.0, 0.5),
        note(220.0, 0.1, 0.5),
        note(440.0, 0.3, 0.5),
    ];
    let buf = render_notes(&notes, SR, &RenderConfig::default());
    assert!(
        !buf.is_empty(),
        "precondition: the valid notes render audio"
    );
    for (i, &x) in buf.iter().enumerate() {
        assert!(
            x.is_finite(),
            "rendered sample {i} = {x} must be finite (no NaN/inf)"
        );
    }
}

// ---------------------------------------------------------------------------
// AC7 — Documented public API & four toolchain gates.
//
// Every public item (Distortion, RenderConfig, render_notes, and the
// re-exports) carries a runnable doc example, and all four gates (cargo build /
// test / clippy -D warnings / fmt --check) are green. These are verified at QA
// sign-off (loop step 7) by running the gates and the doc tests — not by an
// integration test here. There is intentionally no AC7 test fn; AC1–AC6 are the
// "behaviour covered by tests" half of AC7.
// ---------------------------------------------------------------------------

// Compile-time pin: keep the imported public surface referenced even if a future
// edit drops its last in-test use, so a rename surfaces here as a hard error.
#[allow(dead_code)]
fn _public_surface_is_present(cfg: &RenderConfig) -> (f32, Distortion, f32) {
    (cfg.decay, cfg.distortion, cfg.drive)
}
