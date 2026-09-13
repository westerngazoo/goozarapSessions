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

#[test]
fn below_the_fold_threshold_the_tone_lands_exactly_where_it_was_asked_for() {
    // The other half of the documented limitation: *under* `SR / (2·ratio)`
    // nothing folds. Without this, the aliasing test above could be read as
    // "shifts land somewhere unpredictable"; with it, the boundary is the claim.
    let source = sine(10_000.0);
    let up = shift_pitch(&source, SR, Ratio::new(3, 2).expect("ratio")).expect("shift");
    let asked_for = magnitude_at(&up, 15_000.0);
    assert!(
        asked_for > 0.5,
        "15 kHz is under the 24 kHz Nyquist and must carry the energy, measured {asked_for:.3}"
    );
    assert!(
        asked_for > magnitude_at(&up, 10_000.0) * 10.0,
        "the tone must move off its source frequency"
    );
}

#[test]
fn ac1_a_rich_timbre_shifts_as_truly_as_a_lab_sine() {
    // R-0038 exists to move *recordings*, which are never single sinusoids. A
    // sawtooth carries a full harmonic stack, so an interpolation error that a
    // sine hides shows up here as a mistracked fundamental.
    let saw: Vec<f32> = (0..SR as usize)
        .map(|i| {
            let phase = (220.0 * i as f64 / f64::from(SR)).fract();
            (0.8 * (2.0 * phase - 1.0)) as f32
        })
        .collect();
    for (num, den, expected) in [(3, 2, 330.0), (2, 3, 220.0 * 2.0 / 3.0), (2, 1, 440.0)] {
        let shifted = shift_pitch(&saw, SR, Ratio::new(num, den).expect("ratio")).expect("shift");
        let measured = measured_hz(&shifted);
        assert!(
            cents_apart(measured, expected) < 20.0,
            "{num}:{den} should sound at {expected:.1} Hz, measured {measured:.1} Hz"
        );
    }
}

#[test]
fn ac3_shifting_down_then_up_also_recovers_the_pitch_and_the_length() {
    // The suite already walks up-then-down. Down-then-up is the other order,
    // and it is the one where the round trip passes through a *longer* buffer,
    // so a length-rounding error compounds instead of cancelling.
    let source = sine(330.0);
    let down = shift_pitch(&source, SR, Ratio::new(2, 3).expect("ratio")).expect("shift");
    let back = shift_pitch(&down, SR, Ratio::new(3, 2).expect("ratio")).expect("shift");
    assert!(
        cents_apart(measured_hz(&back), 330.0) < 20.0,
        "down a fifth then up a fifth must return to 330 Hz"
    );
    assert!(
        back.len().abs_diff(source.len()) <= 1,
        "the round trip returned {} samples, not {}",
        back.len(),
        source.len()
    );
}

#[test]
fn ac4_an_infinite_sample_is_a_typed_error_just_like_a_nan() {
    // "Non-finite" is two things, and only NaN was pinned.
    let fifth = Ratio::new(3, 2).expect("ratio");
    for poison in [f32::INFINITY, f32::NEG_INFINITY] {
        assert!(
            matches!(
                shift_pitch(&[0.1, poison, 0.2], SR, fifth),
                Err(DspError::NonFiniteSample)
            ),
            "{poison} was not reported as a non-finite sample"
        );
    }
}

#[test]
fn ac4_no_ratio_at_any_scale_and_no_sample_rate_panics() {
    // `Ratio` is unbounded in *both* directions and `sample_rate` is a raw
    // `u32`; the only promise is that every combination either works or says
    // why. Each case here either returns audio or a typed error — reaching the
    // end of this test at all is the assertion.
    let signals: [Vec<f32>; 4] = [
        vec![0.5],
        vec![0.5, -0.5],
        vec![0.5, -0.5, 0.25],
        (0..10).map(|i| i as f32 / 10.0 - 0.5).collect(),
    ];
    let ratios = [
        (1, u64::MAX),
        (u64::MAX, 1),
        (u64::MAX, u64::MAX - 1),
        (u64::MAX - 1, u64::MAX),
        (2, u64::MAX),
        (u64::MAX, 2),
        (1u64 << 62, 1),
        (1, 1u64 << 62),
        (u64::MAX / 2, u64::MAX),
        (7, 3),
        (1, 1),
    ];
    for signal in &signals {
        for (num, den) in ratios {
            let ratio = Ratio::new(num, den).expect("a positive ratio");
            for sample_rate in [1u32, 2, 44_100, 48_000, u32::MAX] {
                match shift_pitch(signal, sample_rate, ratio) {
                    Ok(out) => {
                        assert!(
                            !out.is_empty() && out.iter().all(|s| s.is_finite()),
                            "{num}:{den} at {sample_rate} Hz produced unusable audio"
                        );
                        assert!(
                            out.len() <= 600 * sample_rate as usize,
                            "{num}:{den} at {sample_rate} Hz produced more than the ten-minute cap"
                        );
                    }
                    Err(e) => assert_eq!(
                        e,
                        DspError::OutputTooLong,
                        "{num}:{den} at {sample_rate} Hz failed for the wrong reason"
                    ),
                }
            }
        }
    }
}

#[test]
fn ac4_the_length_cap_is_ten_minutes_at_the_given_rate_on_both_sides() {
    // `OutputTooLong` is only meaningful if it has an edge. At 100 Hz the cap
    // is 60 000 samples: one below is audio, one above is the error. This also
    // pins that the cap is a *duration* — the same ratio passes or fails
    // depending only on the sample rate.
    let one_sample = [0.5f32];
    let at_the_cap = shift_pitch(&one_sample, 100, Ratio::new(1, 60_000).expect("ratio"));
    assert_eq!(at_the_cap.map(|out| out.len()), Ok(60_000));
    assert_eq!(
        shift_pitch(&one_sample, 100, Ratio::new(1, 60_001).expect("ratio")),
        Err(DspError::OutputTooLong)
    );
    // Same ratio, faster rate: comfortably inside ten minutes, so it is audio.
    assert!(shift_pitch(&one_sample, 48_000, Ratio::new(1, 60_001).expect("ratio")).is_ok());
}

#[test]
fn ac5_a_full_scale_input_never_leaves_the_unit_interval() {
    // The bounds check in `ac5` runs on a 0.8-amplitude sine, which has 2 dB of
    // headroom to absorb an interpolation overshoot. A full-scale alternating
    // signal is the worst case the claim `|lerp(a, b)| <= max(|a|, |b|)` has to
    // survive: every read sits between +1 and −1.
    let full_scale: Vec<f32> = (0..1_000)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    for (num, den) in [(3, 2), (2, 3), (5, 4), (16, 15), (2, 1), (1, 2)] {
        let shifted =
            shift_pitch(&full_scale, SR, Ratio::new(num, den).expect("ratio")).expect("shift");
        assert!(
            shifted.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
            "{num}:{den} pushed a full-scale signal outside [-1, 1]"
        );
    }
}

#[test]
fn ac2_identity_is_exact_for_values_that_are_not_musical() {
    // Identity has to mean identity, not "close enough for audio": denormals,
    // signed zero and the extremes of the type come back bit for bit.
    let exotic = vec![
        f32::MIN,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::from_bits(1), // the smallest denormal
        -0.0,
        0.0,
        1.0,
        -1.0,
    ];
    let out = shift_pitch(&exotic, SR, Ratio::UNISON).expect("shift");
    let bits = |v: &[f32]| v.iter().map(|s| s.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&out), bits(&exotic), "unison altered a sample");
}

#[test]
fn the_final_sample_is_held_not_faded_into_silence() {
    // A downward shift reads past the last input sample on its final steps.
    // Holding the edge keeps the tail of the sound; treating the missing
    // neighbour as silence would ramp every stretched buffer down to zero —
    // an audible click at the end of every transposed one-shot.
    let ramp: Vec<f32> = (0..3).map(|i| i as f32).collect();
    let stretched = shift_pitch(&ramp, SR, Ratio::new(2, 3).expect("ratio")).expect("shift");
    assert_eq!(stretched.len(), 5);
    assert_eq!(stretched[3], 2.0);
    assert_eq!(
        stretched[4], 2.0,
        "the read past the end must hold the last sample, not fade to zero"
    );
}

#[test]
fn the_sample_rate_does_not_change_which_samples_are_read() {
    // The module documents resampling as dimensionless: the rate decides only
    // whether the result is too long, never what it contains. If that ever
    // stops being true, every caller that resamples at one rate and plays at
    // another is silently wrong.
    let source = sine(220.0);
    let ratio = Ratio::new(3, 2).expect("ratio");
    let reference = shift_pitch(&source, 48_000, ratio).expect("shift");
    for sample_rate in [1_000u32, 8_000, 44_100, 96_000, 192_000, u32::MAX] {
        assert_eq!(
            shift_pitch(&source, sample_rate, ratio).expect("shift"),
            reference,
            "the output changed at {sample_rate} Hz"
        );
    }
}

#[test]
fn ac5_a_finite_input_can_never_produce_a_non_finite_output() {
    // The crate rejects a non-finite *input*; it must not invent one on the way
    // out. The difference form `left + (right - left) * f` overflows f32 once
    // the neighbours are more than `f32::MAX` apart, and `inf * 0.0` is NaN —
    // so even the read at integer position 0 used to come back NaN.
    let extremes = vec![f32::MIN, f32::MAX, 0.0, f32::MAX, f32::MIN];
    for (num, den) in [(3, 2), (2, 3), (5, 4), (1, 2)] {
        let out = shift_pitch(&extremes, SR, Ratio::new(num, den).expect("ratio")).expect("shift");
        assert!(
            out.iter().all(|sample| sample.is_finite()),
            "{num}:{den} turned a finite input into {out:?}"
        );
    }
}

#[test]
fn ac4_a_near_unison_ratio_with_huge_terms_is_audio_not_an_error() {
    // `u64::MAX : u64::MAX - 1` is a hair above unison, so the honest answer is
    // two samples. It must not be refused as though it were extreme just
    // because `len · den` overflows an intermediate.
    let two = [0.25f32, 0.75];
    let hair = Ratio::new(u64::MAX, u64::MAX - 1).expect("ratio");
    assert_eq!(shift_pitch(&two, SR, hair).map(|out| out.len()), Ok(2));
}

#[test]
fn ac4_an_absurd_sample_rate_cannot_widen_the_length_cap() {
    // The cap is a duration, and the caller picks the rate: without a ceiling
    // on the rate, ten minutes at `u32::MAX` Hz is ~10 TB, which the allocator
    // reserves lazily and then thrashes the machine filling.
    let one_sample = [0.5f32];
    assert_eq!(
        shift_pitch(
            &one_sample,
            u32::MAX,
            Ratio::new(1, 200_000_000).expect("ratio")
        ),
        Err(DspError::OutputTooLong)
    );
    // 192 kHz is the ceiling, so ten minutes there is still allowed.
    assert!(
        shift_pitch(
            &one_sample,
            u32::MAX,
            Ratio::new(1, 115_200_000).expect("ratio")
        )
        .is_ok()
    );
}
