//! R-0041 acceptance tests — a take says how fast it is and where it sits.
//!
//! Every signal here is synthesized with a *known* tempo and a *known* pitch,
//! so the tests check what was measured against what was put in, rather than
//! against what the code happens to compute.

use gooz_dsp::{
    Config, DspError, Follow, PitchFrame, PitchTrack, Transcription, follow_take, tempo_of,
};

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

// ---------------------------------------------------------------------------
// QA sign-off additions (R-0041, step 7).
//
// The tests above synthesize the easiest version of each signal: exact click
// trains, and a *pure* sine at 16 kHz. These use the shapes a real take has —
// a hand that drifts, a voice with harmonics and vibrato, a microphone's
// 44.1 or 48 kHz — because the gates were tuned against the easy shapes, and
// it is the real ones that decide whether `None` is reached for the right
// reasons.
// ---------------------------------------------------------------------------

/// Three seconds of one held note at `sr`: the fundamental — plus the 2nd and
/// 3rd harmonics at half and quarter level when `overtones` — with a 30 ms
/// attack, a 50 ms release, and a vibrato `cents` wide at 5.5 Hz. Padded with
/// a little room silence either side, as a take is.
fn held(sr: u32, hz: f64, cents: f64, overtones: bool) -> Vec<f32> {
    let rate = f64::from(sr);
    let secs = 3.0;
    let mut out = vec![0.0f32; (0.2 * rate) as usize];
    let mut phase = 0.0f64;
    for i in 0..(secs * rate) as usize {
        let t = i as f64 / rate;
        let wobble = cents / 1200.0 * (std::f64::consts::TAU * 5.5 * t).sin();
        phase += std::f64::consts::TAU * hz * wobble.exp2() / rate;
        let envelope = (t / 0.03).min(1.0) * ((secs - t) / 0.05).clamp(0.0, 1.0);
        let upper = if overtones {
            0.5 * (2.0 * phase).sin() + 0.25 * (3.0 * phase).sin()
        } else {
            0.0
        };
        out.push((0.4 * envelope * (phase.sin() + upper)) as f32);
    }
    out.extend(vec![0.0f32; (0.3 * rate) as usize]);
    out
}

/// A deterministic ±`amount` second timing drift, standing in for a hand.
fn drift(seed: u64, amount: f64) -> impl FnMut() -> f64 {
    let mut state = seed;
    move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        amount * (2.0 * ((state >> 11) as f64 / (1u64 << 53) as f64) - 1.0)
    }
}

#[test]
fn ac1_a_hand_played_pulse_that_drifts_20ms_still_reports_its_tempo() {
    // Nobody plays to a click. ±20 ms is an ordinary human wobble, and a gate
    // that refused it would refuse exactly the takes this feature is for.
    for bpm in [92.0, 120.0, 140.0] {
        for seed in 0..4 {
            let mut wobble = drift(seed + 100, 0.020);
            let period = 60.0 / bpm;
            let times: Vec<f64> = (0..12)
                .map(|beat| 0.3 + f64::from(beat) * period + wobble())
                .collect();
            let take = plucks(&times, 220.0, 0.8 + 12.0 * period);
            let measured = follow(&take).bpm.unwrap_or_else(|| {
                panic!("a {bpm} BPM hand-played pulse (seed {seed}) was refused")
            });
            assert!(
                (measured - bpm).abs() < bpm * 0.06,
                "a {bpm} BPM hand-played pulse (seed {seed}) measured {measured:.1}"
            );
        }
    }
}

#[test]
fn ac1_a_long_breath_between_two_phrases_is_not_free_time() {
    // Two phrases at 120 BPM with five and a half seconds between them. The
    // pause is one outlying interval, not evidence against the pulse.
    let take = plucks(&[0.3, 0.8, 1.3, 1.8, 7.3, 7.8, 8.3, 8.8], 220.0, 9.5);
    let measured = follow(&take).bpm.expect("two phrases on one pulse");
    assert!((measured - 120.0).abs() < 7.2, "measured {measured:.1}");
}

#[test]
fn ac5_tempo_of_never_answers_with_a_non_finite_or_unmusical_tempo() {
    // `tempo_of` is public and takes any slice at any rate. The interval-based
    // estimator it replaced returned `inf` for a denormal interval and -60 for
    // onsets running backwards; this one reads loudness, so the hostile inputs
    // are signals and rates rather than onset lists.
    let mut garbage = clicks(120.0, 8);
    for (i, s) in garbage.iter_mut().enumerate().step_by(97) {
        *s = if i % 2 == 0 { f32::NAN } else { f32::INFINITY };
    }
    let takes: [Vec<f32>; 5] = [
        garbage,
        vec![f32::MAX; 8_000],
        vec![f32::MIN_POSITIVE; 8_000],
        vec![1.0e-38; 8_000],
        (0..8_000)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect(),
    ];
    for take in &takes {
        for rate in [1u32, 2, 100, 8_000, SR, 192_000, u32::MAX] {
            let bpm = tempo_of(take, rate);
            assert!(
                bpm.is_none_or(|b| b.is_finite() && (60.0..=180.0).contains(&b)),
                "rate {rate} gave {bpm:?}"
            );
        }
    }
}

/// An analysis that heard nothing but `count` frames voiced at 220 Hz, `step`
/// seconds apart.
fn voiced_frames(count: usize, step: f64) -> Transcription {
    Transcription {
        pitch_track: PitchTrack {
            frames: (0..count)
                .map(|i| PitchFrame {
                    time_secs: i as f64 * step,
                    f0_hz: Some(220.0),
                    confidence: 0.9,
                })
                .collect(),
        },
        onsets: Vec::new(),
        notes: Vec::new(),
    }
}

#[test]
fn ac5_a_non_finite_pitch_frame_cannot_become_the_root() {
    // `follow` takes any `Transcription`, and its fields are public. The unit
    // test of the same name in follow.rs mixes one NaN frame in four, which
    // sorts above every 220 under `total_cmp` and so never reaches the median:
    // it passed with the finiteness filter deleted (mutation-tested on
    // 39a9257). Here the non-finite frames are the majority.
    let mut heard = voiced_frames(30, 0.01);
    heard
        .pitch_track
        .frames
        .extend((30..70).map(|i| PitchFrame {
            time_secs: i as f64 * 0.01,
            f0_hz: Some(f32::NAN),
            confidence: 0.9,
        }));
    assert_eq!(gooz_dsp::follow(&[], SR, &heard).root_hz, Some(220.0));

    // Nor do frames with no finite time span a phrase (removing that guard
    // survived too).
    for when in [f64::NAN, f64::INFINITY] {
        let mut heard = voiced_frames(40, 0.01);
        for frame in &mut heard.pitch_track.frames {
            frame.time_secs = when;
        }
        assert_eq!(
            gooz_dsp::follow(&[], SR, &heard).root_hz,
            None,
            "frames at t = {when}"
        );
    }
}

#[test]
fn ac3_too_little_singing_is_not_a_key() {
    // Sung voicing is read in runs, and the runs must add up to a quarter of a
    // second. Both edges, so moving the threshold either way fails.
    assert_eq!(
        gooz_dsp::follow(&[], SR, &voiced_frames(24, 0.01)).root_hz,
        None
    );
    assert_eq!(
        gooz_dsp::follow(&[], SR, &voiced_frames(26, 0.01)).root_hz,
        Some(220.0)
    );
}

#[test]
fn ac3_a_run_too_short_to_be_a_note_is_not_counted() {
    // Plenty of voicing in total, but in 50 ms runs — breath, consonants,
    // stray frames. None of them is long enough to be a sung note.
    let mut frames = Vec::new();
    for run in 0..20 {
        for i in 0..10 {
            let t = run as f64 * 0.2 + i as f64 * 0.01;
            frames.push(PitchFrame {
                time_secs: t,
                f0_hz: (i < 5).then_some(220.0),
                confidence: 0.9,
            });
        }
    }
    let heard = Transcription {
        pitch_track: PitchTrack { frames },
        onsets: Vec::new(),
        notes: Vec::new(),
    };
    assert_eq!(gooz_dsp::follow(&[], SR, &heard).root_hz, None);
}

/// Adds a clean sung note — a sine with a 20 ms attack and a 30 ms release —
/// to `take` at `start` seconds.
fn sing(take: &mut [f32], sr: u32, start: f64, secs: f64, hz: f64) {
    let rate = f64::from(sr);
    let first = (start * rate) as usize;
    for (i, sample) in take
        .iter_mut()
        .skip(first)
        .take((secs * rate) as usize)
        .enumerate()
    {
        let t = i as f64 / rate;
        let envelope = (t / 0.02).min(1.0) * ((secs - t) / 0.03).clamp(0.0, 1.0);
        *sample += (0.5 * envelope * (std::f64::consts::TAU * hz * t).sin()) as f32;
    }
}

#[test]
fn ac2_a_pitched_take_with_gaps_inside_it_still_has_a_key() {
    // MIN_VOICED_DENSITY is measured inside the voiced span so that silence
    // *around* a phrase stops costing the take its key. Silence *inside* the
    // span still does, and every note below is transcribed at its pitch.
    // Measured on 39a9257 at 48 kHz, both root_hz: None:
    //   eight clean notes at 120 BPM, each 40% of the beat (detached, not a
    //   scattering: `analyze` returns all eight at their pitch);
    //   0.8 s of singing, a 2 s breath, 0.7 s more — 3.5 s, the length of the
    //   studio's take.
    let sr = 48_000;
    let melody = [260.0, 292.0, 327.0, 260.0];

    let mut detached = vec![0.0f32; (4.8 * f64::from(sr)) as usize];
    for beat in 0..8 {
        sing(
            &mut detached,
            sr,
            0.3 + f64::from(beat) * 0.5,
            0.2,
            melody[beat as usize % 4],
        );
    }

    let mut two_phrases = vec![0.0f32; (4.0 * f64::from(sr)) as usize];
    sing(&mut two_phrases, sr, 0.2, 0.4, melody[0]);
    sing(&mut two_phrases, sr, 0.6, 0.4, melody[1]);
    sing(&mut two_phrases, sr, 3.0, 0.35, melody[2]);
    sing(&mut two_phrases, sr, 3.35, 0.35, melody[3]);

    for (shape, take) in [("detached notes", detached), ("two phrases", two_phrases)] {
        let root = follow_take(&take, sr, &Config::default())
            .expect("a valid take")
            .root_hz
            .unwrap_or_else(|| panic!("{shape}: a clearly pitched take reported no key"));
        assert!(
            (250.0..=340.0).contains(&root),
            "{shape}: {root:.1} Hz is outside the notes that were sung"
        );
    }
}

#[test]
fn ac3_a_held_note_sung_with_vibrato_has_no_tempo() {
    // One note held for three seconds, with the vibrato any singer puts on a
    // held note. The onset detector fires on every half-cycle of the wobble,
    // and a wobble is perfectly regular, so no interval-spread gate can refuse
    // it. Measured on 39a9257: Some(165.4) at 48 kHz, which the studio lays
    // out as a 165 BPM riff of 33 notes.
    let take = held(48_000, 220.0, 30.0, true);
    let heard = follow_take(&take, 48_000, &Config::default()).expect("a valid take");
    assert!(heard.root_hz.is_some(), "a held note has a pitch");
    assert_eq!(heard.bpm, None, "one held note reported a tempo");
}

#[test]
fn ac3_a_held_hum_at_a_microphones_sample_rate_has_no_tempo() {
    // No vibrato at all: a steady hum at the rates a microphone delivers.
    // `ac3_the_two_answers_are_independent` passes because its held note is a
    // pure sine at 16 kHz, the one configuration that does not trip the onset
    // detector. Measured on 39a9257:
    //   48 kHz,   220.0 Hz with overtones -> Some(122.3), 23 onsets
    //   48 kHz,   261.6 Hz with overtones -> Some(130.8), 14 onsets
    //   44.1 kHz, 147.0 Hz pure           -> Some(152.0), 28 onsets
    for (sr, hz, overtones) in [
        (48_000, 220.0, true),
        (48_000, 261.6, true),
        (44_100, 147.0, false),
    ] {
        let take = held(sr, hz, 0.0, overtones);
        let heard = follow_take(&take, sr, &Config::default()).expect("a valid take");
        assert_eq!(
            heard.bpm, None,
            "a held {hz} Hz hum at {sr} Hz reported a tempo"
        );
    }
}

#[test]
fn ac3_three_notes_in_free_time_are_not_a_pulse() {
    // With exactly MIN_ONSETS onsets there are two intervals, and the lower
    // middle of two deviations from their own lower middle is always zero, so
    // MAX_IOI_SPREAD cannot fire. Two notes 9 s apart are refused; add a third
    // anywhere and they report a tempo again. Measured on 39a9257:
    //   estimate_bpm([0.0, 0.5, 9.0]) -> Some(120.0)
    //   estimate_bpm([0.0, 2.8, 9.0]) -> Some(21.4), which `follow` folds to 85.7
    //   plucks at 0.2 s, 3.0 s, 9.2 s -> Some(85.7) BPM
    for times in [[0.2, 0.7, 9.2], [0.2, 3.0, 9.2]] {
        let scattered = plucks(&times, 220.0, 10.0);
        assert_eq!(
            follow(&scattered).bpm,
            None,
            "three notes at {times:?} reported a tempo"
        );
    }
    let scattered = plucks(&[0.2, 3.0, 9.2], 220.0, 10.0);
    assert_eq!(
        follow(&scattered).bpm,
        None,
        "three scattered notes reported a tempo"
    );
}
