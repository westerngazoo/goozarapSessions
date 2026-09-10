//! R-0038 acceptance tests — ratio-native pitch shift.
//!
//! The shifts are verified by **measuring the result with the crate's own YIN
//! tracker** rather than by trusting the resampling arithmetic: if the audio
//! does not actually come out at the requested pitch, these fail.

use gooz_dsp::{Config, DspError, Ratio, pitch_track, shift_pitch};

const SR: u32 = 48_000;

/// One second of a steady sine at `hz`.
fn sine(hz: f64) -> Vec<f32> {
    (0..SR as usize)
        .map(|i| 0.8 * (std::f64::consts::TAU * hz * i as f64 / f64::from(SR)).sin() as f32)
        .collect()
}

/// The median fundamental YIN reports for a signal, in Hz.
fn measured_hz(signal: &[f32]) -> f64 {
    let track = pitch_track(signal, SR, &Config::default()).expect("a trackable signal");
    let mut found: Vec<f64> = track
        .frames
        .iter()
        .filter_map(|f| f.f0_hz)
        .map(f64::from)
        .collect();
    assert!(!found.is_empty(), "YIN found no pitch to measure");
    found.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    found[found.len() / 2]
}

/// Distance between two frequencies in cents — the musical unit for "how close".
fn cents_apart(a: f64, b: f64) -> f64 {
    1200.0 * (a / b).log2().abs()
}

#[test]
fn ac1_shifting_by_a_ratio_moves_the_measured_pitch_by_that_ratio() {
    let source = sine(220.0);
    for (num, den, expected) in [(3, 2, 330.0), (5, 4, 275.0), (2, 1, 440.0)] {
        let ratio = Ratio::new(num, den).expect("a valid ratio");
        let shifted = shift_pitch(&source, SR, ratio).expect("shift succeeds");
        let measured = measured_hz(&shifted);
        assert!(
            cents_apart(measured, expected) < 20.0,
            "{num}:{den} should sound at {expected} Hz, measured {measured:.1} Hz"
        );
    }
}

#[test]
fn ac2_unison_is_an_exact_identity() {
    let source = sine(220.0);
    assert_eq!(
        shift_pitch(&source, SR, Ratio::UNISON).expect("shift"),
        source
    );
}

#[test]
fn ac3_shifting_down_works_and_a_ratio_undoes_its_inverse() {
    let source = sine(330.0);
    let down = shift_pitch(&source, SR, Ratio::new(2, 3).expect("ratio")).expect("shift");
    assert!(
        cents_apart(measured_hz(&down), 220.0) < 20.0,
        "2:3 should drop 330 Hz to 220 Hz"
    );
    // Up a fifth, then back down a fifth: the pitch returns.
    let up = shift_pitch(&source, SR, Ratio::new(3, 2).expect("ratio")).expect("shift");
    let back = shift_pitch(&up, SR, Ratio::new(2, 3).expect("ratio")).expect("shift");
    assert!(
        cents_apart(measured_hz(&back), 330.0) < 20.0,
        "a ratio followed by its inverse must recover the original pitch"
    );
}

#[test]
fn ac4_bad_input_is_a_typed_error_and_never_a_panic() {
    let fifth = Ratio::new(3, 2).expect("ratio");
    assert!(matches!(
        shift_pitch(&[], SR, fifth),
        Err(DspError::EmptySignal)
    ));
    assert!(matches!(
        shift_pitch(&[0.1, 0.2], 0, fifth),
        Err(DspError::InvalidSampleRate)
    ));
    assert!(matches!(
        shift_pitch(&[0.1, f32::NAN], SR, fifth),
        Err(DspError::NonFiniteSample)
    ));
    // A single sample has no neighbour to interpolate toward: still no panic.
    assert!(shift_pitch(&[0.5], SR, fifth).is_ok());
}

#[test]
fn ac5_output_is_deterministic_finite_and_stays_bounded() {
    let source = sine(220.0);
    for (num, den) in [(3, 2), (2, 3), (16, 15), (2, 1)] {
        let ratio = Ratio::new(num, den).expect("ratio");
        let first = shift_pitch(&source, SR, ratio).expect("shift");
        let second = shift_pitch(&source, SR, ratio).expect("shift");
        assert_eq!(first, second, "{num}:{den} is not deterministic");
        assert!(
            first.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
            "{num}:{den} left [-1, 1]"
        );
        assert!(!first.is_empty());
    }
}

#[test]
fn length_scales_by_the_inverse_of_the_ratio() {
    let source = sine(220.0);
    let up = shift_pitch(&source, SR, Ratio::new(2, 1).expect("ratio")).expect("shift");
    let down = shift_pitch(&source, SR, Ratio::new(1, 2).expect("ratio")).expect("shift");
    // Varispeed: an octave up halves the length, an octave down doubles it.
    assert!((up.len() as f64 - source.len() as f64 / 2.0).abs() <= 1.0);
    assert!((down.len() as f64 - source.len() as f64 * 2.0).abs() <= 1.0);
}
