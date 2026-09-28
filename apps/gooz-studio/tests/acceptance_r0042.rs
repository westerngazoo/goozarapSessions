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

    let session = build_song("s", TENSE, 55, Some(&song.voice), Some(&song.track), None);
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

        let session = build_song("s", TENSE, 55, Some(&song.voice), Some(&song.track), None);
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

// ---------------------------------------------------------------------------
// QA sign-off additions (R-0042, step 7).
//
// The tests above sing with pure sines that start at full level, under uniform
// noise. The owner will sing into a laptop or a headset, so these sing with a
// voice-like tone (ten harmonics, vibrato, an onset ramp), under Gaussian room
// noise specified by its RMS, at 48, 44.1 and 16 kHz — and with what real takes
// start with: breath, a lip smack, consonants, clipping, a quiet singer.
//
// Every alignment is measured against where the fixture put the note, by
// aligning the voice to the take itself (the voice is the take delayed and
// scaled), never by re-detecting it. Tests that expose a defect are
// `#[ignore]`d with the measured output in their comments.
// ---------------------------------------------------------------------------
mod qa_signoff {
    use std::f64::consts::TAU;

    use gooz_dsp::{Config, DspError};
    use gooz_model::{VoiceRole, parse_intent, plan_sound};
    use gooz_ratio::Tempo;
    use gooz_session::StemKind;
    use gooz_studio::{
        Accompaniment, BeatConfig, BeatView, BeatVoiceSpec, DrumKind, Part, RiffView,
        accompany_take, build_beat, build_song, demo_riff, describe_song, export_master,
        instrument_from_take, riff_from_take, save_session, style_names,
    };

    const TENSE: u8 = 30;

    /// The styles and their own tempos, as the owner decided them (R-0042
    /// decision log, 2026-09-27) — pinned here, not read from the table.
    const STYLE_TEMPOS: [(&str, f64); 4] = [
        ("corrido", 105.0),
        ("trap", 140.0),
        ("metal", 160.0),
        ("free", 92.0),
    ];

    /// −1 dBFS: the level the voice is brought to.
    const MINUS_ONE_DBFS: f32 = 0.891_251;

    /// Where the first note of most fixtures starts, in seconds.
    const LEAD: f64 = 0.7;

    fn style_tempo(preset: &str) -> f64 {
        STYLE_TEMPOS
            .iter()
            .find(|(name, _)| *name == preset)
            .map(|(_, bpm)| *bpm)
            .unwrap_or_else(|| panic!("no such style: {preset}"))
    }

    /// One analysis hop at `rate`, in milliseconds — the spec's own tolerance
    /// for where the first note lands (SPEC-0042 §5, AC2).
    fn hop_ms(rate: u32) -> f64 {
        Config::default().hop as f64 / f64::from(rate) * 1000.0
    }

    fn linear(db: f64) -> f64 {
        10f64.powf(db / 20.0)
    }

    fn at(rate: u32, secs: f64) -> usize {
        (secs * f64::from(rate)).round() as usize
    }

    /// A small deterministic generator, so every fixture is reproducible.
    struct Rng(u64);

    impl Rng {
        fn uniform(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }

        fn gauss(&mut self) -> f64 {
            let u1 = self.uniform().max(1e-12);
            (-2.0 * u1.ln()).sqrt() * (TAU * self.uniform()).cos()
        }
    }

    /// A room: Gaussian noise at `rms_db` dBFS RMS, which is how a
    /// microphone's noise floor is specified. Its peaks run ~12 dB higher.
    fn room(len: usize, rms_db: f64, seed: u64) -> Vec<f32> {
        let mut rng = Rng(seed);
        (0..len)
            .map(|_| (rng.gauss() * linear(rms_db)) as f32)
            .collect()
    }

    /// Breath (dark) or an "s" (bright): shaped noise at RMS `rms`.
    fn hiss(len: usize, rms: f64, seed: u64, bright: bool) -> Vec<f32> {
        let mut rng = Rng(seed);
        let (mut low, mut last) = (0.0, 0.0);
        let raw: Vec<f64> = (0..len)
            .map(|_| {
                let x = rng.gauss();
                if bright {
                    let y = x - last;
                    last = x;
                    y
                } else {
                    low = 0.85 * low + 0.15 * x;
                    low
                }
            })
            .collect();
        let power = (raw.iter().map(|x| x * x).sum::<f64>() / len.max(1) as f64).sqrt();
        raw.iter().map(|x| (x / power * rms) as f32).collect()
    }

    /// A sung vowel at `hz`: ten harmonics falling off like a voice, vibrato
    /// from 150 ms, a linear onset of `attack` seconds and a 30 ms release,
    /// peaking at `level`.
    fn vowel(rate: u32, hz: f64, secs: f64, attack: f64, level: f64) -> Vec<f32> {
        let r = f64::from(rate);
        let mut phase = 0.0f64;
        let raw: Vec<f64> = (0..at(rate, secs))
            .map(|i| {
                let t = i as f64 / r;
                let cents = if t > 0.15 {
                    25.0 * (TAU * 5.5 * t).sin()
                } else {
                    0.0
                };
                phase += TAU * hz * 2f64.powf(cents / 1200.0) / r;
                let tone: f64 = (1..=10)
                    .filter(|h| hz * f64::from(*h) < r / 2.0 - 500.0)
                    .map(|h| (f64::from(h) * phase).sin() / f64::from(h).powf(1.3))
                    .sum();
                let onset = if attack > 0.0 {
                    (t / attack).min(1.0)
                } else {
                    1.0
                };
                tone * onset * ((secs - t) / 0.03).clamp(0.0, 1.0)
            })
            .collect();
        let peak = raw.iter().fold(0.0f64, |m, s| m.max(s.abs()));
        raw.iter().map(|s| (s / peak * level) as f32).collect()
    }

    fn mix_in(out: &mut Vec<f32>, sound: &[f32], from: usize) {
        if out.len() < from + sound.len() {
            out.resize(from + sound.len(), 0.0);
        }
        for (slot, s) in out[from..].iter_mut().zip(sound) {
            *slot += s;
        }
    }

    const MELODY: [f64; 8] = [260.0, 292.0, 327.0, 292.0, 260.0, 347.0, 327.0, 292.0];

    /// Vowels starting at `starts` (seconds), each `secs` long, in a take
    /// `total` seconds long.
    fn sing_at(rate: u32, starts: &[f64], secs: f64, attack: f64, total: f64) -> Vec<f32> {
        let mut out = vec![0.0f32; at(rate, total)];
        for (k, start) in starts.iter().enumerate() {
            let note = vowel(rate, MELODY[k % MELODY.len()], secs, attack, 0.6);
            mix_in(&mut out, &note, at(rate, *start));
        }
        out
    }

    /// `notes` detached vowels at `bpm`, the first `lead` seconds in.
    fn sing(rate: u32, bpm: f64, notes: usize, lead: f64, attack: f64) -> Vec<f32> {
        let beat = 60.0 / bpm;
        let starts: Vec<f64> = (0..notes).map(|k| lead + k as f64 * beat).collect();
        sing_at(
            rate,
            &starts,
            0.8 * beat,
            attack,
            lead + notes as f64 * beat + 0.4,
        )
    }

    fn in_room(take: Vec<f32>, rms_db: f64, seed: u64) -> Vec<f32> {
        let noise = room(take.len(), rms_db, seed);
        take.iter().zip(noise).map(|(s, n)| s + n).collect()
    }

    /// A real-shaped take, where its first sung note starts, and how long that
    /// note's onset ramp is.
    struct Scene {
        name: &'static str,
        take: Vec<f32>,
        first_note: usize,
        attack: f64,
        /// The tempo it was sung at, when it has an energy pulse to follow.
        sung_bpm: Option<f64>,
        style: &'static str,
    }

    fn scene(
        name: &'static str,
        take: Vec<f32>,
        rate: u32,
        sung_bpm: Option<f64>,
        style: &'static str,
    ) -> Scene {
        Scene {
            name,
            take,
            first_note: at(rate, LEAD),
            attack: 0.010,
            sung_bpm,
            style,
        }
    }

    fn scenes(rate: u32) -> Vec<Scene> {
        let r = f64::from(rate);
        let note = at(rate, LEAD);
        let mut out = vec![scene(
            "a firm onset in a laptop room (-50 dBFS RMS)",
            in_room(sing(rate, 118.0, 8, LEAD, 0.010), -50.0, 1),
            rate,
            Some(118.0),
            "corrido",
        )];

        let mut breath = sing(rate, 121.0, 8, LEAD, 0.010);
        let inhale = hiss(at(rate, 0.35), linear(-14.0), 7, false);
        mix_in(&mut breath, &inhale, note - at(rate, 0.43));
        out.push(scene(
            "a loud breath (-14 dBFS RMS) ending 80 ms before",
            in_room(breath, -50.0, 2),
            rate,
            Some(121.0),
            "trap",
        ));

        let mut smack = sing(rate, 118.0, 8, LEAD, 0.010);
        let mut rng = Rng(9);
        let click: Vec<f32> = (0..at(rate, 0.004))
            .map(|i| (rng.gauss() * linear(-8.0) * (-(i as f64) / (0.001 * r)).exp()) as f32)
            .collect();
        mix_in(&mut smack, &click, note - at(rate, 0.12));
        out.push(scene(
            "a lip smack (-8 dBFS) 120 ms before",
            in_room(smack, -50.0, 3),
            rate,
            Some(118.0),
            "metal",
        ));

        let clipped = in_room(sing(rate, 126.0, 8, LEAD, 0.010), -50.0, 4)
            .iter()
            .map(|s| (s * 4.0).clamp(-1.0, 1.0))
            .collect();
        out.push(scene(
            "a clipping take (x4, hard-clipped)",
            clipped,
            rate,
            Some(126.0),
            "trap",
        ));

        let quiet = sing(rate, 118.0, 8, LEAD, 0.010)
            .iter()
            .map(|s| s * (linear(-30.0) / 0.6) as f32)
            .collect();
        out.push(scene(
            "a quiet take (-30 dBFS peak) in a -55 dBFS room",
            in_room(quiet, -55.0, 5),
            rate,
            Some(118.0),
            "free",
        ));

        let beat = 60.0 / 104.0;
        let swung: Vec<f64> = (0..6)
            .flat_map(|k| {
                let on = LEAD + k as f64 * beat;
                [on, on + beat * 2.0 / 3.0]
            })
            .collect();
        let swing = sing_at(rate, &swung, 0.3 * beat, 0.008, LEAD + 6.0 * beat + 0.4);
        out.push(scene(
            "swung eighths (2:1) at 104",
            in_room(swing, -50.0, 6),
            rate,
            Some(104.0),
            "corrido",
        ));

        let beat = 60.0 / 97.0;
        let mut rng = Rng(11);
        let drifting: Vec<f64> = (0..8)
            .map(|k| {
                let wobble = if k == 0 {
                    0.0
                } else {
                    (rng.uniform() * 2.0 - 1.0) * 0.020
                };
                LEAD + k as f64 * beat + wobble
            })
            .collect();
        let drift = sing_at(rate, &drifting, 0.7 * beat, 0.010, LEAD + 8.0 * beat + 0.4);
        out.push(scene(
            "a pulse drifting +/-20 ms at 97",
            in_room(drift, -50.0, 7),
            rate,
            Some(97.0),
            "trap",
        ));

        let beat = 60.0 / 112.0;
        let paused: Vec<f64> = (0..3)
            .chain(7..10)
            .map(|k| LEAD + k as f64 * beat)
            .collect();
        let pause = sing_at(rate, &paused, 0.8 * beat, 0.010, LEAD + 10.0 * beat + 0.4);
        out.push(scene(
            "a long pause mid-phrase at 112",
            in_room(pause, -50.0, 8),
            rate,
            Some(112.0),
            "metal",
        ));

        let syllables = [0.14, 0.22, 0.11, 0.18, 0.31, 0.12, 0.16, 0.27];
        let mut speech = vec![0.0f32; at(rate, 3.0)];
        let mut start = LEAD;
        for (k, secs) in syllables.iter().enumerate() {
            let syllable = vowel(rate, 180.0 + 15.0 * (k % 3) as f64, *secs, 0.010, 0.5);
            mix_in(&mut speech, &syllable, at(rate, start));
            start += secs + 0.03;
        }
        out.push(scene(
            "pure speech rhythm",
            in_room(speech, -50.0, 9),
            rate,
            None,
            "free",
        ));

        let beat = 60.0 / 118.0;
        let end = at(rate, LEAD + 8.0 * beat);
        let mut legato = vec![0.0f32; end + at(rate, 0.4)];
        let mut phase = 0.0f64;
        for (i, slot) in legato[note..end].iter_mut().enumerate() {
            let t = i as f64 / r;
            phase += TAU * MELODY[(t / beat) as usize % MELODY.len()] / r;
            let tone: f64 = (1..=6)
                .map(|h| (f64::from(h) * phase).sin() / f64::from(h).powf(1.3))
                .sum();
            let edges = (t / 0.01).min(1.0) * ((end - note - i) as f64 / (0.03 * r)).min(1.0);
            *slot = (0.3 * tone * edges) as f32;
        }
        out.push(scene(
            "a legato phrase, no dip between notes",
            in_room(legato, -50.0, 10),
            rate,
            None,
            "metal",
        ));
        out
    }

    /// How far into the voice the take was delayed.
    ///
    /// The voice is the take delayed and scaled by one gain, so its loudest
    /// sample is the take's loudest, `pad` later. Checked at three more points,
    /// so a harness that mis-measures fails loudly instead of passing.
    /// Where the take sits in the voice: `voice[i + offset] == take[i] · gain`.
    ///
    /// Negative when whole bars of lead-in were dropped (owner decision,
    /// 2026-09-27: exactly one bar of drums before the singer). Checked at
    /// three points after the loudest sample — inside the singing, so inside
    /// what was kept — so the harness fails loudly rather than mis-measuring.
    fn offset_of(take: &[f32], song: &Accompaniment) -> i64 {
        let loudest = |xs: &[f32]| {
            xs.iter()
                .enumerate()
                .fold((0, 0.0f32), |(best, max), (i, x)| {
                    if x.abs() > max {
                        (i, x.abs())
                    } else {
                        (best, max)
                    }
                })
                .0
        };
        let voice = &song.voice.samples;
        let (t, v) = (loudest(take), loudest(voice));
        let offset = v as i64 - t as i64;
        let gain = voice[v] / take[t];
        let rest = take.len() - t;
        for k in [t, t + rest / 4, t + rest / 2] {
            let j = k as i64 + offset;
            assert!(
                (0..voice.len() as i64).contains(&j),
                "harness: take sample {k} is not in the voice"
            );
            assert!(
                (voice[j as usize] - take[k] * gain).abs() <= 1e-5,
                "harness: the voice is not the take offset by {offset}"
            );
        }
        offset
    }
    /// [`offset_of`] for a take kept whole: where its first sample sits.
    fn delay_of(take: &[f32], song: &Accompaniment) -> usize {
        let offset = offset_of(take, song);
        assert!(offset >= 0, "harness: expected the whole take to be kept");
        offset as usize
    }
    fn bar_len(song: &Accompaniment) -> usize {
        song.voice.samples.len() / song.voice.bars as usize
    }

    /// A bar of 4/4 at `bpm`, in samples, computed here rather than asked for.
    fn bar_at(bpm: f64, rate: u32) -> usize {
        (240.0 / bpm * f64::from(rate)).round() as usize
    }

    /// Where the note that starts at take sample `note` lands: its error from
    /// the downbeat it should land on, in ms, and the bar lines involved.
    struct Landing {
        error_ms: f64,
        nearest_bar: usize,
        want_bar: usize,
    }

    fn landing(take: &[f32], song: &Accompaniment, note: usize) -> Landing {
        let bar = bar_len(song);
        let landed = (note as i64 + offset_of(take, song)).max(0) as usize;
        // Exactly one bar of drums, then the first sung note: the downbeat of
        // bar 2, however long the singer waited (owner decision, 2026-09-27).
        let want_bar = 1;
        Landing {
            error_ms: (landed as f64 - (want_bar * bar) as f64) * 1000.0
                / f64::from(song.voice.sample_rate),
            nearest_bar: (landed + bar / 2) / bar,
            want_bar,
        }
    }

    /// The invariants that make the pair one mix (AC6), and AC7's bounds.
    fn assert_one_mix(song: &Accompaniment, rate: u32, what: &str) {
        let (voice, track) = (&song.voice, &song.track);
        assert_eq!(voice.sample_rate, rate, "{what}: the voice's rate");
        assert_eq!(track.sample_rate, rate, "{what}: the track's rate");
        assert_eq!(voice.samples.len(), track.samples.len(), "{what}: lengths");
        assert_eq!(voice.bars, track.bars, "{what}: bar counts");
        assert!(voice.bars >= 2, "{what}: no count-in bar");
        assert_eq!(
            voice.samples.len(),
            voice.bars as usize * bar_at(song.plan.tempo_bpm, rate),
            "{what}: not whole bars of the clock"
        );
        let secs = voice.samples.len() as f64 / f64::from(rate);
        assert!((voice.seconds - secs).abs() < 1e-9, "{what}: voice seconds");
        assert!((track.seconds - secs).abs() < 1e-9, "{what}: track seconds");
        assert_eq!(voice.part, Part::Voice, "{what}");
        assert_eq!(
            voice.bpm, song.plan.tempo_bpm,
            "{what}: bpm is not the clock"
        );
        match voice.followed_bpm {
            Some(bpm) => assert_eq!(song.plan.tempo_bpm, bpm, "{what}: the voice lost"),
            None => assert_eq!(
                song.plan.tempo_bpm,
                style_tempo(&song.plan.preset),
                "{what}: no pulse, and not the style's own tempo"
            ),
        }
        for s in voice.samples.iter().chain(&track.samples) {
            assert!(s.is_finite() && s.abs() <= 1.0, "{what}: {s} is not audio");
        }
        let peak = voice.samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(
            (peak - MINUS_ONE_DBFS).abs() < 1e-3,
            "{what}: the voice peaks at {peak}, not -1 dBFS"
        );
        // What the studio draws is what it plays: the count-in included.
        assert_eq!(voice.wave, envelope(&voice.samples), "{what}: voice wave");
        assert_eq!(track.wave, envelope(&track.samples), "{what}: track wave");
    }

    /// The waveform the studio draws: the peak of each of 600 equal chunks.
    fn envelope(samples: &[f32]) -> Vec<f32> {
        let chunk = samples.len().div_ceil(600).max(1);
        samples
            .chunks(chunk)
            .map(|c| c.iter().fold(0.0f32, |m, s| m.max(s.abs())))
            .collect()
    }

    /// Saves and mixes through the session path the shell uses (AC6).
    fn assert_it_mixes(song: &Accompaniment, what: &str) {
        let session = build_song("qa", TENSE, 55, Some(&song.voice), Some(&song.track), None);
        let rate = song.voice.sample_rate;
        let stems: Vec<(&str, StemKind, u32)> = session
            .stems
            .iter()
            .map(|s| (s.name.as_str(), s.kind, s.sample_rate))
            .collect();
        assert_eq!(
            stems,
            [
                ("voice", StemKind::Other, rate),
                ("drums", StemKind::Beat, rate)
            ],
            "{what}"
        );
        assert_eq!(session.settings.bpm, song.plan.tempo_bpm, "{what}");
        assert_eq!(session.settings.beats_per_bar, 4.0, "{what}");
        assert_eq!(
            session.settings.root_hz,
            song.voice.followed_root_hz.unwrap_or(220.0),
            "{what}"
        );
        let mix = session.mixdown().unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(mix.sample_rate, rate, "{what}");
        assert_eq!(
            mix.samples.len(),
            song.voice.samples.len(),
            "{what}: the session's bar is not the stems' bar"
        );
        let sum: Vec<f32> = song
            .voice
            .samples
            .iter()
            .zip(&song.track.samples)
            .map(|(v, t)| v + 0.9 * t)
            .collect();
        let peak = sum.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let gain = if peak > 1.0 { 1.0 / peak } else { 1.0 };
        let worst = mix
            .samples
            .iter()
            .zip(&sum)
            .map(|(m, s)| (m - s * gain).abs())
            .fold(0.0f32, f32::max);
        assert!(
            worst < 1e-5,
            "{what}: the mix is not voice + drums ({worst})"
        );
    }

    /// The scenes about how a take *starts* (onset, breath, smack, clipping,
    /// level), and the ones about how it *moves* (swing, drift, pause, speech,
    /// legato) — split so neither test is the suite's long pole.
    const STARTS: std::ops::Range<usize> = 0..5;
    const MOVES: std::ops::Range<usize> = 5..10;

    /// The real-shaped takes in `which`, analysed at `rate`.
    fn ac2_at(rate: u32, which: std::ops::Range<usize>) {
        let hop = hop_ms(rate);
        let all = scenes(rate);
        assert_eq!(all.len(), MOVES.end, "harness: a scene is not run");
        for scene in all.into_iter().skip(which.start).take(which.len()) {
            let what = format!("{} at {rate} Hz", scene.name);
            let song = accompany_take(&scene.take, rate, scene.style, TENSE)
                .unwrap_or_else(|e| panic!("{what}: {e}"));
            assert_one_mix(&song, rate, &what);
            if let Some(sung) = scene.sung_bpm {
                let followed = song
                    .voice
                    .followed_bpm
                    .unwrap_or_else(|| panic!("{what}: a sung pulse was not followed"));
                assert!(
                    (followed / sung - 1.0).abs() < 0.02,
                    "{what}: sung at {sung}, followed at {followed:.2}"
                );
            }
            let landed = landing(&scene.take, &song, scene.first_note);
            // The note is on the downbeat if the downbeat falls within its
            // onset — from its first sample to full level — give or take the
            // spec's one analysis hop.
            let early = scene.attack * 1000.0 + hop;
            assert!(
                (-early..=hop).contains(&landed.error_ms),
                "{what}: the first note lands {:+.1} ms from the downbeat (allowed {:+.1}..{:+.1})",
                landed.error_ms,
                -early,
                hop
            );
            assert_eq!(
                landed.want_bar, 1,
                "{what}: a 0.7 s lead-in enters on bar 2"
            );
            assert_eq!(landed.nearest_bar, landed.want_bar, "{what}: wrong bar");
        }
    }

    #[test]
    fn ac2_real_starts_enter_on_the_downbeat_at_48k() {
        ac2_at(48_000, STARTS);
    }

    #[test]
    fn ac2_real_rhythms_enter_on_the_downbeat_at_48k() {
        ac2_at(48_000, MOVES);
    }

    #[test]
    fn ac2_real_starts_enter_on_the_downbeat_at_44k1() {
        ac2_at(44_100, STARTS);
    }

    #[test]
    fn ac2_real_rhythms_enter_on_the_downbeat_at_44k1() {
        ac2_at(44_100, MOVES);
    }

    #[test]
    fn ac2_real_takes_enter_on_the_downbeat_of_a_headset_at_16k() {
        ac2_at(16_000, STARTS);
        ac2_at(16_000, MOVES);
    }

    #[test]
    fn ac2_a_lead_in_of_any_length_gets_exactly_one_bar_of_drums() {
        // Owner decision (2026-09-27): however long the singer waits after
        // tapping, the drums play exactly one bar before them. At 131 BPM a bar
        // is 1.832 s; a note 1.9 s in and a note 10 s in both enter on bar 2's
        // downbeat. Before, the 10 s wait became five bars of drums over room
        // noise — replayed on every loop.
        let rate = 16_000;
        for lead in [1.9, 10.0] {
            let take = in_room(sing(rate, 131.0, 8, lead, 0.010), -55.0, 14);
            let song = accompany_take(&take, rate, "trap", TENSE).expect("a sung take");
            let what = format!("a {lead} s lead-in");
            assert_one_mix(&song, rate, &what);
            let landed = landing(&take, &song, at(rate, lead));
            assert_eq!(landed.nearest_bar, 1, "{what}: not after exactly one bar");
            assert!(
                (-(10.0 + hop_ms(rate))..=hop_ms(rate)).contains(&landed.error_ms),
                "{what}: {:+.1} ms from the downbeat",
                landed.error_ms
            );
            // The wait is gone from the loop: one count-in bar plus the eight
            // sung beats (3.66 s, two bars), rounded up — not seven bars.
            assert!(
                song.voice.bars <= 4,
                "{what}: the loop is {} bars — the wait is still in it",
                song.voice.bars
            );
            // Only whole bars are dropped, so the voice's first sound is inside
            // the count-in bar and the singer is still on the downbeat.
            let first_sound = song
                .voice
                .samples
                .iter()
                .position(|s| *s != 0.0)
                .expect("sound");
            assert!(first_sound <= bar_len(&song), "{what}: no count-in");
        }
    }

    #[test]
    fn ac2_a_soft_sung_onset_in_a_quiet_room_enters_on_the_downbeat() {
        // Measured on fb253ef (deterministic; release and debug agree):
        //   48 kHz,   60 ms onset, -50 dBFS RMS room: +700.0 ms (lands at bar 1.344)
        //   48 kHz,   80 ms onset, -50 dBFS RMS room: +700.0 ms
        //   44.1 kHz, 40 ms onset, -50 dBFS RMS room: +700.0 ms
        // The same notes in a -60 dBFS room land -19.0 / -26.3 / -14.7 ms —
        // inside their onset ramps, which is right.
        //
        // Why: the onset detector's implicit silent frame before the first makes
        // a -50 dBFS room fire an onset at t = 0.000. A soft note's own onset is
        // stamped *after* its first voiced frame (0.741 s against 0.715 s), so
        // the segment [0.000, 0.741) holds voiced frames and becomes notes[0],
        // onset 0.0 — the case SPEC-0042 says `assemble_notes` rules out.
        // `attack_start` then finds room noise at t = 0, and the start of the
        // take — not the note — is put on the downbeat: the 0.7 s lead-in plays
        // over the drums and the singer enters a third of a bar late.
        for (rate, attack) in [(48_000, 0.060), (48_000, 0.080), (44_100, 0.040)] {
            let take = in_room(sing(rate, 118.0, 8, LEAD, attack), -50.0, 1);
            let song = accompany_take(&take, rate, "corrido", TENSE).expect("a sung take");
            let landed = landing(&take, &song, at(rate, LEAD));
            let early = attack * 1000.0 + hop_ms(rate);
            assert!(
                (-early..=hop_ms(rate)).contains(&landed.error_ms),
                "{rate} Hz, {attack} s onset: the first note lands {:+.1} ms from the downbeat",
                landed.error_ms
            );
        }
    }

    #[test]
    fn ac2_a_sibilant_before_the_first_vowel_does_not_make_the_singer_late() {
        // "sí", "se", "so": 110 ms of "s" (-18 dB under the vowel's peak) runs
        // straight into the first sung vowel at 0.7 s. The sung note — the
        // voiced part (AC2: "the take's first sung note") — lands:
        //   16 kHz:   +46.9 ms after the downbeat (allowed: one hop, +16.0 ms)
        //   44.1 kHz: +23.1 ms (one hop: 5.8 ms)
        //   48 kHz:   +25.6 ms (one hop: 5.3 ms)
        // The commit itself calls 44 ms behind the kick audible, and 16 kHz is
        // the headset rate. The anchor is the first sample at 30% of the
        // note's early peak, searched from the detected onset — which, at
        // 16 kHz, sits inside the "s".
        let rate = 16_000;
        let note = at(rate, LEAD);
        let mut take = sing(rate, 118.0, 8, LEAD, 0.010);
        let s = hiss(at(rate, 0.110), 0.6 * linear(-18.0), 6, true);
        mix_in(&mut take, &s, note - s.len());
        let take = in_room(take, -50.0, 3);
        let song = accompany_take(&take, rate, "trap", TENSE).expect("a sung take");
        let landed = landing(&take, &song, note);
        assert!(
            (-(10.0 + hop_ms(rate))..=hop_ms(rate)).contains(&landed.error_ms),
            "the vowel lands {:+.1} ms from the downbeat",
            landed.error_ms
        );
    }

    #[test]
    fn ac2_a_quiet_take_in_a_noisy_room_enters_on_the_downbeat() {
        // A -30 dBFS-peak take (a quiet singer on a laptop) under room noise.
        // The attack search starts 20 ms *before* the detected onset and takes
        // the first sample at 30% of the note's early peak — which room noise
        // can reach. Across 12 noise seeds at -50 dBFS, 1 in 12 lands out of
        // tolerance at 16 kHz (+38.5 ms) and at 48 kHz (+7.6 ms); at -55 dBFS,
        // none. The cases below, measured on fb253ef:
        for (rate, room_db, seed) in [
            (16_000, -50.0, 5), // +38.5 ms (one hop: 16.0 ms)
            (16_000, -45.0, 6), // +39.9 ms
            (44_100, -45.0, 6), // +27.6 ms (one hop: 5.8 ms)
            (48_000, -45.0, 6), // +20.6 ms (one hop: 5.3 ms)
        ] {
            let quiet: Vec<f32> = sing(rate, 118.0, 8, LEAD, 0.010)
                .iter()
                .map(|s| s * (linear(-30.0) / 0.6) as f32)
                .collect();
            let take = in_room(quiet, room_db, seed);
            let song = accompany_take(&take, rate, "trap", TENSE).expect("a sung take");
            let landed = landing(&take, &song, at(rate, LEAD));
            assert!(
                (-(10.0 + hop_ms(rate))..=hop_ms(rate)).contains(&landed.error_ms),
                "{rate} Hz, {room_db} dBFS room: the first note lands {:+.1} ms from the downbeat",
                landed.error_ms
            );
        }
    }

    #[test]
    fn ac1_the_voice_sets_the_clock_whatever_the_style_and_rate() {
        // Tempos no style has (105, 140, 160, 92), each under a style whose own
        // tempo is far from it, at each rate.
        for (rate, bpm, style) in [
            (48_000, 126.0, "trap"),
            (44_100, 97.0, "metal"),
            (16_000, 131.0, "corrido"),
            (16_000, 112.0, "free"),
        ] {
            let take = in_room(sing(rate, bpm, 8, 0.4, 0.010), -55.0, 21);
            let song = accompany_take(&take, rate, style, TENSE).expect("a sung take");
            let what = format!("{bpm} BPM under {style} at {rate} Hz");
            assert_one_mix(&song, rate, &what);
            let followed = song.voice.followed_bpm.expect("a sung pulse is followed");
            assert!((followed / bpm - 1.0).abs() < 0.02, "{what}: {followed:.2}");
            assert_eq!(bar_len(&song), bar_at(followed, rate), "{what}: bar length");
            assert_eq!(
                song.track.samples.len() / song.track.bars as usize,
                bar_at(followed, rate),
                "{what}: the track's bar"
            );
            assert!(
                (followed - style_tempo(style)).abs() > 5.0,
                "{what}: fixture"
            );
        }
    }

    #[test]
    fn ac1_ac4_a_tempo_or_meter_written_in_the_style_is_never_read() {
        // With a pulse: the voice. Without one: the style's own tempo. Never
        // the number in the text — and never its 6/8 either.
        let rate = 16_000;
        let pulsed = in_room(sing(rate, 104.0, 8, 0.4, 0.010), -55.0, 22);
        let song = accompany_take(&pulsed, rate, "trap a 150 bpm", TENSE).expect("a take");
        let followed = song.voice.followed_bpm.expect("a pulse");
        assert!((followed - 104.0).abs() < 2.0, "{followed}");
        assert_eq!(song.plan.tempo_bpm, followed);

        let held = in_room(vowel(rate, 260.0, 2.5, 0.02, 0.6), -55.0, 23);
        for (style, preset) in [
            ("metal a 75 bpm", "metal"),
            ("corrido tumbado en 6/8 a 150 bpm", "corrido"),
        ] {
            let song = accompany_take(&held, rate, style, TENSE).expect("a take");
            assert_eq!(song.voice.followed_bpm, None, "{style}: a held note");
            assert_eq!(song.plan.preset, preset);
            assert_eq!(song.plan.tempo_bpm, style_tempo(preset), "{style}");
            assert_eq!(
                (song.plan.meter.beats, song.plan.meter.unit),
                (4, 4),
                "{style}"
            );
            assert_one_mix(&song, rate, style);
        }
    }

    #[test]
    fn ac4_every_style_brings_its_own_tempo_to_a_take_with_no_pulse() {
        // A held, vibrato-sung vowel: a pitch, no pulse.
        let rate = 44_100;
        let held = in_room(vowel(rate, 260.0, 2.5, 0.02, 0.6), -55.0, 24);
        for (style, bpm) in STYLE_TEMPOS {
            let song = accompany_take(&held, rate, style, TENSE).expect("a held note");
            assert_eq!(song.voice.followed_bpm, None, "{style}");
            assert_eq!(song.plan.tempo_bpm, bpm, "{style}");
            assert_eq!(song.voice.bpm, bpm, "{style}: the result's own tempo");
            assert_eq!(bar_len(&song), bar_at(bpm, rate), "{style}");
            assert_one_mix(&song, rate, style);
            // What the shell receives says so: no followed tempo, the style's.
            let json = serde_json::to_value(&song).expect("serializes");
            assert!(json["voice"]["followedBpm"].is_null(), "{style}");
            assert_eq!(json["voice"]["bpm"], bpm, "{style}");
            assert_eq!(json["voice"]["part"], "voice", "{style}");
        }
    }

    /// A style's lanes as beat voices, mapped here rather than by the app.
    fn lanes_of(style: &str) -> Vec<BeatVoiceSpec> {
        plan_sound(&parse_intent(style))
            .voices
            .iter()
            .map(|lane| BeatVoiceSpec {
                kind: match lane.role {
                    VoiceRole::Kick => DrumKind::Kick,
                    VoiceRole::Snare => DrumKind::Snare,
                    VoiceRole::Hat => DrumKind::HiHat,
                },
                onsets: lane.onsets,
                steps: lane.steps,
                rotate: lane.rotate,
                level: lane.level,
            })
            .collect()
    }

    #[test]
    fn ac3_the_track_is_exactly_the_styles_pattern_on_the_voices_clock() {
        // The same take under each style: the drums are that preset's lanes —
        // the ones the describe path plays for the style's name — rendered on
        // the voice's clock at the take's rate, and no two styles sound alike.
        let rate = 44_100;
        let take = in_room(sing(rate, 126.0, 8, 0.4, 0.010), -55.0, 25);
        let mut tracks: Vec<(&str, Vec<f32>)> = Vec::new();
        for style in style_names() {
            let song = accompany_take(&take, rate, style, TENSE).expect("a sung take");
            assert_eq!(song.plan.preset, style);
            assert_eq!(song.plan.voices, plan_sound(&parse_intent(style)).voices);
            let tempo = Tempo::new(song.plan.tempo_bpm, 4.0).expect("a tempo");
            let config = BeatConfig {
                voices: lanes_of(style),
                bars: song.track.bars,
            };
            let expected = build_beat(&tempo, rate, &config).expect("valid lanes");
            assert!(
                song.track.samples == expected.samples,
                "{style}: the track is not the preset's lanes on the voice's clock"
            );
            tracks.push((style, song.track.samples));
        }
        for (i, (a, first)) in tracks.iter().enumerate() {
            for (b, second) in &tracks[i + 1..] {
                assert!(first != second, "{a} and {b} sound the same");
            }
        }
    }

    #[test]
    fn ac5_the_cards_are_on_the_takes_own_grid_and_the_key_survives_a_save() {
        // Six notes at 311 Hz — not a simple ratio of Easy Mode's 220.
        let rate = 16_000;
        let mut take = vec![0.0f32; at(rate, 3.6)];
        for k in 0..6 {
            let note = vowel(rate, 311.0, 0.4, 0.01, 0.6);
            mix_in(&mut take, &note, at(rate, 0.4 + 0.5 * k as f64));
        }
        let take = in_room(take, -55.0, 26);
        let song = accompany_take(&take, rate, "corrido", TENSE).expect("a sung take");
        let root = song.voice.followed_root_hz.expect("a sung take has a root");
        assert!(
            (1200.0 * (root / 311.0).log2()).abs() < 20.0,
            "root {root:.1}"
        );
        // The cards are the take quantized onto *its* grid: a note sung at the
        // root is 1:1, barely corrected. On 220's grid it would be 7:5, 17
        // cents away.
        let card = song.voice.notes.first().expect("a card");
        assert_eq!((card.num, card.den, card.octave), (1, 1, 0));
        assert!(card.cents.expect("a sung note").abs() < 10.0, "{card:?}");

        let dir = std::env::temp_dir().join(format!("gooz_qa_r0042_key_{}", std::process::id()));
        let path = save_session(
            &dir,
            "key",
            TENSE,
            55,
            Some(&song.voice),
            Some(&song.track),
            None,
        )
        .expect("saves");
        let loaded = gooz_session::Song::load(&path).expect("loads");
        std::fs::remove_dir_all(&dir).ok();
        // serde_json (without `float_roundtrip`) may read a float back one ulp
        // off, so the file is compared to the relative precision it keeps.
        let same = |a: f64, b: f64| (a / b - 1.0).abs() < 1e-12;
        assert!(same(loaded.settings.root_hz, root), "the saved key");
        assert!(
            same(loaded.settings.bpm, song.plan.tempo_bpm),
            "the saved tempo"
        );
        assert_eq!(loaded.stems[0].name, "voice");
    }

    #[test]
    fn ac6_the_stems_are_one_mix_at_every_rate_and_length() {
        // Shorter than a bar, a very long lead-in, and the 30 s maximum.
        let cases = [
            (
                "one 0.3 s note, shorter than a bar",
                44_100,
                in_room(sing_at(44_100, &[0.05], 0.3, 0.01, 0.4), -55.0, 31),
            ),
            (
                "a 10 s lead-in",
                16_000,
                in_room(sing(16_000, 131.0, 6, 10.0, 0.01), -55.0, 32),
            ),
            (
                "the 30 s maximum",
                16_000,
                in_room(sing(16_000, 123.0, 59, 0.5, 0.01), -55.0, 33),
            ),
            (
                "an ordinary take",
                48_000,
                in_room(sing(48_000, 118.0, 6, 0.7, 0.01), -55.0, 34),
            ),
        ];
        for (what, rate, take) in cases {
            let song = accompany_take(&take, rate, "metal", TENSE)
                .unwrap_or_else(|e| panic!("{what}: {e}"));
            assert_one_mix(&song, rate, what);
            assert_it_mixes(&song, what);
        }
    }

    #[test]
    fn ac6_mixdown_agrees_with_the_stems_at_every_style_tempo_and_rate() {
        // A pulseless note, so each style's own tempo is the clock — the
        // session's bar (from `Settings`) must equal the stems' bar exactly.
        for rate in [48_000, 44_100, 16_000] {
            let held = in_room(vowel(rate, 260.0, 0.6, 0.02, 0.6), -60.0, 35);
            for (style, _) in STYLE_TEMPOS {
                let song = accompany_take(&held, rate, style, TENSE).expect("a note");
                let what = format!("{style} at {rate} Hz");
                assert_one_mix(&song, rate, &what);
                assert_it_mixes(&song, &what);
            }
        }
    }

    #[test]
    fn ac6_a_take_cut_mid_note_does_not_click_where_it_was_cut() {
        // Tap to start and tap to stop land wherever the singer is: here both
        // cuts fall mid-vowel, at full level. SPEC-0042 fades both ends, so the
        // looping voice neither starts nor stops on a step: the first and last
        // millisecond of the take are well under the level they were sung at.
        let rate = 44_100;
        let whole = in_room(sing(rate, 118.0, 6, 0.0, 0.0), -60.0, 38);
        let take = whole[at(rate, 0.1)..at(rate, 2.74)].to_vec();
        let song = accompany_take(&take, rate, "trap", TENSE).expect("a sung take");
        let pad = delay_of(&take, &song);
        let level = |xs: &[f32]| xs.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let gain = level(&song.voice.samples) / level(&take);
        let ms = at(rate, 0.001);
        let n = take.len();
        let voice = &song.voice.samples[pad..pad + n];
        for (what, sung, played) in [
            ("start", &take[..ms], &voice[..ms]),
            ("end", &take[n - ms..], &voice[n - ms..]),
        ] {
            assert!(level(sung) > 0.2, "fixture: the take is loud at its {what}");
            let kept = level(played) / (gain * level(sung));
            assert!(
                kept < 0.5,
                "the voice's {what} is a step: {kept:.2} of full level"
            );
        }
    }

    #[test]
    fn ac6_each_stem_mutes_on_its_own_and_both_export() {
        let rate = 16_000;
        let take = in_room(sing(rate, 118.0, 6, 0.7, 0.01), -55.0, 36);
        let song = accompany_take(&take, rate, "trap", TENSE).expect("a sung take");
        let session = build_song("qa", TENSE, 55, Some(&song.voice), Some(&song.track), None);
        let with_muted = |stem: usize| {
            let mut muted = session.clone();
            muted.arrangement.placements[stem].muted = true;
            muted.mixdown().expect("mixes").samples
        };
        let drums_only = with_muted(0);
        assert_eq!(drums_only.len(), song.track.samples.len());
        for (m, t) in drums_only.iter().zip(&song.track.samples) {
            assert!(
                (m - 0.9 * t).abs() < 1e-6,
                "muting the voice left more than drums"
            );
        }
        assert_eq!(
            with_muted(1),
            song.voice.samples,
            "muting the drums left more than the voice"
        );

        let dir = std::env::temp_dir().join(format!("gooz_qa_r0042_stems_{}", std::process::id()));
        let stems = session.export_stems(&dir).expect("stems export");
        let names: Vec<String> = stems
            .iter()
            .filter_map(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .collect();
        let master = export_master(
            &dir,
            "qa",
            TENSE,
            55,
            Some(&song.voice),
            Some(&song.track),
            None,
        )
        .expect("the master exports");
        let bytes = std::fs::metadata(&master).map(|m| m.len());
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(names, ["00-voice.wav", "01-drums.wav"]);
        assert_eq!(
            bytes.expect("written"),
            44 + 2 * song.voice.samples.len() as u64,
            "a mono 16-bit master, as long as the stems"
        );
    }

    #[test]
    fn ac6_ac8_what_the_shell_sends_back_is_saved_as_a_voice() {
        // The webview holds the result as JSON and hands `voice` and `track`
        // back to save_session / export_master as `riff` and `beat`.
        let rate = 44_100;
        let take = in_room(sing(rate, 126.0, 6, 0.5, 0.01), -55.0, 37);
        let song = accompany_take(&take, rate, "corrido", TENSE).expect("a sung take");
        let json = serde_json::to_value(&song).expect("serializes");
        for key in ["plan", "voice", "track"] {
            assert!(json.get(key).is_some(), "the result has no {key}");
        }
        let voice: RiffView = serde_json::from_value(json["voice"].clone()).expect("a riff");
        let track: BeatView = serde_json::from_value(json["track"].clone()).expect("a beat");
        assert_eq!(
            voice, song.voice,
            "the voice did not survive the round trip"
        );
        assert_eq!(
            track, song.track,
            "the track did not survive the round trip"
        );
        let session = build_song("qa", TENSE, 55, Some(&voice), Some(&track), None);
        assert_eq!(session.stems[0].name, "voice");
        assert_eq!(session.settings.bpm, song.plan.tempo_bpm);
        session.mixdown().expect("one rate, one length");
    }

    #[test]
    fn ac7_hostile_input_is_typed_bounded_and_never_panics() {
        let rate = 16_000;
        let take = in_room(sing(rate, 118.0, 3, 0.2, 0.01), -60.0, 41);
        let typed = |samples: &[f32], expected: DspError, what: &str| {
            assert_eq!(
                accompany_take(samples, rate, "trap", TENSE).map(|_| ()),
                Err(expected),
                "{what}"
            );
        };
        for bad in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN] {
            let mut t = take.clone();
            t[take.len() - 1] = bad;
            typed(&t, DspError::NonFiniteSample, "a non-finite last sample");
        }
        for bad in [1.000_001, -1.000_001, f32::MAX] {
            let mut t = take.clone();
            t[0] = bad;
            typed(&t, DspError::SampleOutOfRange, "a sample past full scale");
        }
        typed(
            &take[..2_047],
            DspError::WindowTooLarge,
            "shorter than the window",
        );
        let mut click = vec![0.0f32; rate as usize];
        click[8_000] = 0.9;
        typed(&click, DspError::Silent, "a single click");
        typed(&vec![0.5; rate as usize], DspError::Silent, "DC");
        for level in [-30.0, -50.0, -70.0] {
            typed(
                &room(2 * rate as usize, level, 42),
                DspError::Silent,
                "a room alone",
            );
        }

        // Full scale is audio, not an error.
        let mut edge = take.clone();
        let middle = take.len() / 2;
        edge[middle] = 1.0;
        edge[middle + 1] = -1.0;
        let song = accompany_take(&edge, rate, "trap", TENSE).expect("±1.0 is in range");
        assert_one_mix(&song, rate, "±1.0");

        // Any rate: a typed error, or a result bounded by the take plus a
        // count-in bar and a padding bar at the slowest tempo.
        for hostile in [1, 2, 100, 8_000, 1_000_000, u32::MAX] {
            if let Ok(song) = accompany_take(&take, hostile, "trap", TENSE) {
                let slowest_bar = bar_at(60.0, hostile);
                assert!(song.voice.samples.len() <= take.len() + 2 * slowest_bar);
                assert_one_mix(&song, hostile, &format!("rate {hostile}"));
            }
        }

        // One window long, or scaled almost to nothing: whatever comes back is
        // audio.
        let window = take[at(rate, 0.2)..][..Config::default().window].to_vec();
        let tiny = |scale: f32| take.iter().map(|s| s * scale).collect::<Vec<f32>>();
        for (what, samples) in [
            ("one window", window),
            ("at 1e-20", tiny(1e-20)),
            ("at 1e-40", tiny(1e-40)),
        ] {
            if let Ok(song) = accompany_take(&samples, rate, "trap", TENSE) {
                assert_one_mix(&song, rate, what);
            }
        }
    }

    #[test]
    fn ac7_the_same_take_gives_the_same_song_whatever_ran_before() {
        let rate = 16_000;
        let first = in_room(sing(rate, 118.0, 6, 0.5, 0.01), -55.0, 43);
        let other = in_room(sing(rate, 97.0, 6, 0.3, 0.01), -55.0, 44);
        let before = accompany_take(&first, rate, "corrido", TENSE).expect("a take");
        accompany_take(&other, rate, "metal", TENSE).expect("a take");
        accompany_take(&other, 44_100, "free", TENSE).expect("a take");
        let after = accompany_take(&first, rate, "corrido", TENSE).expect("a take");
        assert!(before == after, "the same take and style gave another song");
    }

    #[test]
    fn ac8_the_shell_offers_the_engines_styles_through_the_accompany_command() {
        // `cargo check` on the shell proves the command compiles; not that it
        // is registered, nor that the webview calls it.
        let shell = include_str!("../src-tauri/src/main.rs");
        let handlers = shell
            .split("generate_handler![")
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .expect("the shell registers its commands");
        for command in ["record_stop_accompany", "styles"] {
            assert!(
                handlers.split(',').any(|h| h.trim() == command),
                "{command} is not registered"
            );
        }
        let ui = include_str!("../ui/main.js");
        assert!(ui.contains(r#"stop: "record_stop_accompany""#));
        assert!(ui.contains(r#"invoke("styles")"#));
        // The browser preview's fallback list is the engine's list.
        let preview: Vec<&str> = ui
            .split("const PREVIEW_STYLES = [")
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .expect("a preview list")
            .split(',')
            .map(|s| s.trim().trim_matches('"'))
            .collect();
        assert_eq!(preview, style_names());
        assert!(include_str!("../ui/index.html").contains(r#"data-mode="style""#));
    }

    #[test]
    fn regression_every_path_saves_the_clock_it_was_laid_out_at_under_its_own_name() {
        let rate = 48_000;
        let saved = |riff: &RiffView| {
            let song = build_song("qa", TENSE, 55, Some(riff), None, None);
            let stem = &song.stems[0];
            (song.settings.bpm, stem.name.clone(), stem.kind)
        };
        let laid_out = |riff: &RiffView| 240.0 / (riff.seconds / f64::from(riff.bars));

        // Hum → riff with a pulse: its followed tempo, as a guitar.
        let hum = riff_from_take(&sing(rate, 126.0, 8, 0.2, 0.01), rate, TENSE).expect("a hum");
        let followed = hum.followed_bpm.expect("a pulse");
        assert_eq!(hum.part, Part::Guitar);
        assert_eq!(hum.bpm, followed);
        assert!((laid_out(&hum) - followed).abs() < 0.05);
        assert_eq!(saved(&hum), (followed, "guitar".into(), StemKind::Riff));

        // Hum → riff with no pulse: Easy Mode's 92.
        let held =
            riff_from_take(&vowel(rate, 260.0, 2.0, 0.02, 0.6), rate, TENSE).expect("a held note");
        assert_eq!(held.followed_bpm, None);
        assert!((laid_out(&held) - 92.0).abs() < 0.05);
        assert_eq!(saved(&held), (92.0, "guitar".into(), StemKind::Riff));

        // R-0040's knock: an instrument, on Easy Mode's clock.
        let mut knock = vec![0.0f32; rate as usize];
        let burst: Vec<f32> = hiss(at(rate, 0.03), 0.15, 45, true)
            .iter()
            .map(|s| s.clamp(-0.9, 0.9))
            .collect();
        mix_in(&mut knock, &burst, at(rate, 0.3));
        let figure = instrument_from_take(&knock, rate, TENSE).expect("a knock");
        assert_eq!(figure.part, Part::Instrument);
        assert!((laid_out(&figure) - 92.0).abs() < 0.05);
        assert_eq!(saved(&figure), (92.0, "instrument".into(), StemKind::Riff));

        // R-0027's described song: the prompt's tempo (no style has 135).
        let described = describe_song("trap a 135 bpm", 2);
        assert_eq!(described.riff.part, Part::Guitar);
        assert_eq!(described.riff.bpm, 135.0);
        assert_eq!(
            saved(&described.riff),
            (135.0, "guitar".into(), StemKind::Riff)
        );
        let session = build_song(
            "qa",
            TENSE,
            55,
            Some(&described.riff),
            Some(&described.beat),
            None,
        );
        assert_eq!(
            session.mixdown().expect("mixes").samples.len(),
            described.beat.samples.len(),
            "a described song's session bar is not its drums' bar"
        );

        // The demo, unchanged.
        let demo = demo_riff();
        assert_eq!(demo.part, Part::Guitar);
        assert_eq!(saved(&demo), (92.0, "guitar".into(), StemKind::Riff));
    }

    #[test]
    fn regression_a_described_six_eight_song_mixes_to_its_full_length() {
        // `build_song` now saves `riff.bpm` (135) where it saved 92, but still
        // writes `beats_per_bar: 4` whatever the meter. For a 6/8 prompt the
        // stems are 6-beat bars and the session's bar is 4 beats:
        //   drums 256000 samples; mix before R-0042 250434 (97.8%), now 170666 (66.7%).
        // Not reachable from the app today: the shell has no describe command.
        let described = describe_song("corrido tumbado en 6/8 a 135 bpm", 2);
        let session = build_song(
            "qa",
            TENSE,
            55,
            Some(&described.riff),
            Some(&described.beat),
            None,
        );
        assert_eq!(
            session.mixdown().expect("mixes").samples.len(),
            described.beat.samples.len()
        );
    }

    #[test]
    fn ac7_a_room_that_drowns_the_voice_is_refused_not_misaligned() {
        // A fan or an air conditioner: rumble within 8 dB of a quiet singer.
        // The pitch tracker cannot hear singing through it, so there is no
        // sung note to put on the downbeat — and the answer is a typed error
        // the app shows, not a track laid against the rumble. (The gate that
        // keeps a noisy room out of the note's start is tested directly in
        // `gooz-dsp`, with a pitch track that does hear the voice.)
        for rate in [48_000, 16_000] {
            let mut take: Vec<f32> = sing(rate, 118.0, 8, LEAD, 0.010)
                .iter()
                .map(|s| s * (linear(-30.0) / 0.6) as f32)
                .collect();
            let rumble_amp = linear(-38.0) * std::f64::consts::SQRT_2;
            for (i, s) in take.iter_mut().enumerate() {
                let t = i as f64 / f64::from(rate);
                let hum = (std::f64::consts::TAU * 48.0 * t).sin()
                    + 0.5 * (std::f64::consts::TAU * 61.0 * t + 1.0).sin();
                *s += (rumble_amp / 1.118 * hum) as f32;
            }
            let take = in_room(take, -60.0, 21);
            assert_eq!(
                accompany_take(&take, rate, "trap", TENSE).unwrap_err(),
                DspError::Silent,
                "{rate} Hz"
            );
        }
    }

    #[test]
    fn ac2_a_stray_pitched_blip_before_the_song_is_not_its_first_note() {
        // A 30 ms pitched squeak — a chair, a hum under the breath — 0.35 s
        // before the first sung note. It is voiced, but far too short to be a
        // sung note (80 ms). Were any voiced frame enough, the blip would be
        // put on the downbeat and the singer would enter 350 ms late.
        for rate in [48_000, 16_000] {
            let mut take = sing(rate, 118.0, 8, LEAD, 0.010);
            let blip_at = at(rate, LEAD - 0.35);
            let blip: Vec<f32> = (0..at(rate, 0.030))
                .map(|i| {
                    let t = i as f64 / f64::from(rate);
                    (0.5 * (std::f64::consts::TAU * 330.0 * t).sin()) as f32
                })
                .collect();
            mix_in(&mut take, &blip, blip_at);
            let take = in_room(take, -60.0, 22);
            let song = accompany_take(&take, rate, "trap", TENSE).expect("a sung take");
            let landed = landing(&take, &song, at(rate, LEAD));
            assert!(
                (-(10.0 + hop_ms(rate))..=hop_ms(rate)).contains(&landed.error_ms),
                "{rate} Hz: the note lands {:+.1} ms from the downbeat — the blip was taken for it",
                landed.error_ms
            );
        }
    }
}

#[test]
fn a_stereo_microphone_is_heard_at_its_own_pitch_and_tempo() {
    // Most USB microphones and interfaces record two channels, and the capture
    // hands over interleaved frames: L R L R … Read as one channel, that is
    // twice as many samples as there are frames — half speed, an octave low.
    // The shell now downmixes with `Take::mono` before anything listens.
    let mono = Take::at(118.0, 262.0).render();
    let interleaved: Vec<f32> = mono.iter().flat_map(|&s| [s, s]).collect();
    let take = gooz_audio::Take::new(interleaved.clone(), SR, 2);

    let heard = accompany_take(&take.mono(), SR, "trap", TENSE).expect("a stereo take");
    let root = heard.voice.followed_root_hz.expect("a pitch");
    assert!(
        (1200.0 * (root / 262.0).log2()).abs() < 40.0,
        "root {root:.1} Hz"
    );
    let bpm = heard.voice.followed_bpm.expect("a pulse");
    assert!((bpm - 118.0).abs() < 4.0, "tempo {bpm:.1}");

    // And the defect this closes, so the test cannot pass without it: the raw
    // interleaved samples are heard an octave low.
    let misread = accompany_take(&interleaved, SR, "trap", TENSE).expect("still a take");
    let low = misread.voice.followed_root_hz.expect("a pitch");
    assert!(
        (1200.0 * (low / 131.0).log2()).abs() < 60.0,
        "interleaved stereo read as mono should sound an octave low; got {low:.1} Hz"
    );
}
