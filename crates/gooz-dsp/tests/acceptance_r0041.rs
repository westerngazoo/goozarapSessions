//! R-0041 acceptance tests — a take says how fast it is and where it sits.
//!
//! Every signal here is synthesized with a *known* tempo and a *known* pitch,
//! so the tests check what was measured against what was put in, rather than
//! against what the code happens to compute.

use gooz_dsp::{Config, DspError, Follow, follow_take};

/// 16 kHz, not 48: every signal here is synthesized, the analysis defaults
/// (2048-sample window, 80 Hz floor) are comfortable at this rate, and it cuts
/// the work — and the CI minutes — by three.
const SR: u32 = 16_000;

/// A click train at `bpm`: short bursts, so onsets are unambiguous.
fn clicks(bpm: f64, beats: usize) -> Vec<f32> {
    let period = (60.0 / bpm * f64::from(SR)) as usize;
    let burst = (0.012 * f64::from(SR)) as usize;
    let mut out = vec![0.0f32; period * beats + period];
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    for beat in 0..beats {
        for i in 0..burst {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let white = (state >> 40) as f32 / 8_388_608.0 - 1.0;
            let decay = (-30.0 * i as f32 / burst as f32).exp();
            out[beat * period + i] = white * decay * 0.9;
        }
    }
    out
}

/// A steady sine at `hz`, one second of it.
fn tone(hz: f64) -> Vec<f32> {
    (0..SR as usize)
        .map(|i| 0.8 * (std::f64::consts::TAU * hz * i as f64 / f64::from(SR)).sin() as f32)
        .collect()
}

/// Plucked tones at `bpm`, each at `hz` — something with both a pulse and a pitch.
fn sung(bpm: f64, hz: f64, beats: usize) -> Vec<f32> {
    let period = (60.0 / bpm * f64::from(SR)) as usize;
    let mut out = vec![0.0f32; period * beats];
    for beat in 0..beats {
        for i in 0..period {
            let t = (beat * period + i) as f64 / f64::from(SR);
            let env = (-4.0 * i as f64 / period as f64).exp();
            out[beat * period + i] = (0.8 * env * (std::f64::consts::TAU * hz * t).sin()) as f32;
        }
    }
    out
}

/// Plucked notes at the given times — onsets where we put them, nowhere else.
fn plucks(times: &[f64], hz: f64, total_secs: f64) -> Vec<f32> {
    let mut out = vec![0.0f32; (total_secs * f64::from(SR)) as usize];
    let len = (0.35 * f64::from(SR)) as usize;
    for &start_secs in times {
        let start = (start_secs * f64::from(SR)) as usize;
        for i in 0..len {
            if start + i >= out.len() {
                break;
            }
            let t = (start + i) as f64 / f64::from(SR);
            let env = (-6.0 * i as f64 / len as f64).exp();
            out[start + i] += (0.8 * env * (std::f64::consts::TAU * hz * t).sin()) as f32;
        }
    }
    out
}

fn follow(signal: &[f32]) -> Follow {
    follow_take(signal, SR, &Config::default()).expect("a valid take")
}

fn cents_apart(a: f64, b: f64) -> f64 {
    1200.0 * (a / b).log2().abs()
}

#[test]
fn ac1_a_steady_pulse_reports_its_tempo() {
    for bpm in [92.0, 120.0, 140.0] {
        let measured = follow(&clicks(bpm, 12))
            .bpm
            .expect("a click train has a pulse");
        assert!(
            (measured - bpm).abs() < bpm * 0.06,
            "a {bpm} BPM train measured {measured:.1}"
        );
    }
}

#[test]
fn ac2_a_steady_pitch_reports_its_root() {
    for hz in [110.0, 220.0, 330.0] {
        let measured = follow(&tone(hz)).root_hz.expect("a tone has a pitch");
        assert!(
            cents_apart(measured, hz) < 30.0,
            "a {hz} Hz tone measured {measured:.1} Hz"
        );
    }
}

#[test]
fn ac3_a_take_with_no_pulse_at_all_says_nothing_rather_than_guessing() {
    // Two notes nine seconds apart, and a hum in free time. Before the gates
    // these reported 103.2 and 117.8 BPM with full confidence — numbers that
    // would have been mixed into a session as though the singer had played
    // them.
    let two_apart = plucks(&[0.2, 5.5], 220.0, 7.0);
    let free_time = plucks(&[0.0, 1.211, 1.68, 2.699, 3.019, 4.838], 220.0, 6.0);
    assert_eq!(follow(&two_apart).bpm, None, "one gap is not a tempo");
    assert_eq!(follow(&free_time).bpm, None, "free time is not a tempo");
}

#[test]
fn ac2_a_phrase_with_silence_around_it_still_has_a_key() {
    // The most ordinary shape a real take has: a moment of room noise, the
    // phrase, then a moment more. An earlier gate measured voicing against the
    // whole take and rejected exactly this.
    let mut padded = vec![0.0f32; (3.0 * f64::from(SR)) as usize];
    padded.extend(tone(220.0));
    padded.extend(vec![0.0f32; (3.0 * f64::from(SR)) as usize]);
    let root = follow(&padded).root_hz.expect("the phrase has a pitch");
    assert!(cents_apart(root, 220.0) < 30.0, "measured {root:.1} Hz");
}

#[test]
fn ac3_the_two_answers_are_independent() {
    // A knock has a pulse and no pitch; one long note has a pitch and no pulse.
    // If these were one `Option<Follow>`, both halves would fail together and
    // nothing here would notice.
    let knocked = follow(&clicks(120.0, 12));
    assert!(knocked.bpm.is_some(), "a click train has a tempo");
    assert!(
        knocked.root_hz.is_none(),
        "a knock reported a key: {:?}",
        knocked.root_hz
    );

    let held = follow(&tone(220.0));
    assert!(held.root_hz.is_some(), "a held tone has a pitch");
    assert!(
        held.bpm.is_none(),
        "one note reported a tempo: {:?}",
        held.bpm
    );
}

#[test]
fn ac1_ac2_a_sung_take_says_both() {
    let heard = follow(&sung(120.0, 220.0, 10));
    let bpm = heard.bpm.expect("plucked tones have a pulse");
    let root = heard.root_hz.expect("plucked tones have a pitch");
    assert!((bpm - 120.0).abs() < 8.0, "measured {bpm:.1} BPM");
    assert!(cents_apart(root, 220.0) < 50.0, "measured {root:.1} Hz");
}

#[test]
fn ac4_a_tempo_outside_the_range_is_folded_by_octaves() {
    // 300 BPM is felt as 150; 40 is felt as 160 (×4). The reported number must
    // always be musical, never merely arithmetic.
    let fast = follow(&clicks(300.0, 24))
        .bpm
        .expect("fast train has a pulse");
    assert!(
        (60.0..=180.0).contains(&fast),
        "300 BPM reported as {fast:.1}, outside the musical range"
    );
    assert!(
        (fast - 150.0).abs() < 12.0,
        "300 BPM should fold to ~150, got {fast:.1}"
    );

    let slow = follow(&clicks(40.0, 8))
        .bpm
        .expect("slow train has a pulse");
    assert!(
        (60.0..=180.0).contains(&slow),
        "40 BPM reported as {slow:.1}, outside the musical range"
    );
}

#[test]
fn ac5_bad_input_is_a_typed_error_and_nothing_panics() {
    let cfg = Config::default();
    assert_eq!(follow_take(&[], SR, &cfg), Err(DspError::EmptySignal));
    assert_eq!(
        follow_take(&tone(220.0), 0, &cfg),
        Err(DspError::InvalidSampleRate)
    );
    let mut broken = tone(220.0);
    broken[99] = f32::NAN;
    assert_eq!(
        follow_take(&broken, SR, &cfg),
        Err(DspError::NonFiniteSample)
    );
}

#[test]
fn ac5_hostile_takes_never_panic() {
    let cfg = Config::default();
    let takes: Vec<Vec<f32>> = vec![
        vec![0.5],
        vec![0.5, -0.5],
        vec![f32::MIN, f32::MAX, 0.0],
        vec![0.0; 4_800],
        vec![1.0; 4_800],
        vec![f32::MIN_POSITIVE; 4_800],
    ];
    for take in &takes {
        for rate in [1u32, 2, 8_000, SR, u32::MAX] {
            // Not "it did not abort" — whatever it says must still be sayable.
            if let Ok(heard) = follow_take(take, rate, &cfg) {
                assert!(
                    heard.bpm.is_none_or(|bpm| (60.0..=180.0).contains(&bpm)),
                    "rate {rate} reported {:?} BPM",
                    heard.bpm
                );
                assert!(
                    heard.root_hz.is_none_or(|hz| hz.is_finite() && hz > 0.0),
                    "rate {rate} reported {:?} Hz",
                    heard.root_hz
                );
            }
        }
    }
}

#[test]
fn ac6_the_same_take_always_says_the_same_thing() {
    let take = sung(120.0, 220.0, 10);
    assert_eq!(follow(&take), follow(&take));
}

#[test]
fn a_reported_tempo_is_always_inside_the_musical_range() {
    // Two real signals, one either side of the range. The exhaustive sweep over
    // the folding itself is a unit test in `follow.rs` — putting it here meant
    // synthesizing 26 seconds of audio per case and a two-minute CI run.
    for bpm in [55.0, 240.0] {
        if let Some(measured) = follow(&clicks(bpm, 10)).bpm {
            assert!(
                (60.0..=180.0).contains(&measured),
                "a {bpm} BPM train reported {measured:.1}"
            );
        }
    }
}
