//! R-0042 acceptance tests — sing, pick a style, hear yourself over a track in
//! that style: at your tempo, entering on a downbeat.
//!
//! The takes are shaped like real ones — room noise, a breath before the first
//! note — and every alignment is measured against where the fixture *put* the
//! note, never re-detected with the detector under test.

use gooz_dsp::{DspError, MAX_BPM, MIN_BPM};
use gooz_studio::{Accompaniment, Part, accompany_take, build_song};

const SR: u32 = 48_000;
const TENSE: u8 = 30;

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

/// A take: `lead` seconds of room, then `beats` sung notes at `bpm` and `hz`,
/// each decaying toward the next. Optionally a −40 dBFS noise floor and a breath
/// just before the first note — the two things that fooled an onset anchor.
struct Take {
    bpm: f64,
    hz: f64,
    beats: usize,
    lead: f64,
    noisy: bool,
    breath: bool,
    rate: u32,
}

impl Take {
    fn at(bpm: f64, hz: f64) -> Take {
        Take {
            bpm,
            hz,
            beats: 12,
            lead: 0.7,
            noisy: false,
            breath: false,
            rate: SR,
        }
    }

    fn render(&self) -> Vec<f32> {
        let rate = f64::from(self.rate);
        let beat = 60.0 / self.bpm;
        let len = ((self.lead + self.beats as f64 * beat + 0.3) * rate) as usize;
        let mut out = if self.noisy {
            noise(len, db(-40.0), 7)
        } else {
            vec![0.0; len]
        };
        if self.breath {
            let breath = noise(len, db(-26.0), 11);
            let (from, to) = ((0.40 * rate) as usize, (0.65 * rate) as usize);
            for i in from..to {
                out[i] += breath[i];
            }
        }
        for n in 0..self.beats {
            let start = ((self.lead + n as f64 * beat) * rate) as usize;
            for i in 0..(beat * rate) as usize {
                let t = i as f64 / rate;
                let decay = (-7.0 * t / beat).exp();
                let tone = (std::f64::consts::TAU * self.hz * t).sin();
                if start + i < out.len() {
                    out[start + i] += (0.6 * decay * tone) as f32;
                }
            }
        }
        out
    }
}

fn accompany(take: &Take, style: &str) -> Accompaniment {
    accompany_take(&take.render(), take.rate, style, TENSE)
        .unwrap_or_else(|e| panic!("a sung take styled {style}: {e}"))
}

/// One bar of the result, in samples: both stems are whole bars of 4/4.
fn bar_samples(song: &Accompaniment) -> usize {
    song.voice.samples.len() / song.voice.bars as usize
}

fn laid_out_bpm(song: &Accompaniment) -> f64 {
    240.0 * f64::from(song.voice.sample_rate) / bar_samples(song) as f64
}

#[test]
fn ac1_the_track_is_at_the_tempo_the_singer_sang() {
    // 126 is no style's tempo (105, 140, 160, 92), so a track that fell back to
    // the style, or read a number from anywhere else, cannot pass by accident.
    let song = accompany(&Take::at(126.0, 260.0), "trap");
    let followed = song.voice.followed_bpm.expect("a sung pulse is followed");
    assert!((followed - 126.0).abs() < 4.0, "followed {followed:.1}");
    assert!(
        (laid_out_bpm(&song) - followed).abs() < 0.05,
        "laid out at {:.2}, but the voice was followed at {followed:.2}",
        laid_out_bpm(&song)
    );
    assert_eq!(song.voice.bpm, song.plan.tempo_bpm);
    assert!((song.plan.tempo_bpm - followed).abs() < 1e-9);
    assert!(
        (song.plan.tempo_bpm - 140.0).abs() > 5.0,
        "trap's own tempo won"
    );
}

#[test]
fn ac2_the_first_sung_note_enters_on_the_downbeat_of_bar_two() {
    // A breath at 0.40–0.65 s and a −40 dBFS room, then the first note at
    // exactly 0.70 s. Anchoring on the first *onset* put this at t = 0 (the
    // noise) or 0.39 s (the breath); the note is where the fixture put it. And
    // at every rate a microphone delivers: the onset detector runs a frame
    // early — 12 ms at 48 kHz, 44 ms at a headset's 16 kHz — and 44 ms behind
    // the kick is audible.
    for rate in [48_000, 44_100, 16_000] {
        let take = Take {
            noisy: true,
            breath: true,
            rate,
            ..Take::at(118.0, 260.0)
        };
        let song = accompany(&take, "corrido");
        let pad = song
            .voice
            .samples
            .iter()
            .position(|s| *s != 0.0)
            .expect("the voice has sound in it");
        let note_lands = pad + (0.70 * f64::from(rate)) as usize;
        let downbeat = bar_samples(&song);
        let error_ms = (note_lands as f64 - downbeat as f64) / f64::from(rate) * 1000.0;
        assert!(
            error_ms.abs() < 3.0,
            "at {rate} Hz the first note lands {error_ms:.1} ms from the downbeat of bar 2"
        );
    }
}

#[test]
fn ac2_a_take_that_starts_singing_at_once_still_gets_a_bar_of_drums() {
    let take = Take {
        lead: 0.0,
        ..Take::at(118.0, 260.0)
    };
    let song = accompany(&take, "trap");
    let pad = song
        .voice
        .samples
        .iter()
        .position(|s| *s != 0.0)
        .expect("sound");
    let downbeat = bar_samples(&song);
    assert!(
        pad.abs_diff(downbeat) <= 480,
        "no count-in: the voice starts {pad} samples in, a bar is {downbeat}"
    );
}

#[test]
fn ac3_the_style_is_the_presets_not_a_generic_beat() {
    let take = Take::at(118.0, 260.0);
    let corrido = accompany(&take, "corrido");
    let metal = accompany(&take, "metal");
    assert_eq!(corrido.plan.preset, "corrido");
    assert_eq!(metal.plan.preset, "metal");
    // Where each preset puts its snare, in 4/4: corrido on beat 3, metal on 2.
    let snare_beat = |song: &Accompaniment| {
        let snare = song
            .plan
            .voices
            .iter()
            .find(|v| format!("{:?}", v.role) == "Snare")
            .expect("a snare lane");
        snare.rotate as f64 / (snare.steps as f64 / 4.0) + 1.0
    };
    assert_eq!(snare_beat(&corrido), 3.0);
    assert_eq!(snare_beat(&metal), 2.0);
    assert_ne!(
        corrido.track.samples, metal.track.samples,
        "the styles sound the same"
    );
}

#[test]
fn ac4_a_take_with_no_pulse_gets_the_styles_own_tempo_and_says_so() {
    // One held note: a pitch, no pulse.
    let held: Vec<f32> = (0..3 * SR as usize)
        .map(|i| (0.6 * (std::f64::consts::TAU * 260.0 * i as f64 / f64::from(SR)).sin()) as f32)
        .collect();
    let song = accompany_take(&held, SR, "metal", TENSE).expect("a held note is a take");
    assert_eq!(
        song.voice.followed_bpm, None,
        "a held note was given a pulse"
    );
    assert_eq!(song.plan.tempo_bpm, 160.0, "metal's own tempo");
    assert!((laid_out_bpm(&song) - 160.0).abs() < 0.05);
}

#[test]
fn ac5_the_key_is_carried_and_saved() {
    // 260 Hz — not a 220-family pitch, so a grid that did not follow cannot
    // reproduce it.
    let song = accompany(&Take::at(118.0, 260.0), "trap");
    let root = song.voice.followed_root_hz.expect("a sung take has a root");
    assert!(
        (1200.0 * (root / 260.0).log2()).abs() < 40.0,
        "root {root:.1}"
    );

    let session = build_song("s", TENSE, 55, Some(&song.voice), Some(&song.track));
    assert_eq!(
        session.settings.root_hz, root,
        "the saved key is not the take's"
    );
    assert_eq!(
        session.settings.bpm, song.voice.bpm,
        "the saved tempo is not the clock"
    );
}

#[test]
fn ac5_the_key_does_not_touch_the_drums() {
    // Two held notes of the same length at different pitches. Neither has a
    // pulse, so both are laid out at the style's own tempo — the *same* clock,
    // exactly. (Two sung takes at one tempo but different pitches follow to
    // 118.0213 and 118.0203 BPM; their drums would differ for that reason
    // alone and prove nothing about the key.)
    let held = |hz: f64| -> Vec<f32> {
        (0..3 * SR as usize)
            .map(|i| (0.6 * (std::f64::consts::TAU * hz * i as f64 / f64::from(SR)).sin()) as f32)
            .collect()
    };
    let low = accompany_take(&held(260.0), SR, "trap", TENSE).expect("held note");
    let high = accompany_take(&held(311.0), SR, "trap", TENSE).expect("held note");
    assert_ne!(
        low.voice.followed_root_hz, high.voice.followed_root_hz,
        "the keys differ"
    );
    assert_eq!(low.plan.tempo_bpm, high.plan.tempo_bpm, "the clocks differ");
    assert_eq!(
        low.track.samples, high.track.samples,
        "the key changed the drums"
    );
}

#[test]
fn ac6_voice_and_track_are_one_mix_at_any_rate() {
    for rate in [48_000, 44_100] {
        let take = Take {
            rate,
            ..Take::at(118.0, 260.0)
        };
        let song = accompany(&take, "corrido");
        assert_eq!(song.voice.sample_rate, rate);
        assert_eq!(
            song.track.sample_rate, rate,
            "the track is not at the take's rate"
        );
        assert_eq!(song.voice.samples.len(), song.track.samples.len());
        assert_eq!(song.voice.samples.len() % song.voice.bars as usize, 0);
        assert_eq!(song.voice.bars, song.track.bars);

        let session = build_song("s", TENSE, 55, Some(&song.voice), Some(&song.track));
        let mix = session.mixdown().expect("one rate, one length: it mixes");
        assert!(!mix.samples.is_empty());
        assert!(
            session.stems.iter().any(|stem| stem.name == "voice"),
            "the voice was saved as {:?}",
            session.stems.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }
}

#[test]
fn ac6_the_voice_is_at_a_level_that_sits_with_the_drums() {
    // A laptop take peaking at −20 dBFS used to sit ~19 dB under the drums.
    let quiet: Vec<f32> = Take::at(118.0, 260.0)
        .render()
        .iter()
        .map(|s| s * 0.1)
        .collect();
    let song = accompany_take(&quiet, SR, "trap", TENSE).expect("a quiet take");
    let peak = song
        .voice
        .samples
        .iter()
        .fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(
        (peak - db(-1.0)).abs() < 0.01,
        "the voice peaks at {peak:.3}"
    );
}

#[test]
fn ac7_a_failed_take_is_a_typed_error() {
    let cases: [(&str, Vec<f32>, u32, DspError); 5] = [
        ("empty", Vec::new(), SR, DspError::EmptySignal),
        (
            "zero rate",
            Take::at(118.0, 260.0).render(),
            0,
            DspError::InvalidSampleRate,
        ),
        ("silence", vec![0.0; 2 * SR as usize], SR, DspError::Silent),
        (
            "room noise alone",
            noise(2 * SR as usize, db(-40.0), 3),
            SR,
            DspError::Silent,
        ),
        (
            "too hot",
            vec![0.1, 1.5, 0.1],
            SR,
            DspError::SampleOutOfRange,
        ),
    ];
    for (what, take, rate, expected) in cases {
        assert_eq!(
            accompany_take(&take, rate, "trap", TENSE).unwrap_err(),
            expected,
            "{what}"
        );
    }
    let mut corrupt = Take::at(118.0, 260.0).render();
    corrupt[100] = f32::NAN;
    assert_eq!(
        accompany_take(&corrupt, SR, "trap", TENSE).unwrap_err(),
        DspError::NonFiniteSample
    );
}

#[test]
fn ac7_deterministic_and_bounded() {
    let take = Take {
        noisy: true,
        ..Take::at(118.0, 260.0)
    }
    .render();
    let first = accompany_take(&take, SR, "metal", TENSE).expect("take");
    let second = accompany_take(&take, SR, "metal", TENSE).expect("take");
    assert_eq!(first.voice.samples, second.voice.samples);
    assert_eq!(first.track.samples, second.track.samples);
    for s in first.voice.samples.iter().chain(&first.track.samples) {
        assert!(s.is_finite() && s.abs() <= 1.0 + 1e-6);
    }
    assert!((MIN_BPM..=MAX_BPM).contains(&first.plan.tempo_bpm));
}

#[test]
fn the_voice_says_what_it_is() {
    let song = accompany(&Take::at(118.0, 260.0), "trap");
    assert_eq!(song.voice.part, Part::Voice);
}
