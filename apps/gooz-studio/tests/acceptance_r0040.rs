//! R-0040 acceptance tests — a recording becomes an instrument you can hear.
//!
//! Deviceless: the "recordings" are synthesized here, so these run in CI with
//! no microphone. The device half (the Tauri command) is covered by the
//! desktop-shell build job.

use gooz_studio::{DspError, instrument_from_take};

const SR: u32 = 48_000;
const TENSE: u8 = 50;

/// A hummed note: something with a pitch in it.
fn hum() -> Vec<f32> {
    (0..SR as usize / 2)
        .map(|i| 0.8 * (std::f64::consts::TAU * 220.0 * i as f64 / f64::from(SR)).sin() as f32)
        .collect()
}

/// A knock on the table: no pitch to find anywhere in it.
fn knock() -> Vec<f32> {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    (0..SR as usize / 8)
        .map(|i| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let white = (state >> 40) as f32 / 8_388_608.0 - 1.0;
            white * (-14.0 * i as f32 / (SR as f32 / 8.0)).exp() * 0.9
        })
        .collect()
}

#[test]
fn ac1_a_recording_plays_across_the_grid_one_hit_per_degree() {
    let view = instrument_from_take(&hum(), SR, TENSE).expect("a hum is a valid take");
    assert!(!view.samples.is_empty(), "nothing to play");
    assert!(
        view.notes.len() >= 3,
        "a grid has more than a couple of degrees"
    );
    assert_eq!(view.sample_rate, SR);
    assert!(view.seconds > 0.0 && view.bars >= 1);
}

#[test]
fn ac2_a_knock_is_as_playable_as_a_hum() {
    // Nothing in this path may require a detectable pitch.
    let knocked = instrument_from_take(&knock(), SR, TENSE).expect("a knock is a valid take");
    let hummed = instrument_from_take(&hum(), SR, TENSE).expect("a hum is a valid take");
    assert!(
        !knocked.samples.is_empty(),
        "a knock must become an instrument"
    );
    assert_eq!(
        knocked.notes.len(),
        hummed.notes.len(),
        "the scale does not depend on what was recorded"
    );
}

#[test]
fn ac3_the_note_cards_are_the_scale_in_order() {
    let view = instrument_from_take(&hum(), SR, TENSE).expect("take");
    // Ascending, and the first degree is the unison — the sound as recorded.
    assert_eq!((view.notes[0].num, view.notes[0].den), (1, 1));
    let ratios: Vec<f64> = view
        .notes
        .iter()
        .map(|n| n.num as f64 / n.den as f64)
        .collect();
    for pair in ratios.windows(2) {
        assert!(pair[1] > pair[0], "degrees are not ascending: {ratios:?}");
    }
    assert!(
        ratios.iter().all(|r| (1.0..2.0).contains(r)),
        "every card should be an octave-reduced degree: {ratios:?}"
    );
}

#[test]
fn ac4_the_slider_changes_the_scale() {
    let smooth = instrument_from_take(&hum(), SR, 0).expect("take");
    let tense = instrument_from_take(&hum(), SR, 100).expect("take");
    assert!(
        tense.notes.len() >= smooth.notes.len(),
        "a tenser grid must not have fewer degrees"
    );
    let smooth_set: Vec<(u64, u64)> = smooth.notes.iter().map(|n| (n.num, n.den)).collect();
    let tense_set: Vec<(u64, u64)> = tense.notes.iter().map(|n| (n.num, n.den)).collect();
    for degree in &smooth_set {
        assert!(
            tense_set.contains(degree),
            "tensing dropped {degree:?} — the scale should only deepen"
        );
    }
    assert_ne!(smooth_set, tense_set, "the slider did nothing");
}

#[test]
fn ac5_a_failed_recording_is_a_typed_error() {
    assert_eq!(
        instrument_from_take(&[], SR, TENSE).unwrap_err(),
        DspError::EmptySignal
    );
    assert_eq!(
        instrument_from_take(&hum(), 0, TENSE).unwrap_err(),
        DspError::InvalidSampleRate
    );
    let mut broken = hum();
    broken[100] = f32::NAN;
    assert_eq!(
        instrument_from_take(&broken, SR, TENSE).unwrap_err(),
        DspError::NonFiniteSample
    );
    // A take that is not audio is refused too — the invariant `Sampler` gained
    // after QA found an overlapping mix of out-of-range samples rendering NaN.
    let mut too_hot = hum();
    too_hot[100] = 4.0;
    assert_eq!(
        instrument_from_take(&too_hot, SR, TENSE).unwrap_err(),
        DspError::SampleOutOfRange
    );
}

#[test]
fn ac6_the_same_take_always_sounds_the_same_and_stays_bounded() {
    let take = hum();
    let first = instrument_from_take(&take, SR, TENSE).expect("take");
    let second = instrument_from_take(&take, SR, TENSE).expect("take");
    assert_eq!(first, second, "not deterministic");
    assert!(
        first
            .samples
            .iter()
            .all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-6),
        "the riff left [-1, 1]"
    );
    assert!(first.wave.iter().all(|s| s.is_finite()));
}

#[test]
fn every_degree_actually_sounds() {
    // The sampler skips notes it cannot render, silently and by design. If that
    // ever swallows a degree, the cards would still show a scale nobody hears.
    let view = instrument_from_take(&knock(), SR, TENSE).expect("take");
    let beat = 60.0 / 92.0; // Easy Mode's tempo
    let window = SR as usize / 40;
    for (i, note) in view.notes.iter().enumerate() {
        let onset = (i as f64 * beat * f64::from(SR)) as usize;
        let energy: f32 = view.samples[onset..onset + window]
            .iter()
            .map(|s| s * s)
            .sum();
        assert!(
            energy > 1e-6,
            "degree {}:{} is on a card but never sounds",
            note.num,
            note.den
        );
    }
}
