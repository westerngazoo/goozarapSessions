//! Acceptance tests for R-0002 — beat-ratio core, realized by SPEC-0002.
//!
//! TDD red: this file is written **before** the implementation exists. It is
//! the executable contract that the rhythm code in `crates/gooz-ratio/src/`
//! (the new `Pattern`, `BarGrid`, `QuantizedBeat`, `Polyrhythm`, `Tempo`, and
//! `BeatError` items) must satisfy. Until those types land it will not compile;
//! that is the intended red state for loop step 3.
//!
//! One section per acceptance criterion (AC1–AC8); every test fn is prefixed
//! with the criterion id it verifies. AC8 (documented public API with runnable
//! doc examples + the four toolchain gates: build / test / clippy / fmt) is a
//! doc-test / CI concern verified at QA sign-off (loop step 7), not by an
//! integration test in this file.
//!
//! ## AC → test mapping
//!
//! | AC  | Verified by                                                          |
//! |-----|----------------------------------------------------------------------|
//! | AC1 | `ac1_tresillo_e3_8_has_known_onsets`,                                |
//! |     | `ac1_cinquillo_e5_8_has_known_onsets`,                               |
//! |     | `ac1_e4_16_has_known_onsets`,                                        |
//! |     | `ac1_first_step_is_an_onset_when_k_positive`,                        |
//! |     | `ac1_euclidean_is_deterministic`,                                    |
//! |     | `ac1_euclidean_places_exactly_k_onsets` (property)                   |
//! | AC2 | `ac2_k_zero_yields_all_rests`,                                       |
//! |     | `ac2_terminating_e0_n_runs_to_completion_all_rests`,                |
//! |     | `ac2_k_equals_n_yields_all_onsets`,                                  |
//! |     | `ac2_k_greater_than_n_is_too_many_onsets`,                           |
//! |     | `ac2_zero_steps_is_empty_grid`,                                      |
//! |     | `ac2_beat_error_is_a_typed_std_error`                                |
//! | AC3 | `ac3_rotation_preserves_count_and_length`,                          |
//! |     | `ac3_rotate_by_zero_is_identity`,                                    |
//! |     | `ac3_rotate_by_multiples_of_len_is_identity`,                       |
//! |     | `ac3_negative_rotation_is_the_inverse_of_positive`,                 |
//! |     | `ac3_rotation_is_a_cyclic_shift`                                     |
//! | AC4 | `ac4_grid_exposes_n_positions`,                                     |
//! |     | `ac4_downbeat_is_zero_over_one`,                                     |
//! |     | `ac4_position_is_exact_reduced_fraction`,                           |
//! |     | `ac4_positions_are_strictly_ascending_in_unit_interval`,           |
//! |     | `ac4_position_wraps_on_index_at_or_above_steps`,                    |
//! |     | `ac4_zero_steps_is_empty_grid`                                       |
//! | AC5 | `ac5_exact_on_step_phase_is_a_fixed_point`,                         |
//! |     | `ac5_quantize_snaps_to_nearest_step`,                              |
//! |     | `ac5_tie_resolves_to_the_earlier_step`,                            |
//! |     | `ac5_phase_just_below_barline_wraps_to_step_zero`,                 |
//! |     | `ac5_offset_is_signed_input_minus_snapped`,                        |
//! |     | `ac5_quantize_is_idempotent` (property),                           |
//! |     | `ac5_non_finite_phase_is_rejected`                                  |
//! | AC6 | `ac6_polyrhythm_3_against_2_grid_and_pulses`,                       |
//! |     | `ac6_polyrhythm_4_against_3_grid_steps`,                           |
//! |     | `ac6_polyrhythm_pulse_fractions_are_exact_and_reduced`,            |
//! |     | `ac6_zero_pulse_count_is_empty_grid`                               |
//! | AC7 | `ac7_seconds_per_beat_120_bpm_is_exactly_half_second`,             |
//! |     | `ac7_bar_seconds_scales_with_beats_per_bar`,                       |
//! |     | `ac7_step_time_is_phase_times_bar_seconds`,                        |
//! |     | `ac7_non_finite_or_non_positive_tempo_is_rejected`                 |
//! | AC8 | verified at QA sign-off (doc tests + clippy + fmt), not here         |
//!
//! ## API assumptions beyond SPEC-0002 §2/§3
//!
//! SPEC-0002 fixes every constructor, accessor, and error name these tests
//! call; nothing was inferred. The notes below pin the exact spelling/semantics
//! the tests rely on so any mismatch is caught at the implementation step, not
//! blamed on a test typo:
//!
//! * `Pattern::euclidean(onsets: u32, steps: u32) -> Result<Pattern, BeatError>`,
//!   with `len() -> usize`, `is_empty() -> bool`, `onset_count() -> usize`,
//!   `is_onset(i: usize) -> bool`, `steps() -> &[bool]`,
//!   `onsets() -> Vec<usize>` (ascending), `rotate(by: i64) -> Pattern` — all
//!   verbatim from spec §2 ("Accessors"). `Pattern: Clone + PartialEq` is
//!   assumed for `assert_eq!` on whole patterns (mirrors `Ratio`'s derives).
//! * `BeatError: PartialEq` (spec §2 states it derives exactly what
//!   `RatioError` does: `Debug, Clone, Copy, PartialEq, Eq`) — used for
//!   `assert_eq!` on `Result`s and `std::error::Error` for the typed-error test.
//! * `QuantizedBeat { step: u32, phase: f64, offset: f64 }` with public fields,
//!   given verbatim in spec §2; constructed/compared field-by-field here.
//! * `Polyrhythm::a_pulses()/b_pulses() -> Vec<(u64, u64)>` of reduced bar
//!   fractions; `grid_steps() -> u64` — verbatim spec §2.
//! * `Tempo` accessors `seconds_per_beat()/bar_seconds()/step_time(phase)` —
//!   verbatim spec §2.

use std::collections::HashSet;

use gooz_ratio::{BarGrid, BeatError, Pattern, Polyrhythm, Tempo};

/// Test-only constructor for a Euclidean pattern with valid arguments.
fn e(onsets: u32, steps: u32) -> Pattern {
    Pattern::euclidean(onsets, steps).expect("test patterns use valid (onsets <= steps, steps > 0)")
}

/// Test-only constructor for a bar grid with a valid step count.
fn grid(steps: u32) -> BarGrid {
    BarGrid::new(steps).expect("test grids use a positive step count")
}

// ---------------------------------------------------------------------------
// AC1 — Euclidean distribution
// ---------------------------------------------------------------------------

#[test]
fn ac1_tresillo_e3_8_has_known_onsets() {
    let pattern = e(3, 8);
    assert_eq!(pattern.len(), 8);
    assert_eq!(pattern.onset_count(), 3);
    assert_eq!(pattern.onsets(), vec![0, 3, 6], "E(3,8) is the tresillo");
}

#[test]
fn ac1_cinquillo_e5_8_has_known_onsets() {
    let pattern = e(5, 8);
    assert_eq!(pattern.len(), 8);
    assert_eq!(pattern.onset_count(), 5);
    assert_eq!(
        pattern.onsets(),
        vec![0, 2, 3, 5, 6],
        "E(5,8) is the cinquillo"
    );
}

#[test]
fn ac1_e4_16_has_known_onsets() {
    let pattern = e(4, 16);
    assert_eq!(pattern.len(), 16);
    assert_eq!(pattern.onset_count(), 4);
    assert_eq!(
        pattern.onsets(),
        vec![0, 4, 8, 12],
        "E(4,16) is four-on-the-floor"
    );
}

#[test]
fn ac1_first_step_is_an_onset_when_k_positive() {
    for (k, n) in [
        (1u32, 8u32),
        (3, 8),
        (5, 8),
        (4, 16),
        (7, 12),
        (1, 1),
        (2, 2),
    ] {
        let pattern = e(k, n);
        assert!(
            pattern.is_onset(0),
            "E({k},{n}): the first step must be an onset when k > 0"
        );
        assert_eq!(pattern.onsets().first(), Some(&0));
    }
}

#[test]
fn ac1_euclidean_is_deterministic() {
    for (k, n) in [(3u32, 8u32), (5, 8), (4, 16), (7, 16), (2, 5)] {
        assert_eq!(e(k, n), e(k, n), "same input must yield the same pattern");
        assert_eq!(e(k, n).steps(), e(k, n).steps());
    }
}

/// Property (architect-requested): E(k, n) places exactly k onsets across the
/// whole small-grid domain.
#[test]
fn ac1_euclidean_places_exactly_k_onsets() {
    for n in 1u32..=16 {
        for k in 0u32..=n {
            let pattern = e(k, n);
            assert_eq!(pattern.len(), n as usize, "E({k},{n}) must have length n");
            assert_eq!(
                pattern.onset_count(),
                k as usize,
                "E({k},{n}) must have exactly k onsets"
            );
            // onsets() and is_onset() must agree with the count.
            assert_eq!(pattern.onsets().len(), k as usize);
            let counted = (0..n as usize).filter(|&i| pattern.is_onset(i)).count();
            assert_eq!(counted, k as usize);
        }
    }
}

// ---------------------------------------------------------------------------
// AC2 — Euclidean boundaries
// ---------------------------------------------------------------------------

#[test]
fn ac2_k_zero_yields_all_rests() {
    let pattern = e(0, 8);
    assert_eq!(pattern.len(), 8);
    assert_eq!(pattern.onset_count(), 0);
    assert_eq!(pattern.onsets(), Vec::<usize>::new());
    assert!(pattern.steps().iter().all(|&onset| !onset));
}

/// Architect-flagged regression: E(0, n) for n >= 2 must run to completion
/// (the Bjorklund loop hangs on an empty onset pile; the fix short-circuits
/// before entering it). This test passing — i.e. *returning at all* — is the
/// guard against the non-termination bug.
#[test]
fn ac2_terminating_e0_n_runs_to_completion_all_rests() {
    for n in [2u32, 5, 8, 16] {
        let pattern = e(0, n);
        assert_eq!(pattern.len(), n as usize);
        assert_eq!(pattern.onset_count(), 0);
        assert!(
            pattern.steps().iter().all(|&onset| !onset),
            "E(0,{n}) must be all rests"
        );
    }
}

#[test]
fn ac2_k_equals_n_yields_all_onsets() {
    for n in [1u32, 2, 8, 16] {
        let pattern = e(n, n);
        assert_eq!(pattern.len(), n as usize);
        assert_eq!(pattern.onset_count(), n as usize);
        assert!(
            pattern.steps().iter().all(|&onset| onset),
            "E({n},{n}) must be all onsets"
        );
        assert_eq!(pattern.onsets(), (0..n as usize).collect::<Vec<_>>());
    }
}

#[test]
fn ac2_k_greater_than_n_is_too_many_onsets() {
    assert_eq!(Pattern::euclidean(9, 8), Err(BeatError::TooManyOnsets));
    assert_eq!(Pattern::euclidean(2, 1), Err(BeatError::TooManyOnsets));
    assert_eq!(Pattern::euclidean(17, 16), Err(BeatError::TooManyOnsets));
}

#[test]
fn ac2_zero_steps_is_empty_grid() {
    assert_eq!(Pattern::euclidean(0, 0), Err(BeatError::EmptyGrid));
    assert_eq!(
        Pattern::euclidean(3, 0),
        Err(BeatError::EmptyGrid),
        "steps == 0 is rejected before the k > n check"
    );
}

#[test]
fn ac2_beat_error_is_a_typed_std_error() {
    fn assert_std_error<E: std::error::Error>(_: &E) {}
    assert_std_error(&BeatError::EmptyGrid);
    assert!(!BeatError::EmptyGrid.to_string().is_empty());
    assert!(!BeatError::TooManyOnsets.to_string().is_empty());
    assert!(!BeatError::InvalidPhase.to_string().is_empty());
    assert!(!BeatError::InvalidTempo.to_string().is_empty());
}

// ---------------------------------------------------------------------------
// AC3 — Rotation
// ---------------------------------------------------------------------------

#[test]
fn ac3_rotation_preserves_count_and_length() {
    let pattern = e(3, 8);
    for by in [-16i64, -3, -1, 0, 1, 2, 5, 8, 13] {
        let rotated = pattern.rotate(by);
        assert_eq!(rotated.len(), pattern.len(), "rotate({by}) keeps length");
        assert_eq!(
            rotated.onset_count(),
            pattern.onset_count(),
            "rotate({by}) keeps onset count"
        );
    }
}

#[test]
fn ac3_rotate_by_zero_is_identity() {
    let pattern = e(5, 8);
    assert_eq!(pattern.rotate(0), pattern);
}

#[test]
fn ac3_rotate_by_multiples_of_len_is_identity() {
    let pattern = e(3, 8);
    for multiple in [-2i64, -1, 1, 2, 3] {
        let by = multiple * pattern.len() as i64;
        assert_eq!(
            pattern.rotate(by),
            pattern,
            "rotating by {by} (a whole multiple of len) is the identity"
        );
    }
}

#[test]
fn ac3_negative_rotation_is_the_inverse_of_positive() {
    let pattern = e(5, 8);
    // Rotating one way then the other returns the original.
    assert_eq!(pattern.rotate(3).rotate(-3), pattern);
    assert_eq!(pattern.rotate(-3).rotate(3), pattern);
    // A negative offset is congruent (mod len) to its positive complement.
    assert_eq!(pattern.rotate(-1), pattern.rotate(pattern.len() as i64 - 1));
}

#[test]
fn ac3_rotation_is_a_cyclic_shift() {
    // E(3,8) onsets {0,3,6}. Rotating "forward" by 1 maps step s -> (s+1) mod 8,
    // so onsets become {1,4,7}. (Negative rotates the other way.)
    let pattern = e(3, 8);
    let len = pattern.len();
    let forward = pattern.rotate(1);
    let expected: Vec<usize> = pattern
        .onsets()
        .into_iter()
        .map(|s| (s + 1) % len)
        .collect();
    let mut got = forward.onsets();
    got.sort_unstable();
    let mut want = expected;
    want.sort_unstable();
    assert_eq!(
        got, want,
        "forward rotation shifts every onset by +1 (mod len)"
    );
}

// ---------------------------------------------------------------------------
// AC4 — Bar grid
// ---------------------------------------------------------------------------

#[test]
fn ac4_grid_exposes_n_positions() {
    for n in [1u32, 2, 4, 8, 16] {
        let g = grid(n);
        assert_eq!(g.steps(), n);
        let positions: Vec<(u64, u64)> = (0..n).map(|i| g.position(i)).collect();
        assert_eq!(
            positions.len(),
            n as usize,
            "a grid of n steps has n positions"
        );
    }
}

#[test]
fn ac4_downbeat_is_zero_over_one() {
    for n in [1u32, 3, 4, 8, 16] {
        let g = grid(n);
        assert_eq!(
            g.position(0),
            (0, 1),
            "the downbeat reduces to (0, 1), not (0, n)"
        );
        assert_eq!(g.phase(0), 0.0);
    }
}

#[test]
fn ac4_position_is_exact_reduced_fraction() {
    let g = grid(8);
    // i/8 reduced: 0/1, 1/8, 1/4, 3/8, 1/2, 5/8, 3/4, 7/8.
    assert_eq!(g.position(0), (0, 1));
    assert_eq!(g.position(1), (1, 8));
    assert_eq!(g.position(2), (1, 4));
    assert_eq!(g.position(3), (3, 8));
    assert_eq!(g.position(4), (1, 2));
    assert_eq!(g.position(5), (5, 8));
    assert_eq!(g.position(6), (3, 4));
    assert_eq!(g.position(7), (7, 8));
}

#[test]
fn ac4_positions_are_strictly_ascending_in_unit_interval() {
    for n in [1u32, 4, 8, 12, 16] {
        let g = grid(n);
        let mut previous = -1.0f64;
        for i in 0..n {
            let phase = g.phase(i);
            assert!(
                (0.0..1.0).contains(&phase),
                "phase {phase} must be in [0, 1)"
            );
            assert!(
                phase > previous,
                "phases are strictly ascending: {phase} must exceed {previous}"
            );
            // phase agrees with the exact fraction.
            let (num, den) = g.position(i);
            assert_eq!(phase, num as f64 / den as f64);
            previous = phase;
        }
    }
}

#[test]
fn ac4_position_wraps_on_index_at_or_above_steps() {
    let g = grid(8);
    // index mod steps: 8 -> 0, 9 -> 1, 16 -> 0, 11 -> 3.
    assert_eq!(g.position(8), g.position(0));
    assert_eq!(g.position(9), g.position(1));
    assert_eq!(g.position(16), g.position(0));
    assert_eq!(g.position(11), g.position(3));
    assert_eq!(g.phase(8), g.phase(0));
}

#[test]
fn ac4_zero_steps_is_empty_grid() {
    assert_eq!(BarGrid::new(0), Err(BeatError::EmptyGrid));
}

// ---------------------------------------------------------------------------
// AC5 — Time quantization
// ---------------------------------------------------------------------------

#[test]
fn ac5_exact_on_step_phase_is_a_fixed_point() {
    let g = grid(8);
    for i in 0..8u32 {
        let phase = g.phase(i);
        let q = g.quantize(phase).expect("a finite phase quantizes");
        assert_eq!(q.step, i, "an exact step phase snaps to itself");
        assert_eq!(q.phase, phase);
        assert_eq!(q.offset, 0.0, "an on-step phase has zero offset");
    }
}

#[test]
fn ac5_quantize_snaps_to_nearest_step() {
    let g = grid(8);
    // Steps sit at multiples of 0.125. 0.20 is nearest to step 2 (0.25);
    // 0.30 is nearest to step 2 (0.25); 0.40 nearest to step 3 (0.375).
    assert_eq!(g.quantize(0.20).expect("finite").step, 2);
    assert_eq!(g.quantize(0.30).expect("finite").step, 2);
    assert_eq!(g.quantize(0.40).expect("finite").step, 3);
    assert_eq!(g.quantize(0.13).expect("finite").step, 1);
}

#[test]
fn ac5_tie_resolves_to_the_earlier_step() {
    let g = grid(8);
    // Step spacing is 0.125; a midpoint tie sits at an odd multiple of 0.0625.
    // 0.0625 ties step 0 (0.0) and step 1 (0.125) -> earlier step 0.
    let low = g.quantize(0.0625).expect("finite");
    assert_eq!(low.step, 0, "a midpoint tie resolves to the earlier step");
    // 0.9375 ties step 7 (0.875) and the barline (1.0 ~ step 0) -> earlier step 7.
    let high = g.quantize(0.9375).expect("finite");
    assert_eq!(
        high.step, 7,
        "the tie just below the barline resolves to step 7, not the wrapped downbeat"
    );
}

#[test]
fn ac5_phase_just_below_barline_wraps_to_step_zero() {
    let g = grid(8);
    // 0.99 is closer to the barline (1.0 ~ step 0) than to step 7 (0.875).
    let q = g.quantize(0.99).expect("finite");
    assert_eq!(
        q.step, 0,
        "a phase just below the barline wraps to the downbeat"
    );
    assert_eq!(q.phase, 0.0);
    assert!(
        q.offset < 0.0,
        "wrap-around reports a small negative offset (input below the snapped barline)"
    );
    // offset = input - snapped target (1.0 here, the next barline): 0.99 - 1.0.
    assert!((q.offset - (0.99 - 1.0)).abs() < 1e-12);
}

#[test]
fn ac5_offset_is_signed_input_minus_snapped() {
    let g = grid(8);
    // 0.20 snaps to step 2 (phase 0.25): offset = 0.20 - 0.25 = -0.05 (early).
    let early = g.quantize(0.20).expect("finite");
    assert_eq!(early.step, 2);
    assert!((early.offset - (0.20 - 0.25)).abs() < 1e-12);
    assert!(
        early.offset < 0.0,
        "an input before its step is a negative offset"
    );

    // 0.30 snaps to step 2 (phase 0.25): offset = 0.30 - 0.25 = +0.05 (late).
    let late = g.quantize(0.30).expect("finite");
    assert_eq!(late.step, 2);
    assert!((late.offset - (0.30 - 0.25)).abs() < 1e-12);
    assert!(
        late.offset > 0.0,
        "an input after its step is a positive offset"
    );
}

/// Property (architect-requested): quantize is idempotent — snapping the
/// snapped phase lands on the same step with zero offset.
#[test]
fn ac5_quantize_is_idempotent() {
    let g = grid(8);
    let phases = [
        0.0, 0.05, 0.0625, 0.1, 0.13, 0.20, 0.25, 0.30, 0.40, 0.5, 0.6, 0.75, 0.8, 0.9, 0.9375,
        0.99,
    ];
    for &phase in &phases {
        let once = g.quantize(phase).expect("finite phase quantizes");
        let twice = g.quantize(once.phase).expect("a step phase is finite");
        assert_eq!(
            twice.step, once.step,
            "re-quantizing the snapped phase of {phase} keeps the step"
        );
        assert_eq!(
            twice.offset, 0.0,
            "re-quantizing an on-step phase has zero offset (idempotent)"
        );
        assert_eq!(twice.phase, once.phase);
    }
}

#[test]
fn ac5_non_finite_phase_is_rejected() {
    let g = grid(8);
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(g.quantize(bad), Err(BeatError::InvalidPhase));
    }
}

// ---------------------------------------------------------------------------
// AC6 — Polyrhythm
// ---------------------------------------------------------------------------

#[test]
fn ac6_polyrhythm_3_against_2_grid_and_pulses() {
    let poly = Polyrhythm::new(3, 2).expect("3 and 2 are valid pulse counts");
    assert_eq!(poly.grid_steps(), 6, "lcm(3, 2) = 6");
    assert_eq!(
        poly.a_pulses(),
        vec![(0, 1), (1, 3), (2, 3)],
        "the 3-pulses fall at {{0, 1/3, 2/3}}"
    );
    assert_eq!(
        poly.b_pulses(),
        vec![(0, 1), (1, 2)],
        "the 2-pulses fall at {{0, 1/2}}"
    );
}

#[test]
fn ac6_polyrhythm_4_against_3_grid_steps() {
    let poly = Polyrhythm::new(4, 3).expect("4 and 3 are valid pulse counts");
    assert_eq!(poly.grid_steps(), 12, "lcm(4, 3) = 12");
    assert_eq!(poly.a_pulses().len(), 4, "four evenly spaced a-pulses");
    assert_eq!(poly.b_pulses().len(), 3, "three evenly spaced b-pulses");
}

#[test]
fn ac6_polyrhythm_pulse_fractions_are_exact_and_reduced() {
    let poly = Polyrhythm::new(4, 3).expect("valid");
    // i/4 reduced for i in 0..4: 0/1, 1/4, 1/2, 3/4.
    assert_eq!(poly.a_pulses(), vec![(0, 1), (1, 4), (1, 2), (3, 4)]);
    // i/3 reduced for i in 0..3: 0/1, 1/3, 2/3.
    assert_eq!(poly.b_pulses(), vec![(0, 1), (1, 3), (2, 3)]);
}

#[test]
fn ac6_zero_pulse_count_is_empty_grid() {
    assert_eq!(Polyrhythm::new(0, 2), Err(BeatError::EmptyGrid));
    assert_eq!(Polyrhythm::new(3, 0), Err(BeatError::EmptyGrid));
    assert_eq!(Polyrhythm::new(0, 0), Err(BeatError::EmptyGrid));
}

// ---------------------------------------------------------------------------
// AC7 — Tempo mapping
// ---------------------------------------------------------------------------

#[test]
fn ac7_seconds_per_beat_120_bpm_is_exactly_half_second() {
    let tempo = Tempo::new(120.0, 4.0).expect("120 bpm is a valid tempo");
    assert_eq!(
        tempo.seconds_per_beat(),
        0.5,
        "120 bpm is exactly half a second per beat"
    );
}

#[test]
fn ac7_bar_seconds_scales_with_beats_per_bar() {
    // 120 bpm: 0.5 s/beat. A 4-beat bar lasts 2.0 s; a 3-beat bar lasts 1.5 s.
    let four = Tempo::new(120.0, 4.0).expect("valid");
    assert_eq!(four.bar_seconds(), 2.0);
    let three = Tempo::new(120.0, 3.0).expect("valid");
    assert_eq!(three.bar_seconds(), 1.5);
}

#[test]
fn ac7_step_time_is_phase_times_bar_seconds() {
    let tempo = Tempo::new(120.0, 4.0).expect("valid");
    // bar_seconds = 2.0; phase 0.5 is half a bar -> 1.0 s; downbeat -> 0.0 s.
    assert_eq!(tempo.step_time(0.0), 0.0);
    assert_eq!(
        tempo.step_time(0.5),
        1.0,
        "half a bar at 120/4 is one second"
    );
    assert_eq!(tempo.step_time(1.0), tempo.bar_seconds());
    assert_eq!(tempo.step_time(0.25), 0.5);
}

#[test]
fn ac7_non_finite_or_non_positive_tempo_is_rejected() {
    for bad_bpm in [0.0, -120.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(Tempo::new(bad_bpm, 4.0), Err(BeatError::InvalidTempo));
    }
    for bad_bpb in [0.0, -4.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(Tempo::new(120.0, bad_bpb), Err(BeatError::InvalidTempo));
    }
}

// ---------------------------------------------------------------------------
// Cross-criterion sanity: Euclidean onsets index into a matching BarGrid.
// (Not an AC of its own — guards that the rhythm and grid halves compose, the
// way R-0006/R-0009 will consume them. Uses only public API.)
// ---------------------------------------------------------------------------

#[test]
fn euclidean_onsets_map_onto_the_same_step_grid() {
    let pattern = e(3, 8);
    let g = grid(8);
    let onset_phases: HashSet<u64> = pattern
        .onsets()
        .into_iter()
        .map(|i| (g.phase(i as u32) * 1_000_000.0).round() as u64)
        .collect();
    // {0, 3, 6} on an 8-grid -> {0.0, 0.375, 0.75}.
    let expected: HashSet<u64> = [0u64, 375_000, 750_000].into_iter().collect();
    assert_eq!(onset_phases, expected);
}
