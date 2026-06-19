//! Acceptance tests for R-0006 — snap-to-grid (quantize note events onto the
//! ratio grids), realized by SPEC-0006.
//!
//! TDD red: this file is written **before** the implementation exists. It is the
//! executable contract that the new `gooz-dsp` `quantize` module
//! (`QuantizedNote` + `quantize_notes`) and the re-exports it depends on must
//! satisfy. Until those items land — and until `gooz-dsp` re-adds its inward
//! `gooz-ratio` dependency (SPEC-0006 §2) — the crate does not name these
//! symbols and this file will not compile. That is the intended red state for
//! loop step 3.
//!
//! **No microphone, no device.** Every `NoteEvent` here is a hand-constructed
//! literal (R-0005's public struct), the grids are deterministic
//! (`PitchGrid::harmonic` / `Tempo::new`), and `quantize_notes` is a pure
//! function — so the suite is fully reproducible (R-0006 §2/§6, SPEC-0006 §2).
//!
//! One section per acceptance criterion (AC1–AC7); every test fn is prefixed
//! with the criterion id it verifies.
//!
//! ## AC → test mapping
//!
//! | AC  | Verified by                                                          |
//! |-----|----------------------------------------------------------------------|
//! | AC1 | `ac1_pitch_446_snaps_to_unison_one_octave_up`,                       |
//! |     | `ac1_pitch_332_snaps_to_fifth_same_octave`                           |
//! | AC2 | `ac2_sharp_hum_has_positive_cents_offset`,                           |
//! |     | `ac2_flat_hum_has_negative_cents_offset`,                            |
//! |     | `ac2_on_pitch_hum_has_zero_cents_offset`                             |
//! | AC3 | `ac3_onset_snaps_to_nearest_step`,                                   |
//! |     | `ac3_onset_at_zero_snaps_to_step_zero`,                              |
//! |     | `ac3_onset_just_over_half_step_rounds_up`                            |
//! | AC4 | `ac4_duration_snaps_to_whole_steps`,                                 |
//! |     | `ac4_tiny_duration_never_collapses_to_zero`,                         |
//! |     | `ac4_duration_is_a_multiple_of_step_secs`                            |
//! | AC5 | `ac5_three_valid_notes_preserve_count_and_order`,                    |
//! |     | `ac5_empty_input_yields_empty_output`,                               |
//! |     | `ac5_non_positive_pitch_is_skipped`,                                 |
//! |     | `ac5_non_finite_pitch_is_skipped`,                                   |
//! |     | `ac5_same_step_collisions_are_both_kept`                             |
//! | AC6 | `ac6_quantize_is_deterministic`,                                     |
//! |     | `ac6_subdivision_zero_is_treated_as_one`                             |
//! | AC7 | verified at QA sign-off (doc examples + clippy + fmt + build),       |
//! |     | not by a CI test in this file — see the AC7 note below.              |
//!
//! ## API assumptions beyond SPEC-0006 §2/§3
//!
//! SPEC-0006 §2/§3 fix every type, field, function, and re-export these tests
//! call; nothing was invented. The notes below pin the exact spelling and
//! semantics relied on, so any mismatch surfaces at the implementation step, not
//! as a test typo:
//!
//! * `QuantizedNote` has public fields `degree: Ratio, octave: i32,
//!   freq_hz: f64, cents_offset: f64, onset_step: u64, onset_secs: f64,
//!   duration_secs: f64` — verbatim SPEC-0006 §2. The pitch fields are
//!   `SnappedPitch`'s (`degree`/`octave`/`hz`→`freq_hz`/`cents_offset`); the
//!   timing fields are the beat-grid snap.
//! * `quantize_notes(notes: &[NoteEvent], pitch_grid: &PitchGrid,
//!   tempo: &Tempo, subdivision: u32) -> Vec<QuantizedNote>` — SPEC-0006 §3.
//!   Total and panic-free: an unsnappable pitch is skipped, `subdivision == 0`
//!   is treated as `1`.
//! * `gooz-dsp` re-exports `PitchGrid`, `Tempo`, `Ratio` (SPEC-0006 §2,
//!   enumerated — not a glob) so callers name argument/result types from
//!   `gooz_dsp` alone. If a re-export is missing the test won't compile — that
//!   is itself a valid red signal for the missing re-export.
//! * `step_secs = tempo.seconds_per_beat() / subdivision.max(1)` (SPEC-0006 §2).
//!   With `Tempo::new(120.0, 4.0)` (`seconds_per_beat() == 0.5`) and
//!   `subdivision == 2`, `step_secs == 0.25 s`. Multiples of `0.25` are exact in
//!   `f64`, so the timing asserts use `assert_eq!`.
//! * `cents_offset` is `input − snapped`, computed as `1200·log2(input/snapped)`
//!   by `PitchGrid::snap` (SPEC-0001) — sharp ⇒ positive, flat ⇒ negative,
//!   on-pitch ⇒ ≈ 0.
//! * `QuantizedNote` derives `PartialEq + Debug + Clone` (assumed; required by
//!   the determinism assert in AC6, which compares whole `Vec<QuantizedNote>`s).
//!   `degree` is the re-exported `Ratio`, which already derives those.

use gooz_dsp::{NoteEvent, PitchGrid, QuantizedNote, Ratio, Tempo, quantize_notes};

// ---------------------------------------------------------------------------
// Pinned grids and tempo — the deterministic harness for every test.
//
//   PitchGrid::harmonic(220.0, 9)  → degrees 1:1, 9:8, 5:4, 3:2, 7:4
//     octave 0 frequencies: 220, 247.5, 275, 330, 385 Hz; octave 1: ·2.
//   Tempo::new(120.0, 4.0)         → seconds_per_beat() == 0.5
//     with subdivision 2           → step_secs == 0.25 s.
// ---------------------------------------------------------------------------

const ROOT_HZ: f64 = 220.0;
const ODD_LIMIT: u64 = 9;
const SUBDIVISION: u32 = 2;
const STEP_SECS: f64 = 0.25;

/// The pinned 220-rooted harmonic grid used across the suite.
fn grid() -> PitchGrid {
    PitchGrid::harmonic(ROOT_HZ, ODD_LIMIT).expect("220 Hz / odd-limit 9 is a valid harmonic grid")
}

/// The pinned 120 bpm / 4-beats-per-bar tempo (seconds_per_beat == 0.5).
fn tempo() -> Tempo {
    Tempo::new(120.0, 4.0).expect("120 bpm / 4 beats per bar is a valid tempo")
}

/// A `NoteEvent` literal (R-0005's public struct: all fields public).
fn note(onset_secs: f64, pitch_hz: f32, duration_secs: f64) -> NoteEvent {
    NoteEvent {
        onset_secs,
        pitch_hz,
        duration_secs,
    }
}

/// The single quantized note for a one-note input, with a clear failure message.
fn quantize_one(n: NoteEvent) -> QuantizedNote {
    let out = quantize_notes(&[n], &grid(), &tempo(), SUBDIVISION);
    assert_eq!(
        out.len(),
        1,
        "a single finite-positive-pitch note must quantize to exactly one note"
    );
    out.into_iter().next().expect("exactly one quantized note")
}

// ---------------------------------------------------------------------------
// AC1 — Pitch snaps to the nearest grid degree, in the correct octave, at the
// exact grid frequency.
//
// On a 220-rooted grid: 446 Hz → the unison one octave up (440 Hz exactly);
// 332 Hz → the fifth (3:2) in the same octave (330 Hz exactly).
// ---------------------------------------------------------------------------

#[test]
fn ac1_pitch_446_snaps_to_unison_one_octave_up() {
    // 446 Hz is just sharp of 440 = 220 · 2 (unison, one octave up). The nearest
    // grid candidates are 440 (1:1, oct 1) and 385 (7:4, oct 0); 440 wins.
    let q = quantize_one(note(0.0, 446.0, 1.0));

    assert_eq!(q.degree, Ratio::UNISON, "446 Hz snaps to the unison (1:1)");
    assert_eq!(q.octave, 1, "446 Hz snaps one octave above the root");
    assert_eq!(
        q.freq_hz, 440.0,
        "the snapped frequency is the exact grid pitch 220 · 2 = 440 Hz"
    );
}

#[test]
fn ac1_pitch_332_snaps_to_fifth_same_octave() {
    // 332 Hz is just sharp of 330 = 220 · 3/2 (the fifth, octave 0). Nearest
    // candidates: 330 (3:2) and 385 (7:4); 330 wins.
    let q = quantize_one(note(0.0, 332.0, 1.0));

    assert_eq!(
        q.degree,
        Ratio::new(3, 2).expect("3:2 is a valid ratio"),
        "332 Hz snaps to the fifth (3:2)"
    );
    assert_eq!(q.octave, 0, "332 Hz snaps in the root octave");
    assert_eq!(
        q.freq_hz, 330.0,
        "the snapped frequency is the exact grid pitch 220 · 3/2 = 330 Hz"
    );
}

// ---------------------------------------------------------------------------
// AC2 — Cents offset reports the signed distance of the original pitch from the
// snapped grid pitch (input − snapped): sharp +, flat −, on-pitch ≈ 0.
// ---------------------------------------------------------------------------

/// Cents tolerance for matching the exact `1200·log2(input/snapped)` formula.
const CENTS_TOL: f64 = 1e-6;

#[test]
fn ac2_sharp_hum_has_positive_cents_offset() {
    // 446 Hz snapped to 440 Hz: the hum is sharp, so the offset is positive and
    // equals 1200·log2(446/440) ≈ +23.45 cents.
    let q = quantize_one(note(0.0, 446.0, 1.0));

    assert!(
        q.cents_offset > 0.0,
        "a sharp hum (446 vs 440 Hz) has a positive cents offset, got {}",
        q.cents_offset
    );
    let expected = 1200.0 * (446.0f64 / 440.0).log2();
    assert!(
        (q.cents_offset - expected).abs() < CENTS_TOL,
        "cents offset {} must equal 1200·log2(446/440) = {expected}",
        q.cents_offset
    );
}

#[test]
fn ac2_flat_hum_has_negative_cents_offset() {
    // 326 Hz snapped to 330 Hz (the fifth): the hum is flat, so the offset is
    // negative and equals 1200·log2(326/330) ≈ −21.1 cents.
    let q = quantize_one(note(0.0, 326.0, 1.0));
    assert_eq!(
        q.freq_hz, 330.0,
        "precondition: 326 Hz snaps to the fifth at 330 Hz"
    );

    assert!(
        q.cents_offset < 0.0,
        "a flat hum (326 vs 330 Hz) has a negative cents offset, got {}",
        q.cents_offset
    );
    let expected = 1200.0 * (326.0f64 / 330.0).log2();
    assert!(
        (q.cents_offset - expected).abs() < CENTS_TOL,
        "cents offset {} must equal 1200·log2(326/330) = {expected}",
        q.cents_offset
    );
}

#[test]
fn ac2_on_pitch_hum_has_zero_cents_offset() {
    // A note exactly on the grid (330 Hz = the fifth) has a zero cents offset:
    // snapping is idempotent and the grid frequency is a bitwise fixed point.
    let q = quantize_one(note(0.0, 330.0, 1.0));
    assert_eq!(
        q.freq_hz, 330.0,
        "precondition: 330 Hz is an exact grid pitch (the fifth)"
    );

    assert!(
        q.cents_offset.abs() < CENTS_TOL,
        "an on-pitch hum (330 Hz) has ≈ 0 cents offset, got {}",
        q.cents_offset
    );
}

// ---------------------------------------------------------------------------
// AC3 — Onset snaps to the nearest beat-grid step: round(onset / step_secs)
// with step_secs == 0.25 s. The reported onset_secs is onset_step · step_secs.
// A note at t = 0 snaps to step 0. Pitch is held on-grid (440 Hz) so timing is
// the only variable.
// ---------------------------------------------------------------------------

/// An on-grid pitch (the unison one octave up, 440 Hz) so timing tests isolate
/// the onset/duration snap from the pitch snap.
const ON_GRID_HZ: f32 = 440.0;

#[test]
fn ac3_onset_snaps_to_nearest_step() {
    // onset 0.41 s: round(0.41 / 0.25) = round(1.64) = 2 → step 2, 0.5 s.
    let q = quantize_one(note(0.41, ON_GRID_HZ, 1.0));

    assert_eq!(
        q.onset_step, 2,
        "onset 0.41 s snaps to step 2 (round(1.64) = 2)"
    );
    assert_eq!(
        q.onset_secs, 0.5,
        "the snapped onset time is step 2 · 0.25 s = 0.5 s"
    );
}

#[test]
fn ac3_onset_at_zero_snaps_to_step_zero() {
    // A note starting at t = 0 (the grid origin / downbeat) snaps to step 0.
    let q = quantize_one(note(0.0, ON_GRID_HZ, 1.0));

    assert_eq!(q.onset_step, 0, "onset at t = 0 snaps to step 0");
    assert_eq!(
        q.onset_secs, 0.0,
        "the snapped onset time for step 0 is 0 s"
    );
}

#[test]
fn ac3_onset_just_over_half_step_rounds_up() {
    // onset 0.13 s: round(0.13 / 0.25) = round(0.52) = 1 → step 1, 0.25 s.
    // Just over half a step rounds up to the next step.
    let q = quantize_one(note(0.13, ON_GRID_HZ, 1.0));

    assert_eq!(
        q.onset_step, 1,
        "onset 0.13 s snaps to step 1 (round(0.52) = 1)"
    );
    assert_eq!(
        q.onset_secs, 0.25,
        "the snapped onset time is step 1 · 0.25 s = 0.25 s"
    );
}

// ---------------------------------------------------------------------------
// AC4 — Duration snaps to whole steps and is always ≥ 1 step. The snapped end
// is round((onset + duration) / step_secs), clamped so end_step ≥ onset_step + 1;
// duration_secs = (end_step − onset_step) · step_secs. A note never collapses to
// zero length.
// ---------------------------------------------------------------------------

#[test]
fn ac4_duration_snaps_to_whole_steps() {
    // onset 0.0 (step 0), duration 0.37: end_raw = round(0.37 / 0.25) =
    // round(1.48) = 1; end_step = max(1, 0 + 1) = 1; duration = 1 step = 0.25 s.
    let q = quantize_one(note(0.0, ON_GRID_HZ, 0.37));

    assert_eq!(q.onset_step, 0, "precondition: onset at step 0");
    assert_eq!(
        q.duration_secs, 0.25,
        "duration 0.37 s snaps to one whole step (0.25 s)"
    );
}

#[test]
fn ac4_tiny_duration_never_collapses_to_zero() {
    // onset 0.0 (step 0), duration 0.01: end_raw = round(0.04) = 0; end_step =
    // max(0, 0 + 1) = 1 (the ≥ 1-step floor). duration = 1 step = 0.25 s, never 0.
    let q = quantize_one(note(0.0, ON_GRID_HZ, 0.01));

    assert_eq!(q.onset_step, 0, "precondition: onset at step 0");
    assert!(
        q.duration_secs > 0.0,
        "a quantized note never has zero duration, got {}",
        q.duration_secs
    );
    assert_eq!(
        q.duration_secs, 0.25,
        "a sub-step duration is floored to one whole step (0.25 s)"
    );
}

#[test]
fn ac4_duration_is_a_multiple_of_step_secs() {
    // A longer note's snapped duration must be an exact integer multiple of the
    // step. onset 0.0 (step 0), duration 0.80: end_raw = round(3.2) = 3;
    // end_step = max(3, 1) = 3; duration = 3 steps = 0.75 s.
    let q = quantize_one(note(0.0, ON_GRID_HZ, 0.80));

    assert!(
        q.duration_secs > 0.0,
        "duration is strictly positive, got {}",
        q.duration_secs
    );
    let steps = q.duration_secs / STEP_SECS;
    assert_eq!(
        steps,
        steps.round(),
        "duration {} s is an exact whole-step multiple of {STEP_SECS} s",
        q.duration_secs
    );
    assert!(
        steps >= 1.0,
        "duration is at least one whole step, got {steps} steps"
    );
    assert_eq!(
        q.duration_secs, 0.75,
        "duration 0.80 s snaps to 3 steps = 0.75 s"
    );
}

// ---------------------------------------------------------------------------
// AC5 — Order and count preserved; bad-pitch notes skipped; empty → empty;
// same-step collisions both kept (no merge).
// ---------------------------------------------------------------------------

#[test]
fn ac5_three_valid_notes_preserve_count_and_order() {
    // Three valid, onset-sorted notes → three quantized notes, onset_step
    // non-decreasing (input onset order preserved).
    let notes = [
        note(0.0, 220.0, 0.3),
        note(0.5, 330.0, 0.3),
        note(1.1, 440.0, 0.3),
    ];
    let out = quantize_notes(&notes, &grid(), &tempo(), SUBDIVISION);

    assert_eq!(
        out.len(),
        3,
        "three valid notes yield three quantized notes"
    );
    for pair in out.windows(2) {
        assert!(
            pair[0].onset_step <= pair[1].onset_step,
            "onset order is preserved (steps non-decreasing): {} then {}",
            pair[0].onset_step,
            pair[1].onset_step
        );
    }
    // The input is strictly onset-ascending and the steps are far apart, so the
    // mapping is order-preserving in the strict sense here too.
    assert!(
        out[0].onset_step < out[1].onset_step && out[1].onset_step < out[2].onset_step,
        "distinct input onsets map to strictly increasing steps: {:?}",
        out.iter().map(|q| q.onset_step).collect::<Vec<_>>()
    );
}

#[test]
fn ac5_empty_input_yields_empty_output() {
    let out = quantize_notes(&[], &grid(), &tempo(), SUBDIVISION);
    assert!(out.is_empty(), "empty input yields empty output");
}

#[test]
fn ac5_non_positive_pitch_is_skipped() {
    // A zero-Hz and a negative-Hz pitch cannot be snapped (PitchGrid::snap
    // rejects non-positive input) → they are skipped, never a panic. The two
    // valid notes around them survive, in order.
    let notes = [
        note(0.0, 220.0, 0.3),
        note(0.5, 0.0, 0.3),    // zero pitch → skipped
        note(1.0, -110.0, 0.3), // negative pitch → skipped
        note(1.5, 440.0, 0.3),
    ];
    let out = quantize_notes(&notes, &grid(), &tempo(), SUBDIVISION);

    assert_eq!(
        out.len(),
        2,
        "only the two finite-positive-pitch notes survive: {:?}",
        out
    );
    assert_eq!(
        out[0].freq_hz, 220.0,
        "the first surviving note is the 220 Hz unison"
    );
    assert_eq!(
        out[1].freq_hz, 440.0,
        "the second surviving note is the 440 Hz unison (octave up)"
    );
}

#[test]
fn ac5_non_finite_pitch_is_skipped() {
    // NaN and +∞ pitches are non-finite → PitchGrid::snap rejects them → skipped
    // (never a panic). Output length is the count of finite-positive-pitch notes.
    let notes = [
        note(0.0, f32::NAN, 0.3),      // NaN → skipped
        note(0.5, 330.0, 0.3),         // valid
        note(1.0, f32::INFINITY, 0.3), // +∞ → skipped
    ];
    let out = quantize_notes(&notes, &grid(), &tempo(), SUBDIVISION);

    assert_eq!(
        out.len(),
        1,
        "only the single finite-positive-pitch note survives: {:?}",
        out
    );
    assert_eq!(out[0].freq_hz, 330.0, "the survivor is the 330 Hz fifth");
}

#[test]
fn ac5_same_step_collisions_are_both_kept() {
    // Two notes whose onsets snap to the same step (0.0 → step 0 and 0.10 →
    // round(0.4) = 0 → step 0) are BOTH retained: no merge, dedup, or overlap
    // resolution (that is Advanced Mode).
    let notes = [note(0.0, 220.0, 0.3), note(0.10, 330.0, 0.3)];
    let out = quantize_notes(&notes, &grid(), &tempo(), SUBDIVISION);

    assert_eq!(
        out.len(),
        2,
        "two notes colliding on the same onset step are both kept: {:?}",
        out
    );
    assert_eq!(out[0].onset_step, 0, "first note snaps to step 0");
    assert_eq!(
        out[1].onset_step, 0,
        "second note (0.10 s → round(0.4) = 0) also snaps to step 0"
    );
    // Order is still preserved and the two notes are distinct by pitch.
    assert_eq!(out[0].freq_hz, 220.0, "first kept note is 220 Hz");
    assert_eq!(out[1].freq_hz, 330.0, "second kept note is 330 Hz");
}

// ---------------------------------------------------------------------------
// AC6 — Pure & exact: deterministic, and subdivision 0 is treated as 1.
// ---------------------------------------------------------------------------

#[test]
fn ac6_quantize_is_deterministic() {
    // The same input quantized twice must yield equal results (field-for-field).
    // This requires QuantizedNote: PartialEq + Debug (assumed; see header).
    let notes = [
        note(0.0, 221.0, 0.3),
        note(0.41, 332.0, 0.37),
        note(1.13, 446.0, 0.80),
    ];
    let a = quantize_notes(&notes, &grid(), &tempo(), SUBDIVISION);
    let b = quantize_notes(&notes, &grid(), &tempo(), SUBDIVISION);

    assert_eq!(
        a, b,
        "quantize_notes is deterministic: identical input → identical output"
    );
}

#[test]
fn ac6_subdivision_zero_is_treated_as_one() {
    // SPEC-0006 §2: subdivision 0 is clamped to 1 (documented, not a silent
    // surprise). So quantizing with subdivision 0 and subdivision 1 must agree
    // on the onset steps (step_secs = seconds_per_beat / 1 = 0.5 s in both).
    let notes = [
        note(0.0, 220.0, 0.3),
        note(0.6, 330.0, 0.3),
        note(1.4, 440.0, 0.3),
    ];
    let with_zero = quantize_notes(&notes, &grid(), &tempo(), 0);
    let with_one = quantize_notes(&notes, &grid(), &tempo(), 1);

    assert_eq!(
        with_zero, with_one,
        "subdivision 0 behaves identically to subdivision 1 (0 ⇒ 1)"
    );
    // And the step grid really is the 0.5 s beat grid: onset 0.6 s →
    // round(0.6 / 0.5) = round(1.2) = 1.
    assert_eq!(
        with_one[1].onset_step, 1,
        "with subdivision 1, step_secs = 0.5 s so onset 0.6 s → step 1"
    );
}

// ---------------------------------------------------------------------------
// AC7 — Documented public API & four toolchain gates.
//
// Every public item (QuantizedNote, quantize_notes, and the re-exports) carries
// a runnable doc example, and all four gates (cargo build / test /
// clippy -D warnings / fmt --check) are green. These are verified at QA sign-off
// (loop step 7) by running the gates and the doc tests — not by an integration
// test here. There is intentionally no AC7 test fn; the cases above (AC1–AC6)
// are the "behaviour covered by tests" half of AC7.
// ---------------------------------------------------------------------------

// Compile-time pin: keep the imported public surface referenced even if a future
// edit drops its last in-test use, so a rename surfaces here as a hard error.
#[allow(dead_code)]
fn _type_surface_is_present(q: &QuantizedNote) -> (Ratio, i32, f64, f64, u64, f64, f64) {
    (
        q.degree,
        q.octave,
        q.freq_hz,
        q.cents_offset,
        q.onset_step,
        q.onset_secs,
        q.duration_secs,
    )
}
