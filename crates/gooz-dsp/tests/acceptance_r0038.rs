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

#[test]
fn ac4_an_extreme_ratio_is_a_typed_error_not_a_capacity_panic() {
    // `Ratio` guarantees positive and non-zero, but not *bounded*: a tiny step
    // asks for an output the machine cannot hold. That must be a typed error.
    let absurd = Ratio::new(1, u64::MAX).expect("ratio");
    assert!(matches!(
        shift_pitch(&[0.5], SR, absurd),
        Err(DspError::OutputTooLong)
    ));
    // Short of overflow, an implausibly long output is refused just as clearly:
    // one second shifted down by 1:10_000 would be nearly three hours of audio.
    let source = sine(220.0);
    assert!(matches!(
        shift_pitch(&source, SR, Ratio::new(1, 10_000).expect("ratio")),
        Err(DspError::OutputTooLong)
    ));
}

#[test]
fn golden_ramp_reads_every_other_sample_when_shifted_an_octave_up() {
    // Five samples at double speed: positions 0, 2, 4 — and the length rounds
    // *up*, so the final sample is not dropped.
    let ramp: Vec<f32> = (0..5).map(|i| i as f32).collect();
    let up = shift_pitch(&ramp, SR, Ratio::new(2, 1).expect("ratio")).expect("shift");
    assert_eq!(up, vec![0.0, 2.0, 4.0]);
}

#[test]
fn golden_ramp_interpolates_between_samples_on_a_fractional_step() {
    // Step 1.5 lands between samples on every odd read. Nearest-neighbour would
    // give [0, 1, 3, 4]; linear interpolation is the decision the spec defends.
    let ramp: Vec<f32> = (0..6).map(|i| i as f32).collect();
    let shifted = shift_pitch(&ramp, SR, Ratio::new(3, 2).expect("ratio")).expect("shift");
    assert_eq!(shifted, vec![0.0, 1.5, 3.0, 4.5]);
}

#[test]
fn output_length_is_exact_integer_arithmetic_not_float_rounding() {
    // 5 · 49 = 245 exactly, but `(5.0 / (1.0 / 49.0)).ceil()` is 246: the float
    // round-trip overshoots and duplicates the final sample.
    let ramp: Vec<f32> = (0..5).map(|i| i as f32).collect();
    let stretched = shift_pitch(&ramp, SR, Ratio::new(1, 49).expect("ratio")).expect("shift");
    assert_eq!(stretched.len(), 245);
}

#[test]
fn the_shift_changes_pitch_without_changing_level() {
    // A gain bug anywhere in the read path would sail past the bounds check in
    // `ac5`, because a 0.8-amplitude source has room to grow before it clips.
    let source = sine(220.0);
    let rms = |s: &[f32]| {
        (s.iter().map(|x| f64::from(*x) * f64::from(*x)).sum::<f64>() / s.len() as f64).sqrt()
    };
    for (num, den) in [(3, 2), (2, 3), (2, 1)] {
        let shifted =
            shift_pitch(&source, SR, Ratio::new(num, den).expect("ratio")).expect("shift");
        let ratio_of_levels = rms(&shifted) / rms(&source);
        assert!(
            (ratio_of_levels - 1.0).abs() < 0.05,
            "{num}:{den} changed the level by {:.1}%",
            (ratio_of_levels - 1.0) * 100.0
        );
    }
}

/// The energy at one frequency, via Goertzel — cheaper than a full FFT and
/// enough to ask "did the tone land where we asked for it?".
fn magnitude_at(signal: &[f32], hz: f64) -> f64 {
    let omega = std::f64::consts::TAU * hz / f64::from(SR);
    let coeff = 2.0 * omega.cos();
    let (mut previous, mut before_that) = (0.0f64, 0.0f64);
    for &sample in signal {
        let current = f64::from(sample) + coeff * previous - before_that;
        before_that = previous;
        previous = current;
    }
    let power = previous * previous + before_that * before_that - coeff * previous * before_that;
    2.0 * power.max(0.0).sqrt() / signal.len() as f64
}

#[test]
fn upward_shifts_fold_content_above_nyquist_a_documented_limitation() {
    // Reading faster than the source is decimation, and there is no pre-filter,
    // so a tone whose target sits above Nyquist comes back folded. Classic
    // sampler behaviour, and fine for the one-shots R-0039 wants — but it must
    // be pinned, not assumed away. Adding a decimation filter will change this.
    let source = sine(15_000.0);
    let octave_up = shift_pitch(&source, SR, Ratio::new(2, 1).expect("ratio")).expect("shift");
    // Asked for 30 kHz at a 24 kHz Nyquist; 48 − 30 = 18 kHz is where it lands.
    let folded = magnitude_at(&octave_up, 18_000.0);
    assert!(
        folded > 0.5,
        "the alias should carry the signal's energy, measured {folded:.3}"
    );
    assert!(
        folded > magnitude_at(&octave_up, 15_000.0) * 10.0,
        "the folded tone should dominate what remains at the source frequency"
    );
}
