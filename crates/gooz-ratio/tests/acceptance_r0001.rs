//! Acceptance tests for R-0001 — frequency-ratio core, realized by SPEC-0001.
//!
//! TDD red: this file is written **before** the implementation exists. It is
//! the executable contract that the code in `crates/gooz-ratio/src/` must
//! satisfy. Until the types land it will not compile; that is the intended red
//! state for loop step 3.
//!
//! One section per acceptance criterion (AC1–AC7); every test fn is prefixed
//! with the criterion id it verifies. AC8 (documented public API with runnable
//! doc examples + the four toolchain gates) is a doc-test / CI concern verified
//! at QA sign-off (loop step 7), not by an integration test in this file.
//!
//! ## AC → test mapping
//!
//! | AC  | Verified by                                                          |
//! |-----|----------------------------------------------------------------------|
//! | AC1 | `ac1_unreduced_spelling_is_stored_canonical`,                        |
//! |     | `ac1_equality_hashing_and_ordering_agree_across_spellings`,          |
//! |     | `ac1_display_shows_canonical_form`,                                  |
//! |     | `ac1_zero_components_are_rejected`,                                  |
//! |     | `ac1_ratio_error_is_a_typed_std_error`                               |
//! | AC2 | `ac2_stacking_fifth_and_fourth_yields_exact_octave`,                 |
//! |     | `ac2_unstack_is_the_exact_inverse_of_stack`,                         |
//! |     | `ac2_invert_swaps_components_and_roundtrips`,                        |
//! |     | `ac2_unison_is_the_identity_for_stack_and_unstack`,                  |
//! |     | `ac2_overflowing_arithmetic_surfaces_typed_error`,                   |
//! |     | `ac2_gcd_cross_cancellation_avoids_spurious_overflow_near_u64_max`   |
//! | AC3 | `ac3_octave_shifts_reduce_to_the_same_canonical_value`,              |
//! |     | `ac3_reduction_lands_in_unit_octave`,                                |
//! |     | `ac3_reduction_is_idempotent`,                                       |
//! |     | `ac3_pure_octaves_reduce_to_unison`,                                 |
//! |     | `ac3_unrepresentable_reduced_form_overflows`                         |
//! | AC4 | `ac4_complexity_sorts_classic_intervals_into_canonical_consonance_order`, |
//! |     | `ac4_complexity_is_strictly_increasing_along_the_canonical_order`    |
//! | AC5 | `ac5_harmonic_grid_odd_limit_9_has_exact_degrees`,                   |
//! |     | `ac5_harmonic_grid_odd_limit_15_has_exact_degrees`,                  |
//! |     | `ac5_even_odd_limit_bounds_the_same_odd_set`,                        |
//! |     | `ac5_odd_limit_1_grid_is_unison_only`,                               |
//! |     | `ac5_odd_limit_0_is_an_empty_grid_error`,                            |
//! |     | `ac5_from_ratios_rejects_empty_input`,                               |
//! |     | `ac5_from_ratios_reduces_dedups_and_roots_at_unison`,                |
//! |     | `ac5_grid_constructors_reject_invalid_root_frequency`               |
//! | AC6 | `ac6_ratio_maps_to_hz_exactly`,                                      |
//! |     | `ac6_to_hz_rejects_invalid_root_frequency`,                          |
//! |     | `ac6_on_grid_frequency_is_a_bitwise_fixed_point`,                    |
//! |     | `ac6_every_grid_degree_to_hz_is_a_fixed_point_of_snap`,              |
//! |     | `ac6_snap_chooses_the_correct_octave`,                               |
//! |     | `ac6_snap_near_miss_reports_positive_cents_offset`,                  |
//! |     | `ac6_snap_wraps_up_to_next_octave_unison`,                           |
//! |     | `ac6_snap_below_root_yields_negative_octave`,                        |
//! |     | `ac6_snap_tie_breaks_to_the_lower_pitch`,                            |
//! |     | `ac6_snap_is_idempotent`,                                            |
//! |     | `ac6_snap_is_deterministic`,                                         |
//! |     | `ac6_snap_rejects_non_finite_or_non_positive_input`,                 |
//! |     | `ac6_snap_rejects_non_finite_log_quotient`                           |
//! | AC7 | `ac7_octave_is_exactly_1200_cents`,                                  |
//! |     | `ac7_unison_is_exactly_0_cents`,                                     |
//! |     | `ac7_fifth_is_701_955_cents_within_a_thousandth`,                    |
//! |     | `ac7_descending_octave_is_exactly_minus_1200_cents`                  |
//! | AC8 | verified at QA sign-off (doc tests + clippy + fmt), not here         |
//!
//! ## API assumptions beyond SPEC-0001 §2/§3
//!
//! The spec fixes the constructor, operation, and error names but leaves a few
//! observability hooks unstated. Where the spec is silent the tests adopt the
//! minimal idiomatic shape and flag it here so any mismatch is caught at the
//! implementation step, not blamed on a test typo:
//!
//! * `Ratio::num() -> u64` / `Ratio::den() -> u64` — public read accessors for
//!   the otherwise-private components (spec §3 marks the fields private). AC1's
//!   "stored reduced to lowest terms" is only directly observable through them.
//! * `impl Display for Ratio` rendering `num:den` (e.g. `"3:2"`) — AC1 phrases
//!   canonical form in `n:d` terms; the spec promises `Display` on `RatioError`
//!   and a documented API, so a matching `Display` on `Ratio` is assumed.
//! * `PitchGrid::degrees() -> &[Ratio]` and `PitchGrid::root_hz() -> f64` —
//!   read accessors for the invariant-bearing fields (spec §2). AC5's exact
//!   degree set is observed through `degrees()`.
//! * `PitchGrid::from_ratios` accepts `IntoIterator<Item = Ratio>` — the spec
//!   writes `from_ratios(root_hz, ratios)` without pinning the container; an
//!   iterator bound is the idiomatic reading and is exercised with both arrays
//!   and `std::iter::empty()`.
//! * `RatioError: PartialEq` — used for `assert_eq!` on `Result`; derivable and
//!   consistent with the spec's `Debug` + `Error` requirements.
//! * `SnappedPitch: PartialEq` with public fields `degree`, `octave`, `hz`,
//!   `cents_offset` — the spec gives the struct literal verbatim (§2), so
//!   constructing and comparing it directly is in-contract.

use std::cmp::Ordering;
use std::collections::HashSet;

use gooz_ratio::{PitchGrid, Ratio, RatioError, SnappedPitch};

/// Test-only constructor for ratio literals with nonzero components.
fn r(num: u64, den: u64) -> Ratio {
    Ratio::new(num, den).expect("test ratio literals have nonzero components")
}

// ---------------------------------------------------------------------------
// AC1 — Canonical form
// ---------------------------------------------------------------------------

#[test]
fn ac1_unreduced_spelling_is_stored_canonical() -> Result<(), RatioError> {
    let unreduced = Ratio::new(6, 4)?;
    // Assumption: num()/den() expose the reduced components (see header).
    assert_eq!(unreduced.num(), 3);
    assert_eq!(unreduced.den(), 2);
    assert_eq!(unreduced, Ratio::new(3, 2)?);
    Ok(())
}

#[test]
fn ac1_equality_hashing_and_ordering_agree_across_spellings() {
    let mut set = HashSet::new();
    set.insert(r(6, 4));
    set.insert(r(3, 2));
    assert_eq!(set.len(), 1, "6:4 and 3:2 must hash identically");
    set.insert(r(2, 1));
    assert_eq!(set.len(), 2);

    assert_eq!(r(6, 4).cmp(&r(3, 2)), Ordering::Equal);
    assert!(r(4, 3) < r(3, 2), "ordering is by musical size");
    assert!(r(3, 2) < r(2, 1));
    assert!(r(1, 2) < r(1, 1), "descending ratios order below unison");
}

#[test]
fn ac1_display_shows_canonical_form() {
    // Assumption: Display renders the reduced `num:den` (see header).
    assert_eq!(r(6, 4).to_string(), "3:2");
    assert_eq!(Ratio::UNISON.to_string(), "1:1");
}

#[test]
fn ac1_zero_components_are_rejected() {
    assert_eq!(Ratio::new(0, 5), Err(RatioError::ZeroComponent));
    assert_eq!(Ratio::new(5, 0), Err(RatioError::ZeroComponent));
    assert_eq!(Ratio::new(0, 0), Err(RatioError::ZeroComponent));
}

#[test]
fn ac1_ratio_error_is_a_typed_std_error() {
    fn assert_std_error<E: std::error::Error>(_: &E) {}
    assert_std_error(&RatioError::ZeroComponent);
    assert!(!RatioError::ZeroComponent.to_string().is_empty());
    assert!(!RatioError::Overflow.to_string().is_empty());
    assert!(!RatioError::InvalidFrequency.to_string().is_empty());
    assert!(!RatioError::EmptyGrid.to_string().is_empty());
}

// ---------------------------------------------------------------------------
// AC2 — Exact interval arithmetic
// ---------------------------------------------------------------------------

#[test]
fn ac2_stacking_fifth_and_fourth_yields_exact_octave() -> Result<(), RatioError> {
    let fifth = Ratio::new(3, 2)?;
    let fourth = Ratio::new(4, 3)?;
    assert_eq!(fifth.stack(fourth)?, Ratio::OCTAVE);
    assert_eq!(fourth.stack(fifth)?, Ratio::OCTAVE);
    Ok(())
}

#[test]
fn ac2_unstack_is_the_exact_inverse_of_stack() -> Result<(), RatioError> {
    let fifth = r(3, 2);
    let fourth = r(4, 3);
    assert_eq!(fifth.stack(fourth)?.unstack(fourth)?, fifth);
    assert_eq!(Ratio::OCTAVE.unstack(fourth)?, fifth);
    assert_eq!(fifth.unstack(fifth)?, Ratio::UNISON);
    Ok(())
}

#[test]
fn ac2_invert_swaps_components_and_roundtrips() {
    let fifth = r(3, 2);
    let inverted = fifth.invert();
    assert_eq!(inverted.num(), 2);
    assert_eq!(inverted.den(), 3);
    assert_eq!(inverted.invert(), fifth);
    assert_eq!(Ratio::UNISON.invert(), Ratio::UNISON);
}

#[test]
fn ac2_unison_is_the_identity_for_stack_and_unstack() -> Result<(), RatioError> {
    let fifth = r(3, 2);
    assert_eq!(fifth.stack(Ratio::UNISON)?, fifth);
    assert_eq!(Ratio::UNISON.stack(fifth)?, fifth);
    assert_eq!(fifth.unstack(Ratio::UNISON)?, fifth);
    Ok(())
}

#[test]
fn ac2_overflowing_arithmetic_surfaces_typed_error() -> Result<(), RatioError> {
    // u64::MAX is odd, so MAX:2 is already in lowest terms and nothing
    // cross-cancels: the products MAX*MAX (and the denominator's) overflow.
    let huge = Ratio::new(u64::MAX, 2)?;
    assert_eq!(huge.stack(huge), Err(RatioError::Overflow));
    assert_eq!(huge.unstack(huge.invert()), Err(RatioError::Overflow));
    Ok(())
}

#[test]
fn ac2_gcd_cross_cancellation_avoids_spurious_overflow_near_u64_max() -> Result<(), RatioError> {
    // The exact result of r * r^-1 is 1:1; cross-cancellation must find it
    // even though the naive intermediate products exceed u64.
    let huge = Ratio::new(u64::MAX, 2)?;
    assert_eq!(huge.stack(huge.invert())?, Ratio::UNISON);
    assert_eq!(huge.unstack(huge)?, Ratio::UNISON);
    Ok(())
}

// ---------------------------------------------------------------------------
// AC3 — Octave equivalence
// ---------------------------------------------------------------------------

fn assert_in_unit_octave(ratio: Ratio) {
    assert!(ratio.num() >= ratio.den(), "{ratio} is below 1:1");
    assert!(
        (ratio.num() as u128) < 2 * ratio.den() as u128,
        "{ratio} is at or above 2:1"
    );
}

#[test]
fn ac3_octave_shifts_reduce_to_the_same_canonical_value() -> Result<(), RatioError> {
    let fifth = r(3, 2);
    assert_eq!(Ratio::new(6, 1)?.reduce_to_octave()?, fifth); // 4r
    assert_eq!(Ratio::new(3, 1)?.reduce_to_octave()?, fifth); // 2r
    assert_eq!(Ratio::new(3, 4)?.reduce_to_octave()?, fifth); // r/2
    assert_eq!(fifth.reduce_to_octave()?, fifth); // r itself
    Ok(())
}

#[test]
fn ac3_reduction_lands_in_unit_octave() -> Result<(), RatioError> {
    for ratio in [r(7, 1), r(9, 1), r(5, 2), r(1, 3), r(15, 8), r(1, 1)] {
        assert_in_unit_octave(ratio.reduce_to_octave()?);
    }
    Ok(())
}

#[test]
fn ac3_reduction_is_idempotent() -> Result<(), RatioError> {
    for ratio in [r(6, 1), r(1, 3), r(3, 2), r(9, 4)] {
        let once = ratio.reduce_to_octave()?;
        assert_eq!(once.reduce_to_octave()?, once);
    }
    Ok(())
}

#[test]
fn ac3_pure_octaves_reduce_to_unison() -> Result<(), RatioError> {
    assert_eq!(Ratio::OCTAVE.reduce_to_octave()?, Ratio::UNISON);
    assert_eq!(Ratio::new(4, 1)?.reduce_to_octave()?, Ratio::UNISON);
    assert_eq!(Ratio::new(1, 2)?.reduce_to_octave()?, Ratio::UNISON);
    Ok(())
}

#[test]
fn ac3_unrepresentable_reduced_form_overflows() -> Result<(), RatioError> {
    // 1:(2^64 - 1) reduces toward 2^64:(2^64 - 1), whose numerator does not
    // fit in u64 — the spec bounds "any ratio" by representability (§2).
    let tiny = Ratio::new(1, u64::MAX)?;
    assert_eq!(tiny.reduce_to_octave(), Err(RatioError::Overflow));
    Ok(())
}

// ---------------------------------------------------------------------------
// AC4 — Consonance ordering
// ---------------------------------------------------------------------------

fn canonical_consonance_order() -> Vec<Ratio> {
    vec![
        r(1, 1),
        r(2, 1),
        r(3, 2),
        r(4, 3),
        r(5, 3),
        r(5, 4),
        r(6, 5),
        r(9, 8),
        r(16, 15),
    ]
}

#[test]
fn ac4_complexity_sorts_classic_intervals_into_canonical_consonance_order() {
    let mut intervals = vec![
        r(16, 15),
        r(9, 8),
        r(6, 5),
        r(5, 4),
        r(5, 3),
        r(4, 3),
        r(3, 2),
        r(2, 1),
        r(1, 1),
    ];
    intervals.sort_by(|a, b| a.complexity().total_cmp(&b.complexity()));
    assert_eq!(intervals, canonical_consonance_order());
}

#[test]
fn ac4_complexity_is_strictly_increasing_along_the_canonical_order() {
    // Tenney height log2(num*den): unison is 0, then strictly monotone.
    assert_eq!(Ratio::UNISON.complexity(), 0.0);
    for pair in canonical_consonance_order().windows(2) {
        assert!(
            pair[0].complexity() < pair[1].complexity(),
            "complexity({}) must be strictly below complexity({})",
            pair[0],
            pair[1]
        );
    }
}

// ---------------------------------------------------------------------------
// AC5 — Harmonic pitch grids
// ---------------------------------------------------------------------------

#[test]
fn ac5_harmonic_grid_odd_limit_9_has_exact_degrees() -> Result<(), RatioError> {
    // The requirement's worked example (AC5): odd harmonics 1,3,5,7,9 octave-
    // reduced, deduped, sorted ascending, starting at 1:1.
    let grid = PitchGrid::harmonic(220.0, 9)?;
    assert_eq!(
        grid.degrees(),
        &[r(1, 1), r(9, 8), r(5, 4), r(3, 2), r(7, 4)]
    );
    assert_eq!(grid.root_hz(), 220.0);
    Ok(())
}

#[test]
fn ac5_harmonic_grid_odd_limit_15_has_exact_degrees() -> Result<(), RatioError> {
    // Second odd limit (independently computed): odd harmonics 1..=15.
    let grid = PitchGrid::harmonic(220.0, 15)?;
    assert_eq!(
        grid.degrees(),
        &[
            r(1, 1),
            r(9, 8),
            r(5, 4),
            r(11, 8),
            r(3, 2),
            r(13, 8),
            r(7, 4),
            r(15, 8),
        ]
    );
    Ok(())
}

#[test]
fn ac5_even_odd_limit_bounds_the_same_odd_set() -> Result<(), RatioError> {
    // Spec §2: an even odd_limit simply bounds the same odd set
    // (harmonic(root, 8) === harmonic(root, 7)).
    let even = PitchGrid::harmonic(220.0, 8)?;
    let odd = PitchGrid::harmonic(220.0, 7)?;
    assert_eq!(even.degrees(), odd.degrees());
    assert_eq!(even.degrees(), &[r(1, 1), r(5, 4), r(3, 2), r(7, 4)]);
    Ok(())
}

#[test]
fn ac5_odd_limit_1_grid_is_unison_only() -> Result<(), RatioError> {
    let grid = PitchGrid::harmonic(220.0, 1)?;
    assert_eq!(grid.degrees(), &[Ratio::UNISON]);
    Ok(())
}

#[test]
fn ac5_odd_limit_0_is_an_empty_grid_error() {
    assert!(matches!(
        PitchGrid::harmonic(220.0, 0),
        Err(RatioError::EmptyGrid)
    ));
}

#[test]
fn ac5_from_ratios_rejects_empty_input() {
    assert!(matches!(
        PitchGrid::from_ratios(220.0, std::iter::empty::<Ratio>()),
        Err(RatioError::EmptyGrid)
    ));
}

#[test]
fn ac5_from_ratios_reduces_dedups_and_roots_at_unison() -> Result<(), RatioError> {
    // 6:4 == 3:2, 3:1 reduces to 3:2, 7:2 reduces to 7:4; 1:1 is inserted.
    let grid = PitchGrid::from_ratios(220.0, [r(3, 2), r(6, 4), r(3, 1), r(7, 2)])?;
    assert_eq!(grid.degrees(), &[r(1, 1), r(3, 2), r(7, 4)]);
    assert_eq!(grid.degrees()[0], Ratio::UNISON, "1:1 is always first");
    Ok(())
}

#[test]
fn ac5_grid_constructors_reject_invalid_root_frequency() {
    for bad_root in [0.0, -220.0, f64::NAN, f64::INFINITY] {
        assert!(matches!(
            PitchGrid::harmonic(bad_root, 9),
            Err(RatioError::InvalidFrequency)
        ));
        assert!(matches!(
            PitchGrid::from_ratios(bad_root, [r(3, 2)]),
            Err(RatioError::InvalidFrequency)
        ));
    }
}

// ---------------------------------------------------------------------------
// AC6 — Frequency mapping & snapping
// ---------------------------------------------------------------------------

#[test]
fn ac6_ratio_maps_to_hz_exactly() -> Result<(), RatioError> {
    // AC6 worked example: 3:2 over root 220 Hz -> 330 Hz, exactly.
    assert_eq!(Ratio::new(3, 2)?.to_hz(220.0)?, 330.0);
    assert_eq!(Ratio::UNISON.to_hz(220.0)?, 220.0);
    assert_eq!(Ratio::OCTAVE.to_hz(220.0)?, 440.0);
    Ok(())
}

#[test]
fn ac6_to_hz_rejects_invalid_root_frequency() {
    for bad_root in [0.0, -5.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(r(3, 2).to_hz(bad_root), Err(RatioError::InvalidFrequency));
    }
}

#[test]
fn ac6_on_grid_frequency_is_a_bitwise_fixed_point() -> Result<(), RatioError> {
    let grid = PitchGrid::harmonic(220.0, 9)?;
    assert_eq!(
        grid.snap(330.0)?,
        SnappedPitch {
            degree: r(3, 2),
            octave: 0,
            hz: 330.0,
            cents_offset: 0.0,
        }
    );
    Ok(())
}

#[test]
fn ac6_every_grid_degree_to_hz_is_a_fixed_point_of_snap() -> Result<(), RatioError> {
    let grid = PitchGrid::harmonic(220.0, 9)?;
    for &degree in grid.degrees() {
        let hz = degree.to_hz(220.0)?;
        let snapped = grid.snap(hz)?;
        assert_eq!(snapped.degree, degree);
        assert_eq!(snapped.octave, 0);
        assert_eq!(snapped.hz, hz, "to_hz output must be a bitwise fixed point");
        assert_eq!(snapped.cents_offset, 0.0);
    }
    Ok(())
}

#[test]
fn ac6_snap_chooses_the_correct_octave() -> Result<(), RatioError> {
    // The requirement's own example: 660 Hz against root 220 is 3:2 one
    // octave up (660 Hz), not 3:2 in the root octave (330 Hz).
    let grid = PitchGrid::harmonic(220.0, 9)?;
    let snapped = grid.snap(660.0)?;
    assert_eq!(snapped.degree, r(3, 2));
    assert_eq!(snapped.octave, 1);
    assert_eq!(snapped.hz, 660.0);
    assert!(snapped.cents_offset.abs() < 1e-9);
    Ok(())
}

#[test]
fn ac6_snap_near_miss_reports_positive_cents_offset() -> Result<(), RatioError> {
    let grid = PitchGrid::harmonic(220.0, 9)?;
    let snapped = grid.snap(445.0)?;
    assert_eq!(snapped.degree, Ratio::UNISON);
    assert_eq!(snapped.octave, 1);
    assert_eq!(snapped.hz, 440.0);
    assert!(snapped.cents_offset > 0.0, "445 Hz sits above 440 Hz");
    // cents_offset = input relative to snapped (input - snapped), in cents.
    let expected = 1200.0 * (445.0_f64 / 440.0).log2(); // ~= +19.56 cents
    assert!((snapped.cents_offset - expected).abs() < 1e-6);
    Ok(())
}

#[test]
fn ac6_snap_wraps_up_to_next_octave_unison() -> Result<(), RatioError> {
    // 438 Hz is just under 2x root: the nearest pitch is 1:1 in the next
    // octave (440 Hz), not a lower degree of the current octave. This is the
    // ±1-octave-candidate wrap-around path in spec §2.
    let grid = PitchGrid::harmonic(220.0, 9)?;
    let snapped = grid.snap(438.0)?;
    assert_eq!(snapped.degree, Ratio::UNISON);
    assert_eq!(snapped.octave, 1);
    assert_eq!(snapped.hz, 440.0);
    assert!(snapped.cents_offset < 0.0, "438 Hz sits below 440 Hz");
    let expected = 1200.0 * (438.0_f64 / 440.0).log2(); // ~= -7.89 cents
    assert!((snapped.cents_offset - expected).abs() < 1e-6);
    Ok(())
}

#[test]
fn ac6_snap_below_root_yields_negative_octave() -> Result<(), RatioError> {
    let grid = PitchGrid::harmonic(220.0, 9)?;
    let snapped = grid.snap(110.0)?;
    assert_eq!(snapped.degree, Ratio::UNISON);
    assert_eq!(snapped.octave, -1);
    assert_eq!(snapped.hz, 110.0);
    assert_eq!(snapped.cents_offset, 0.0);
    Ok(())
}

#[test]
fn ac6_snap_tie_breaks_to_the_lower_pitch() -> Result<(), RatioError> {
    // A two-degree grid {1:1, 2:1-as-unison-next-octave} via {1:1, 3:2}: pick
    // an input exactly midway (in log space) between two adjacent candidates
    // and confirm the deterministic lower-pitch tie-break (spec §2).
    //
    // Grid {1:1, 3:2} on root 1 Hz. Candidates in log2: 0.0 (unison) and
    // log2(1.5) = 0.58496. Their midpoint is 0.29248, i.e. 2^0.29248 Hz.
    let grid = PitchGrid::from_ratios(1.0, [r(3, 2)])?;
    let midpoint_hz = 2f64.powf((3f64 / 2f64).log2() / 2.0);
    let snapped = grid.snap(midpoint_hz)?;
    assert_eq!(
        snapped.degree,
        Ratio::UNISON,
        "an exact tie resolves to the lower-pitched candidate"
    );
    assert_eq!(snapped.octave, 0);
    assert!(snapped.cents_offset > 0.0, "input sits above the unison");
    Ok(())
}

#[test]
fn ac6_snap_is_idempotent() -> Result<(), RatioError> {
    let grid = PitchGrid::harmonic(220.0, 9)?;

    // Off-grid input: re-snapping the snapped frequency is a no-op.
    let first = grid.snap(445.0)?;
    let resnapped = grid.snap(first.hz)?;
    assert_eq!(resnapped.degree, first.degree);
    assert_eq!(resnapped.octave, first.octave);
    assert_eq!(resnapped.hz, first.hz);
    assert_eq!(resnapped.cents_offset, 0.0);

    // On-grid input: the full result is a fixed point.
    let on_grid = grid.snap(330.0)?;
    assert_eq!(grid.snap(on_grid.hz)?, on_grid);
    Ok(())
}

#[test]
fn ac6_snap_is_deterministic() -> Result<(), RatioError> {
    let grid = PitchGrid::harmonic(220.0, 9)?;
    for hz in [445.0, 438.0, 330.0, 110.0, 1234.5] {
        assert_eq!(grid.snap(hz)?, grid.snap(hz)?);
    }
    Ok(())
}

#[test]
fn ac6_snap_rejects_non_finite_or_non_positive_input() -> Result<(), RatioError> {
    let grid = PitchGrid::harmonic(220.0, 9)?;
    for bad_hz in [0.0, -5.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(grid.snap(bad_hz), Err(RatioError::InvalidFrequency));
    }
    Ok(())
}

#[test]
fn ac6_snap_rejects_non_finite_log_quotient() -> Result<(), RatioError> {
    // Valid root, valid input, but hz / root overflows (or underflows) f64:
    // the log-quotient is non-finite and must be rejected (spec §2), not
    // folded into a garbage octave.
    let tiny_root = PitchGrid::harmonic(1e-308, 9)?;
    assert_eq!(tiny_root.snap(1e308), Err(RatioError::InvalidFrequency));

    let huge_root = PitchGrid::harmonic(1e308, 9)?;
    assert_eq!(huge_root.snap(1e-308), Err(RatioError::InvalidFrequency));
    Ok(())
}

// ---------------------------------------------------------------------------
// AC7 — Cents
// ---------------------------------------------------------------------------

#[test]
fn ac7_octave_is_exactly_1200_cents() {
    assert_eq!(Ratio::OCTAVE.cents(), 1200.0);
}

#[test]
fn ac7_unison_is_exactly_0_cents() {
    assert_eq!(Ratio::UNISON.cents(), 0.0);
}

#[test]
fn ac7_fifth_is_701_955_cents_within_a_thousandth() -> Result<(), RatioError> {
    let fifth = Ratio::new(3, 2)?;
    assert!((fifth.cents() - 701.955).abs() < 0.001);
    Ok(())
}

#[test]
fn ac7_descending_octave_is_exactly_minus_1200_cents() -> Result<(), RatioError> {
    assert_eq!(Ratio::new(1, 2)?.cents(), -1200.0);
    Ok(())
}
