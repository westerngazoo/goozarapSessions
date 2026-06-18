//! Acceptance tests for R-0005 — pitch tracking & onset detection
//! (note transcription), realized by SPEC-0005.
//!
//! TDD red: this file is written **before** the implementation exists. It is
//! the executable contract that the new `gooz-dsp` items (`DspError`, `Config`,
//! `PitchFrame`, `PitchTrack`, `Onset`, `NoteEvent`, `Transcription`, and the
//! `analyze` / `pitch_track` / `detect_onsets` functions) must satisfy. Until
//! those types land the crate is a stub and this file will not compile — that
//! is the intended red state for loop step 3.
//!
//! **No microphone, no device.** Every signal here is synthesized in-process
//! from deterministic helpers (`sine`, `silence`, `noise`, concatenation). The
//! analysis is pure (`&[f32]` + sample rate), so the suite is fully
//! reproducible on golden signals (R-0005 §2, SPEC-0005 §6).
//!
//! One section per acceptance criterion (AC1–AC7); every test fn is prefixed
//! with the criterion id it verifies.
//!
//! ## AC → test mapping
//!
//! | AC  | Verified by                                                          |
//! |-----|----------------------------------------------------------------------|
//! | AC1 | `ac1_yin_tracks_220_within_one_percent`,                            |
//! |     | `ac1_yin_tracks_330_within_one_percent`,                            |
//! |     | `ac1_yin_tracks_440_within_one_percent`                             |
//! | AC2 | `ac2_silence_is_all_unvoiced`,                                      |
//! |     | `ac2_tone_is_majority_voiced`,                                      |
//! |     | `ac2_broadband_noise_is_mostly_unvoiced`,                           |
//! |     | `ac2_unvoiced_frames_carry_no_pitch`                                |
//! | AC3 | `ac3_three_bursts_yield_exactly_three_onsets`,                      |
//! |     | `ac3_onset_times_match_burst_starts`,                               |
//! |     | `ac3_steady_tone_yields_exactly_one_onset`,                         |
//! |     | `ac3_tone_starting_at_sample_zero_onsets_near_zero`                 |
//! | AC4 | `ac4_two_tone_signal_yields_exactly_two_notes`,                     |
//! |     | `ac4_note_pitches_match_within_one_percent`,                        |
//! |     | `ac4_note_onsets_match_within_tolerance`,                           |
//! |     | `ac4_notes_are_time_ordered_and_non_overlapping`,                   |
//! |     | `ac4_note_durations_are_positive`                                   |
//! | AC5 | `ac5_tone_above_f_max_yields_no_in_range_voiced_frame`,             |
//! |     | `ac5_tone_above_f_max_yields_no_note`,                              |
//! |     | `ac5_silence_only_signal_yields_zero_notes`,                        |
//! |     | `ac5_notes_are_sorted_by_onset`                                     |
//! | AC6 | `ac6_empty_signal_is_empty_signal_error`,                           |
//! |     | `ac6_zero_sample_rate_is_invalid_sample_rate_error`,                |
//! |     | `ac6_signal_shorter_than_window_is_window_too_large_error`,         |
//! |     | `ac6_nan_sample_is_non_finite_error`,                               |
//! |     | `ac6_infinite_sample_is_non_finite_error`,                          |
//! |     | `ac6_dsp_error_is_typed_std_error_with_display`,                    |
//! |     | `ac6_pitch_track_and_detect_onsets_reject_bad_input`               |
//! | AC7 | verified at QA sign-off (doc examples + clippy + fmt + build),       |
//! |     | not by a CI test in this file — see the AC7 note below.              |
//!
//! ## API assumptions beyond SPEC-0005 §2/§3
//!
//! SPEC-0005 §2/§3 fix every type, field, function, and error name these tests
//! call; nothing was inferred. The notes below pin the exact spelling and
//! semantics the tests rely on so any mismatch is caught at the implementation
//! step, not blamed on a test typo:
//!
//! * `DspError` is a fieldless enum (`EmptySignal`, `InvalidSampleRate`,
//!   `WindowTooLarge`, `NonFiniteSample`) deriving
//!   `Debug, Clone, Copy, PartialEq, Eq` and implementing `std::error::Error`
//!   + `Display` — verbatim spec §2 ("DspError (error.rs)").
//! * `Config` has public fields `window, hop, f_min, f_max, yin_threshold,
//!   fft_size, onset_sensitivity, onset_window_frames` with the spec §2
//!   defaults (`window 2048`, `hop 256`, `f_min 80.0`, `f_max 1000.0`,
//!   `yin_threshold 0.15`, `fft_size 1024`, `onset_sensitivity 0.3`,
//!   `onset_window_frames 8`) via `impl Default`. The tests rely on
//!   `Config::default()` except where a custom `f_max` is needed (AC5); they
//!   build custom configs with `..Config::default()` so they stay valid even
//!   if extra fields are added.
//! * `PitchFrame { time_secs: f64, f0_hz: Option<f32>, confidence: f32 }`;
//!   `f0_hz == None` ⇒ unvoiced (spec §2). `PitchTrack { frames: Vec<PitchFrame> }`.
//! * `Onset { time_secs: f64, strength: f32 }`.
//! * `NoteEvent { onset_secs: f64, pitch_hz: f32, duration_secs: f64 }`.
//! * `Transcription { pitch_track: PitchTrack, onsets: Vec<Onset>, notes: Vec<NoteEvent> }`.
//! * `analyze(signal: &[f32], sample_rate: u32, cfg: &Config) -> Result<Transcription, DspError>`
//!   is the primary entry (spec §2 "Assembly"); `pitch_track(...) -> Result<PitchTrack, DspError>`
//!   and `detect_onsets(...) -> Result<Vec<Onset>, DspError>` are the exposed
//!   intermediates (spec §2 "YIN" / "Onsets").
//! * Voicing semantics (spec §2 "YIN" step 5): a frame is voiced iff
//!   `d'(τ) < yin_threshold` AND `f0` is within `[f_min, f_max]`; otherwise
//!   `f0_hz` is `None`. The AC5 tests rely on the in-range clause.

use gooz_dsp::{Config, DspError, NoteEvent, Onset, PitchTrack, Transcription};

/// CD/voice sample rate all golden signals are generated at.
const SR: u32 = 48_000;

/// Pitch tolerance: ±1 % relative (R-0005 AC1/AC4, ≈17 cents).
const PITCH_TOL: f32 = 0.01;

/// Onset/note-start tolerance: ±20 ms (R-0005 AC3/AC4).
const TIME_TOL: f64 = 0.020;

// ---------------------------------------------------------------------------
// Golden-signal helpers (deterministic; no rng crate, no device).
// ---------------------------------------------------------------------------

/// A pure sine of `freq` Hz for `secs` seconds at `sr`, amplitude ~0.8.
/// Deterministic and allocation-simple — the YIN/onset golden source.
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

/// Deterministic broadband noise via a fixed-seed LCG (no external rng crate),
/// amplitude ~0.8. Used to assert noise is reported unvoiced (AC2).
fn noise(secs: f64, sr: u32) -> Vec<f32> {
    let n = (secs * sr as f64).round() as usize;
    let mut state: u64 = 0x2545_F491_4F6C_DD1D; // fixed seed → reproducible
    (0..n)
        .map(|_| {
            // Numerical Recipes LCG constants; map the high bits to [-0.8, 0.8].
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let unit = (state >> 40) as f32 / (1u64 << 24) as f32; // [0, 1)
            0.8 * (2.0 * unit - 1.0)
        })
        .collect()
}

/// Concatenate signal segments into one buffer (tones, gaps, lead-ins).
fn concat(segments: &[&[f32]]) -> Vec<f32> {
    let mut out = Vec::with_capacity(segments.iter().map(|s| s.len()).sum());
    for seg in segments {
        out.extend_from_slice(seg);
    }
    out
}

/// Median of a slice of `f32`, panicking with `msg` when empty — used only on
/// test-controlled, known-non-empty data to summarize a voiced pitch track.
fn median(xs: &[f32], msg: &str) -> f32 {
    assert!(!xs.is_empty(), "{msg}");
    let mut v: Vec<f32> = xs.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

/// The voiced `f0` values of a pitch track, in frame order.
fn voiced_f0(track: &PitchTrack) -> Vec<f32> {
    track.frames.iter().filter_map(|f| f.f0_hz).collect()
}

/// Assert `actual` is within `PITCH_TOL` (relative) of `expected`.
fn assert_pitch_close(actual: f32, expected: f32, what: &str) {
    let rel = (actual - expected).abs() / expected;
    assert!(
        rel <= PITCH_TOL,
        "{what}: detected {actual} Hz vs true {expected} Hz is {:.4} relative error, exceeds {PITCH_TOL}",
        rel
    );
}

// ---------------------------------------------------------------------------
// AC1 — Pitch accuracy (YIN within ±1 % on steady tones).
//
// For each known f0 the median of the voiced frames' f0 must land within 1 %.
// ---------------------------------------------------------------------------

fn assert_yin_tracks(f0: f32) {
    let cfg = Config::default();
    let signal = sine(f0, 0.5, SR);
    let track = gooz_dsp::pitch_track(&signal, SR, &cfg)
        .expect("pitch_track on a clean tone within the default range succeeds");

    let voiced = voiced_f0(&track);
    assert!(
        !voiced.is_empty(),
        "a steady {f0} Hz tone must produce at least one voiced frame"
    );
    let m = median(&voiced, "voiced frames exist");
    assert_pitch_close(m, f0, "AC1 median voiced f0");
}

#[test]
fn ac1_yin_tracks_220_within_one_percent() {
    assert_yin_tracks(220.0);
}

#[test]
fn ac1_yin_tracks_330_within_one_percent() {
    assert_yin_tracks(330.0);
}

#[test]
fn ac1_yin_tracks_440_within_one_percent() {
    assert_yin_tracks(440.0);
}

// ---------------------------------------------------------------------------
// AC2 — Voiced / unvoiced classification.
// ---------------------------------------------------------------------------

#[test]
fn ac2_silence_is_all_unvoiced() {
    let cfg = Config::default();
    let signal = silence(0.5, SR);
    let track = gooz_dsp::pitch_track(&signal, SR, &cfg)
        .expect("pitch_track over silence succeeds (it is just unvoiced)");

    assert!(
        !track.frames.is_empty(),
        "silence still yields frames, just unvoiced ones"
    );
    assert!(
        track.frames.iter().all(|f| f.f0_hz.is_none()),
        "every frame of pure silence is unvoiced (f0_hz == None)"
    );
}

#[test]
fn ac2_tone_is_majority_voiced() {
    let cfg = Config::default();
    let signal = sine(440.0, 0.5, SR);
    let track = gooz_dsp::pitch_track(&signal, SR, &cfg).expect("pitch_track on a clean tone");

    let voiced = track.frames.iter().filter(|f| f.f0_hz.is_some()).count();
    assert!(
        voiced * 2 > track.frames.len(),
        "a clear 440 Hz tone is voiced in the majority of frames ({voiced}/{})",
        track.frames.len()
    );
}

#[test]
fn ac2_broadband_noise_is_mostly_unvoiced() {
    let cfg = Config::default();
    let signal = noise(0.5, SR);
    let track =
        gooz_dsp::pitch_track(&signal, SR, &cfg).expect("pitch_track over deterministic noise");

    let voiced = track.frames.iter().filter(|f| f.f0_hz.is_some()).count();
    assert!(
        voiced * 2 < track.frames.len(),
        "broadband noise is unvoiced in the majority of frames ({voiced}/{} voiced)",
        track.frames.len()
    );
}

#[test]
fn ac2_unvoiced_frames_carry_no_pitch() {
    // A signal with a clear silent region: the silent frames must be unvoiced,
    // and no unvoiced frame may carry a pitch (None is the only "no pitch"
    // representation per spec §2). This is the invariant note events rely on.
    let cfg = Config::default();
    let signal = concat(&[&silence(0.3, SR), &sine(440.0, 0.3, SR), &silence(0.3, SR)]);
    let track = gooz_dsp::pitch_track(&signal, SR, &cfg).expect("pitch_track on tone-with-silence");

    // The invariant: "unvoiced" and "no pitch" are the same thing — a frame is
    // either voiced-with-Some(f0) or unvoiced-with-None.
    for frame in &track.frames {
        match frame.f0_hz {
            Some(f0) => assert!(
                f0.is_finite() && f0 > 0.0,
                "a voiced frame carries a finite positive pitch, got {f0}"
            ),
            None => { /* unvoiced ⇒ no pitch, as required */ }
        }
    }

    // And there must be both kinds present in this mixed signal.
    assert!(
        track.frames.iter().any(|f| f.f0_hz.is_some()),
        "the tone region must produce voiced frames"
    );
    assert!(
        track.frames.iter().any(|f| f.f0_hz.is_none()),
        "the silent regions must produce unvoiced frames"
    );
}

// ---------------------------------------------------------------------------
// AC3 — Onset detection.
//
// Golden corpus note (SPEC-0005 §6): a short silent lead-in gives the first
// attack a genuine rising edge. One test additionally starts a tone at sample 0
// to exercise the implicit-zero-frame rule (spec §2 "Onsets" step 2).
// ---------------------------------------------------------------------------

/// 50 ms silent lead-in before the first attack (golden-corpus convention).
const LEAD_IN: f64 = 0.050;

#[test]
fn ac3_three_bursts_yield_exactly_three_onsets() {
    let cfg = Config::default();
    let burst = || sine(440.0, 0.20, SR);
    let gap = || silence(0.15, SR);
    let signal = concat(&[
        &silence(LEAD_IN, SR),
        &burst(),
        &gap(),
        &burst(),
        &gap(),
        &burst(),
    ]);

    let onsets = gooz_dsp::detect_onsets(&signal, SR, &cfg)
        .expect("detect_onsets on three separated bursts");
    assert_eq!(
        onsets.len(),
        3,
        "exactly K=3 bursts must yield exactly 3 onsets, no spurious extras: {onsets:?}"
    );
}

#[test]
fn ac3_onset_times_match_burst_starts() {
    let cfg = Config::default();
    let burst = 0.20; // s
    let gap = 0.15; // s
    let signal = concat(&[
        &silence(LEAD_IN, SR),
        &sine(440.0, burst, SR),
        &silence(gap, SR),
        &sine(440.0, burst, SR),
        &silence(gap, SR),
        &sine(440.0, burst, SR),
    ]);

    let onsets = gooz_dsp::detect_onsets(&signal, SR, &cfg).expect("detect_onsets on three bursts");
    assert_eq!(onsets.len(), 3, "precondition: exactly three onsets");

    // True burst starts: lead-in, then each burst follows the previous
    // burst+gap.
    let expected = [
        LEAD_IN,
        LEAD_IN + burst + gap,
        LEAD_IN + 2.0 * (burst + gap),
    ];
    let mut times: Vec<f64> = onsets.iter().map(|o| o.time_secs).collect();
    times.sort_by(|a, b| a.total_cmp(b));

    for (got, want) in times.iter().zip(expected.iter()) {
        assert!(
            (got - want).abs() <= TIME_TOL,
            "onset at {got} s must be within {TIME_TOL} s of true start {want} s"
        );
    }
}

#[test]
fn ac3_steady_tone_yields_exactly_one_onset() {
    let cfg = Config::default();
    // Lead-in then a long steady tone: exactly one onset (its start), not a
    // stream of spurious ones across the sustain.
    let signal = concat(&[&silence(LEAD_IN, SR), &sine(440.0, 0.6, SR)]);

    let onsets =
        gooz_dsp::detect_onsets(&signal, SR, &cfg).expect("detect_onsets on a single steady tone");
    assert_eq!(
        onsets.len(),
        1,
        "a single steady tone yields exactly one onset, not a stream: {onsets:?}"
    );
    assert!(
        (onsets[0].time_secs - LEAD_IN).abs() <= TIME_TOL,
        "the lone onset is at the tone start ({} s vs {LEAD_IN} s)",
        onsets[0].time_secs
    );
}

#[test]
fn ac3_tone_starting_at_sample_zero_onsets_near_zero() {
    // No lead-in: energy is present in frame 0. The implicit all-zero frame
    // before the first (spec §2 "Onsets" step 2) makes this register as a
    // single onset at t ≈ 0.
    let cfg = Config::default();
    let signal = sine(440.0, 0.6, SR);

    let onsets = gooz_dsp::detect_onsets(&signal, SR, &cfg)
        .expect("detect_onsets on a tone that starts at sample 0");
    assert_eq!(
        onsets.len(),
        1,
        "a tone present from sample 0 still yields exactly one onset: {onsets:?}"
    );
    assert!(
        onsets[0].time_secs <= TIME_TOL,
        "the sample-0 attack onsets at t ≈ 0 ({} s)",
        onsets[0].time_secs
    );
}

// ---------------------------------------------------------------------------
// AC4 — Note-event assembly (the headline output).
//
// Two-tone signal: 220 Hz for 0.4 s, an 80 ms gap, 330 Hz for 0.4 s, after a
// lead-in. Must transcribe to exactly two ordered, non-overlapping notes with
// the right pitch and onset.
// ---------------------------------------------------------------------------

const TONE_A_HZ: f32 = 220.0;
const TONE_B_HZ: f32 = 330.0;
const TONE_DUR: f64 = 0.40;
const NOTE_GAP: f64 = 0.080;

/// The shared two-tone golden signal for the AC4 suite.
fn two_tone_signal() -> Vec<f32> {
    concat(&[
        &silence(LEAD_IN, SR),
        &sine(TONE_A_HZ, TONE_DUR, SR),
        &silence(NOTE_GAP, SR),
        &sine(TONE_B_HZ, TONE_DUR, SR),
    ])
}

/// True onset times of the two tones in `two_tone_signal`.
fn two_tone_true_onsets() -> [f64; 2] {
    [LEAD_IN, LEAD_IN + TONE_DUR + NOTE_GAP]
}

#[test]
fn ac4_two_tone_signal_yields_exactly_two_notes() {
    let cfg = Config::default();
    let t = gooz_dsp::analyze(&two_tone_signal(), SR, &cfg).expect("analyze on a two-tone signal");
    assert_eq!(
        t.notes.len(),
        2,
        "a two-tone (A, gap, B) signal transcribes to exactly two notes: {:?}",
        t.notes
    );
}

#[test]
fn ac4_note_pitches_match_within_one_percent() {
    let cfg = Config::default();
    let t = gooz_dsp::analyze(&two_tone_signal(), SR, &cfg).expect("analyze on a two-tone signal");
    assert_eq!(t.notes.len(), 2, "precondition: exactly two notes");

    // Notes are time-ordered (AC5), so [0] is the 220 Hz tone, [1] the 330 Hz.
    assert_pitch_close(t.notes[0].pitch_hz, TONE_A_HZ, "AC4 note 0 pitch");
    assert_pitch_close(t.notes[1].pitch_hz, TONE_B_HZ, "AC4 note 1 pitch");
}

#[test]
fn ac4_note_onsets_match_within_tolerance() {
    let cfg = Config::default();
    let t = gooz_dsp::analyze(&two_tone_signal(), SR, &cfg).expect("analyze on a two-tone signal");
    assert_eq!(t.notes.len(), 2, "precondition: exactly two notes");

    let [want0, want1] = two_tone_true_onsets();
    assert!(
        (t.notes[0].onset_secs - want0).abs() <= TIME_TOL,
        "note 0 onset {} s within {TIME_TOL} s of true start {want0} s",
        t.notes[0].onset_secs
    );
    assert!(
        (t.notes[1].onset_secs - want1).abs() <= TIME_TOL,
        "note 1 onset {} s within {TIME_TOL} s of true start {want1} s",
        t.notes[1].onset_secs
    );
}

#[test]
fn ac4_notes_are_time_ordered_and_non_overlapping() {
    let cfg = Config::default();
    let t = gooz_dsp::analyze(&two_tone_signal(), SR, &cfg).expect("analyze on a two-tone signal");
    assert_eq!(t.notes.len(), 2, "precondition: exactly two notes");

    assert!(
        t.notes[0].onset_secs < t.notes[1].onset_secs,
        "notes are in time order (onset 0 < onset 1)"
    );
    // Note 0 must end (onset + duration) no later than note 1's onset — a tiny
    // tolerance absorbs frame-quantization rounding at the segment boundary.
    let note0_end = t.notes[0].onset_secs + t.notes[0].duration_secs;
    assert!(
        note0_end <= t.notes[1].onset_secs + TIME_TOL,
        "note 0 (ends {note0_end} s) does not overlap note 1 (onset {} s)",
        t.notes[1].onset_secs
    );
}

#[test]
fn ac4_note_durations_are_positive() {
    let cfg = Config::default();
    let t = gooz_dsp::analyze(&two_tone_signal(), SR, &cfg).expect("analyze on a two-tone signal");
    assert_eq!(t.notes.len(), 2, "precondition: exactly two notes");

    for (i, note) in t.notes.iter().enumerate() {
        assert!(
            note.duration_secs > 0.0,
            "note {i} has a strictly positive duration, got {}",
            note.duration_secs
        );
    }
}

// ---------------------------------------------------------------------------
// AC5 — Range & segmentation.
//
// A configurable [f_min, f_max] confines pitch detection; notes come only from
// voiced segments; events are sorted by onset.
// ---------------------------------------------------------------------------

/// A config whose `f_max` (500 Hz) excludes a 900 Hz tone, leaving everything
/// else at the defaults. `..Config::default()` keeps it valid if fields grow.
fn cfg_f_max_500() -> Config {
    Config {
        f_max: 500.0,
        ..Config::default()
    }
}

#[test]
fn ac5_tone_above_f_max_yields_no_in_range_voiced_frame() {
    // 900 Hz tone analyzed with f_max = 500: per spec §2 step 5 a frame is
    // voiced only if f0 is within [f_min, f_max]. So no frame may be reported
    // voiced with an in-range f0 — out-of-range pitch is not "voiced".
    let cfg = cfg_f_max_500();
    let signal = sine(900.0, 0.5, SR);
    let track = gooz_dsp::pitch_track(&signal, SR, &cfg)
        .expect("pitch_track on a 900 Hz tone with f_max=500");

    for frame in &track.frames {
        if let Some(f0) = frame.f0_hz {
            assert!(
                f0 <= cfg.f_max,
                "no voiced frame may report a pitch above f_max ({f0} Hz > {} Hz)",
                cfg.f_max
            );
        }
    }
}

#[test]
fn ac5_tone_above_f_max_yields_no_note() {
    // The 900 Hz tone (out of the configured [f_min=80, f_max=500] range)
    // produces no in-range voiced segment, hence no note.
    let cfg = cfg_f_max_500();
    let signal = concat(&[&silence(LEAD_IN, SR), &sine(900.0, 0.5, SR)]);
    let t = gooz_dsp::analyze(&signal, SR, &cfg).expect("analyze a 900 Hz tone with f_max=500");
    assert_eq!(
        t.notes.len(),
        0,
        "a tone entirely above f_max yields no in-range note: {:?}",
        t.notes
    );
}

#[test]
fn ac5_silence_only_signal_yields_zero_notes() {
    // Notes are assembled only from voiced segments; pure silence has none.
    let cfg = Config::default();
    let signal = silence(0.6, SR);
    let t = gooz_dsp::analyze(&signal, SR, &cfg).expect("analyze pure silence");
    assert_eq!(
        t.notes.len(),
        0,
        "a silence-only signal yields zero notes: {:?}",
        t.notes
    );
}

#[test]
fn ac5_notes_are_sorted_by_onset() {
    // A three-tone signal must yield notes sorted strictly ascending by onset.
    let cfg = Config::default();
    let signal = concat(&[
        &silence(LEAD_IN, SR),
        &sine(220.0, 0.30, SR),
        &silence(NOTE_GAP, SR),
        &sine(330.0, 0.30, SR),
        &silence(NOTE_GAP, SR),
        &sine(440.0, 0.30, SR),
    ]);
    let t = gooz_dsp::analyze(&signal, SR, &cfg).expect("analyze a three-tone signal");
    assert_eq!(t.notes.len(), 3, "three tones transcribe to three notes");

    for pair in t.notes.windows(2) {
        assert!(
            pair[0].onset_secs < pair[1].onset_secs,
            "notes are sorted strictly ascending by onset ({} !< {})",
            pair[0].onset_secs,
            pair[1].onset_secs
        );
    }
}

// ---------------------------------------------------------------------------
// AC6 — Typed errors, no panic.
// ---------------------------------------------------------------------------

#[test]
fn ac6_empty_signal_is_empty_signal_error() {
    let cfg = Config::default();
    let err = gooz_dsp::analyze(&[], SR, &cfg).expect_err("empty input must be rejected");
    assert_eq!(err, DspError::EmptySignal, "empty signal ⇒ EmptySignal");
}

#[test]
fn ac6_zero_sample_rate_is_invalid_sample_rate_error() {
    let cfg = Config::default();
    let signal = sine(440.0, 0.1, SR);
    let err = gooz_dsp::analyze(&signal, 0, &cfg).expect_err("a zero sample rate must be rejected");
    assert_eq!(
        err,
        DspError::InvalidSampleRate,
        "sample_rate == 0 ⇒ InvalidSampleRate"
    );
}

#[test]
fn ac6_signal_shorter_than_window_is_window_too_large_error() {
    // A buffer shorter than cfg.window cannot fill a single analysis frame.
    let cfg = Config::default();
    let short = vec![0.1f32; cfg.window - 1];
    let err = gooz_dsp::analyze(&short, SR, &cfg)
        .expect_err("a signal shorter than the window must be rejected");
    assert_eq!(
        err,
        DspError::WindowTooLarge,
        "window > signal.len() ⇒ WindowTooLarge"
    );
}

#[test]
fn ac6_nan_sample_is_non_finite_error() {
    // Non-finite input is rejected up front (spec §2 step 1) so no downstream
    // sum/sort/median can be poisoned — and crucially, no panic.
    let cfg = Config::default();
    let mut signal = sine(440.0, 0.2, SR);
    signal[100] = f32::NAN;
    let err =
        gooz_dsp::analyze(&signal, SR, &cfg).expect_err("a NaN sample must be rejected, not panic");
    assert_eq!(
        err,
        DspError::NonFiniteSample,
        "NaN sample ⇒ NonFiniteSample"
    );
}

#[test]
fn ac6_infinite_sample_is_non_finite_error() {
    let cfg = Config::default();
    let mut signal = sine(440.0, 0.2, SR);
    signal[100] = f32::INFINITY;
    let err = gooz_dsp::analyze(&signal, SR, &cfg)
        .expect_err("an infinite sample must be rejected, not panic");
    assert_eq!(
        err,
        DspError::NonFiniteSample,
        "infinite sample ⇒ NonFiniteSample"
    );
}

#[test]
fn ac6_dsp_error_is_typed_std_error_with_display() {
    fn assert_std_error<E: std::error::Error>(_: &E) {}
    assert_std_error(&DspError::EmptySignal);

    // Every variant has a non-empty Display message (errors are reported, never
    // panicked on). PartialEq/Eq let Results be asserted directly above.
    for err in [
        DspError::EmptySignal,
        DspError::InvalidSampleRate,
        DspError::WindowTooLarge,
        DspError::NonFiniteSample,
    ] {
        assert!(
            !err.to_string().is_empty(),
            "every DspError variant has a non-empty Display message ({err:?})"
        );
    }

    // Distinct variants compare unequal (used throughout the AC6 asserts).
    assert_ne!(DspError::EmptySignal, DspError::InvalidSampleRate);
    assert_ne!(DspError::WindowTooLarge, DspError::NonFiniteSample);
}

#[test]
fn ac6_pitch_track_and_detect_onsets_reject_bad_input() {
    // The exposed intermediates share the same up-front validation as analyze,
    // so they too surface typed errors instead of panicking.
    let cfg = Config::default();

    assert_eq!(
        gooz_dsp::pitch_track(&[], SR, &cfg).expect_err("empty ⇒ error"),
        DspError::EmptySignal,
        "pitch_track rejects an empty signal"
    );
    assert_eq!(
        gooz_dsp::detect_onsets(&[], SR, &cfg).expect_err("empty ⇒ error"),
        DspError::EmptySignal,
        "detect_onsets rejects an empty signal"
    );

    let signal = sine(440.0, 0.1, SR);
    assert_eq!(
        gooz_dsp::pitch_track(&signal, 0, &cfg).expect_err("zero sr ⇒ error"),
        DspError::InvalidSampleRate,
        "pitch_track rejects a zero sample rate"
    );
    assert_eq!(
        gooz_dsp::detect_onsets(&signal, 0, &cfg).expect_err("zero sr ⇒ error"),
        DspError::InvalidSampleRate,
        "detect_onsets rejects a zero sample rate"
    );
}

// ---------------------------------------------------------------------------
// AC7 — Documented public API & four toolchain gates.
//
// Every public item carries a runnable doc example, and all four gates
// (cargo build / test / clippy -D warnings / fmt --check) are green. These are
// verified at QA sign-off (loop step 7) — by running the gates and the doc
// tests — not by an integration test in this file. There is intentionally no
// AC7 test fn here; the golden-signal coverage above (AC1–AC6) is the
// "covered by golden-signal tests (no microphone)" half of AC7.
// ---------------------------------------------------------------------------

// Compile-time pin: keep the imported public types referenced even if a future
// edit drops their last in-test use, so a rename surfaces here as a hard error.
#[allow(dead_code)]
fn _type_surface_is_present(t: &Transcription) -> (&PitchTrack, &[Onset], &[NoteEvent]) {
    (&t.pitch_track, &t.onsets, &t.notes)
}
