//! R-0040 acceptance tests — a recording becomes an instrument you can hear.
//!
//! Deviceless: the "recordings" are synthesized here, so these run in CI with
//! no microphone. They are shaped like what the app actually records — a
//! 3.5 s capture window with silence (or room noise) before the sound — because
//! an earlier suite only ever used takes that started at sample 0, which the
//! app never produces, and passed while every real take landed off the beat.

use gooz_dsp::{Config, DspError, pitch_track};
use gooz_studio::{RiffView, instrument_from_take};

const SR: u32 = 48_000;
const TENSE: u8 = 30;
/// What the shell records: a fixed capture window.
const WINDOW_SECS: f64 = 3.5;

fn db(level: f32) -> f32 {
    10f32.powf(level / 20.0)
}

fn noise(len: usize, level: f32, seed: u64) -> Vec<f32> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            ((state >> 40) as f32 / 8_388_608.0 - 1.0) * level
        })
        .collect()
}

/// A knock on the table: a short noise burst with no pitch in it.
fn knock() -> Vec<f32> {
    noise(SR as usize / 10, 0.8, 0x9E37_79B9)
        .into_iter()
        .enumerate()
        .map(|(i, s)| s * (-(i as f32) / 400.0).exp())
        .collect()
}

/// A plucked note at `hz`: a pitch to measure, and short enough to fit a beat.
fn pluck(hz: f64) -> Vec<f32> {
    (0..(0.3 * f64::from(SR)) as usize)
        .map(|i| {
            let t = i as f64 / f64::from(SR);
            (0.7 * (-6.0 * t).exp() * (std::f64::consts::TAU * hz * t).sin()) as f32
        })
        .collect()
}

/// A capture window: `lead_secs` of silence (or noise), then `sound`.
fn window(lead_secs: f64, sound: &[f32], noise_db: Option<f32>) -> Vec<f32> {
    let len = (WINDOW_SECS * f64::from(SR)) as usize;
    let mut take = match noise_db {
        Some(level) => noise(len, db(level), 0x1234_5678),
        None => vec![0.0; len],
    };
    let at = (lead_secs * f64::from(SR)) as usize;
    for (i, s) in sound.iter().enumerate() {
        if at + i < len {
            take[at + i] += s;
        }
    }
    take
}

/// One beat, read off the view itself rather than hard-coded: the loop is a
/// whole number of bars, and a bar is four beats.
fn beat_samples(view: &RiffView) -> usize {
    view.samples.len() / (view.bars as usize * 4)
}

/// Where the hit that belongs to beat `i` actually starts, in beats.
fn hit_position(view: &RiffView, i: usize) -> Option<f64> {
    let beat = beat_samples(view);
    let from = i * beat;
    let window = view
        .samples
        .get(from..(from + beat).min(view.samples.len()))?;
    let first = window.iter().position(|s| s.abs() > 0.02)?;
    Some((from + first) as f64 / beat as f64)
}

#[test]
fn ac1_every_hit_lands_on_its_beat_whatever_the_lead_in() {
    // The regression this suite exists for: a knock 0.4 s into the window used
    // to land at beats 0.61, 1.49, 2.41, 3.35.
    for (lead, floor) in [
        (0.0, None),
        (0.4, None),
        (0.8, None),
        (2.0, None),
        (0.7, Some(-40.0)),
    ] {
        let view = instrument_from_take(&window(lead, &knock(), floor), SR, TENSE)
            .unwrap_or_else(|e| panic!("lead {lead}: {e}"));
        for i in 0..view.notes.len() {
            let at =
                hit_position(&view, i).unwrap_or_else(|| panic!("lead {lead}: beat {i} is silent"));
            assert!(
                (at - i as f64).abs() < 0.02,
                "lead {lead}, floor {floor:?}: the hit for beat {i} landed at beat {at:.2}"
            );
        }
    }
}

#[test]
fn ac1_each_hit_sounds_at_its_own_ratio() {
    // A pluck at 220 Hz, placed after a lead-in: hit `i` must *measure* at
    // 220 · rᵢ. The earlier suite passed with every hit rendered at 1:1.
    let view = instrument_from_take(&window(0.5, &pluck(220.0), None), SR, TENSE)
        .expect("a pluck is a valid take");
    let beat = beat_samples(&view);
    for (i, card) in view.notes.iter().enumerate() {
        let hit = &view.samples[i * beat..(i + 1) * beat];
        let track = pitch_track(hit, SR, &Config::default()).expect("a hit to measure");
        let mut heard: Vec<f64> = track
            .frames
            .iter()
            .filter_map(|f| f.f0_hz)
            .map(f64::from)
            .collect();
        assert!(
            !heard.is_empty(),
            "no pitch in the {}:{} hit",
            card.num,
            card.den
        );
        heard.sort_by(f64::total_cmp);
        let measured = heard[heard.len() / 2];
        let expected = 220.0 * card.num as f64 / card.den as f64;
        let cents = 1200.0 * (measured / expected).log2().abs();
        assert!(
            cents < 25.0,
            "the {}:{} hit measured {measured:.1} Hz, expected {expected:.1}",
            card.num,
            card.den
        );
    }
}

#[test]
fn ac2_a_knock_is_as_playable_as_a_hum() {
    let knocked = instrument_from_take(&window(0.3, &knock(), None), SR, TENSE).expect("a knock");
    let plucked =
        instrument_from_take(&window(0.3, &pluck(220.0), None), SR, TENSE).expect("a pluck");
    assert_eq!(
        knocked.notes.len(),
        plucked.notes.len(),
        "the scale does not depend on what was recorded"
    );
}

#[test]
fn ac2_the_unison_hit_is_the_sound_itself_not_a_saturated_copy() {
    // With the bypass curve, the first hit is the recorded sound times one gain.
    // A saturating curve bends that ratio, so a loud steady sound shows it.
    let steady: Vec<f32> = (0..(0.4 * f64::from(SR)) as usize)
        .map(|i| (0.9 * (std::f64::consts::TAU * 330.0 * i as f64 / f64::from(SR)).sin()) as f32)
        .collect();
    let view = instrument_from_take(&window(0.0, &steady, None), SR, TENSE).expect("a tone");
    // Well inside the sound: clear of both fades.
    let (from, to) = (4_800, 9_600);
    let ratios: Vec<f32> = (from..to)
        .filter(|&i| steady[i].abs() > 0.3)
        .map(|i| view.samples[i] / steady[i])
        .collect();
    let first = ratios[0];
    assert!(
        ratios.iter().all(|r| (r - first).abs() < 1e-3),
        "the unison hit is not proportional to the sound — it was distorted"
    );
}

#[test]
fn ac4_the_slider_changes_the_scale_you_hear() {
    let smooth = instrument_from_take(&window(0.3, &knock(), None), SR, 0).expect("take");
    let tense = instrument_from_take(&window(0.3, &knock(), None), SR, 100).expect("take");
    let degrees =
        |v: &RiffView| -> Vec<(u64, u64)> { v.notes.iter().map(|n| (n.num, n.den)).collect() };
    let (smooth_set, tense_set) = (degrees(&smooth), degrees(&tense));
    assert!(tense_set.len() > smooth_set.len(), "the slider did nothing");
    // The degrees tensing adds are the more complex ratios.
    let height = |(n, d): (u64, u64)| (n * d) as f64;
    let smoothest_max = smooth_set.iter().map(|&d| height(d)).fold(0.0, f64::max);
    for &degree in tense_set.iter().filter(|d| !smooth_set.contains(d)) {
        assert!(
            height(degree) > smoothest_max,
            "tensing added {degree:?}, which is not more complex than the smooth set"
        );
    }
    // And you hear each of them: one hit per card.
    for i in 0..tense.notes.len() {
        assert!(
            hit_position(&tense, i).is_some(),
            "beat {i} is on a card but silent"
        );
    }
}

#[test]
fn ac5_a_failed_recording_is_a_typed_error() {
    let silent = vec![0.0f32; (WINDOW_SECS * f64::from(SR)) as usize];
    let hiss = noise(silent.len(), db(-60.0), 42);
    let cases: [(&str, Vec<f32>, u32, DspError); 5] = [
        ("empty", Vec::new(), SR, DspError::EmptySignal),
        (
            "zero rate",
            window(0.3, &knock(), None),
            0,
            DspError::InvalidSampleRate,
        ),
        ("a muted mic", silent, SR, DspError::Silent),
        ("room hiss alone", hiss, SR, DspError::Silent),
        (
            "too hot",
            vec![0.2, 1.5, 0.2],
            SR,
            DspError::SampleOutOfRange,
        ),
    ];
    for (what, take, rate, expected) in cases {
        assert_eq!(
            instrument_from_take(&take, rate, TENSE).unwrap_err(),
            expected,
            "{what}"
        );
    }
    // A corrupt sample is reported even when it sits in silence that gets cut.
    let mut corrupt = window(1.0, &knock(), None);
    corrupt[100] = f32::NAN;
    assert_eq!(
        instrument_from_take(&corrupt, SR, TENSE).unwrap_err(),
        DspError::NonFiniteSample
    );
}

#[test]
fn ac6_the_same_take_always_sounds_the_same_and_stays_bounded() {
    let take = window(0.6, &knock(), Some(-45.0));
    let first = instrument_from_take(&take, SR, TENSE).expect("take");
    assert_eq!(first, instrument_from_take(&take, SR, TENSE).expect("take"));
    assert!(
        first
            .samples
            .iter()
            .all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-6)
    );
}

#[test]
fn ac7_the_loop_is_a_whole_number_of_bars() {
    // Unpadded, the figure looped against the beat and drifted from the first
    // repeat, and the export replayed its start mid-bar.
    let view = instrument_from_take(&window(0.3, &knock(), None), SR, TENSE).expect("take");
    assert!(view.bars >= 1);
    assert_eq!(view.samples.len() % view.bars as usize, 0);
    let bar_secs = view.seconds / f64::from(view.bars);
    assert!(
        (bar_secs - 4.0 * 60.0 / 92.0).abs() < 1e-3,
        "a bar is {bar_secs:.4} s — not a whole 4/4 bar at 92 BPM"
    );
}

#[test]
fn a_long_sound_climbs_the_scale_instead_of_stacking_into_a_chord() {
    // A three-second hum used to sound all its degrees at once. Each hit now
    // ends inside its own beat.
    let hum: Vec<f32> = (0..3 * SR as usize)
        .map(|i| (0.6 * (std::f64::consts::TAU * 150.0 * i as f64 / f64::from(SR)).sin()) as f32)
        .collect();
    let view = instrument_from_take(&window(0.2, &hum, None), SR, TENSE).expect("a hum");
    let beat = beat_samples(&view);
    for i in 1..view.notes.len() {
        let before_next = &view.samples[i * beat - 48..i * beat];
        let peak = before_next.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            peak < 0.05,
            "hit {} is still sounding when beat {i} starts ({peak:.3})",
            i - 1
        );
    }
}
