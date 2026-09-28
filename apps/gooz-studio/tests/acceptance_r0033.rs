//! R-0033 acceptance tests — singing over trap plays an 808 under you (AC7),
//! and the studio plays, saves and exports it with the voice and the drums,
//! without clipping, over any style (AC8). Realized by SPEC-0033 §2.4–§2.5.
//!
//! The takes are sung at 16 kHz (a headset's rate), which keeps the analysis
//! cheap. Every expectation is computed here from the pieces the spec names —
//! the plan's kick lane, `pattern_onsets`, `render_808`, the session's levels —
//! and the kick positions are cross-checked against the kick `render_beat`
//! actually plays, so a wrong onset computation cannot agree with itself.
//!
//! AC8's browser half (three sources through gain nodes set from `levels`,
//! started at one `at`) is SPEC-0033 §6's browser check with a mocked shell, not
//! part of this suite.

use std::f64::consts::TAU;
use std::path::PathBuf;

use gooz_model::{BassVoice, SoundPlan, VoiceRole};
use gooz_ratio::{Pattern, Tempo};
use gooz_session::{Song, StemKind};
use gooz_studio::{
    Accompaniment, BassView, accompany_take, build_song, export_master, save_session, style_names,
};
use gooz_synth::{Bass808, BassNote, BeatVoice, DrumKind, pattern_onsets, render_808, render_beat};

const RATE: u32 = 16_000;
const TENSE: u8 = 30;
/// Easy Mode's root, the grid's root when the take's was not followed.
const DEFAULT_ROOT_HZ: f64 = 220.0;
/// The levels `build_song` places each stem at (SPEC-0033 §2.4).
const VOICE_LEVEL: f32 = 1.0;
const DRUMS_LEVEL: f32 = 0.9;
const BASS_LEVEL: f32 = 1.0;

fn at(rate: u32, secs: f64) -> usize {
    (secs * f64::from(rate)).round() as usize
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, s| m.max(s.abs()))
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

/// A sung vowel at `hz`: ten harmonics falling off like a voice, vibrato from
/// 150 ms, a 10 ms onset and a 30 ms release, peaking at 0.6.
fn vowel(rate: u32, hz: f64, secs: f64) -> Vec<f32> {
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
            tone * (t / 0.010).min(1.0) * ((secs - t) / 0.03).clamp(0.0, 1.0)
        })
        .collect();
    let top = raw.iter().fold(0.0f64, |m, s| m.max(s.abs()));
    raw.iter().map(|s| (s / top * 0.6) as f32).collect()
}

/// `notes` vowels at `hz`, one per beat at `bpm`, the first 0.5 s in, in a
/// −55 dBFS RMS room.
fn sing(hz: f64, bpm: f64, notes: usize) -> Vec<f32> {
    let beat = 60.0 / bpm;
    let lead = 0.5;
    let mut take = vec![0.0f32; at(RATE, lead + notes as f64 * beat + 0.4)];
    for k in 0..notes {
        let from = at(RATE, lead + k as f64 * beat);
        for (slot, s) in take[from..].iter_mut().zip(vowel(RATE, hz, 0.8 * beat)) {
            *slot += s;
        }
    }
    let mut rng = Rng(0x0808);
    let room = 10f64.powf(-55.0 / 20.0);
    take.iter()
        .map(|s| s + (rng.gauss() * room) as f32)
        .collect()
}

fn accompany(take: &[f32], style: &str) -> Accompaniment {
    accompany_take(take, RATE, style, TENSE).unwrap_or_else(|e| panic!("sung over {style}: {e}"))
}

fn bass_of<'a>(song: &'a Accompaniment, what: &str) -> &'a BassView {
    song.bass
        .as_ref()
        .unwrap_or_else(|| panic!("{what}: singing over trap brought no bass"))
}

/// The plan's kick lane as the beat builder plays it: `E(k, n)` rotated.
fn kick_pattern(plan: &SoundPlan) -> Pattern {
    let kick = plan
        .voices
        .iter()
        .find(|v| v.role == VoiceRole::Kick)
        .expect("a kick lane");
    Pattern::euclidean(kick.onsets, kick.steps)
        .expect("a valid lane")
        .rotate(kick.rotate)
}

/// The clock the result was laid out on (4/4 at the plan's tempo).
fn clock(plan: &SoundPlan) -> Tempo {
    Tempo::new(plan.tempo_bpm, f64::from(plan.meter.beats)).expect("a tempo")
}

/// Where the kick really starts in the drums the style plays: the kick lane
/// rendered alone, each hit found as sound after at least 1 ms of silence.
fn kicks_played(song: &Accompaniment) -> Vec<usize> {
    let beat = render_beat(
        &[BeatVoice {
            kind: DrumKind::Kick,
            pattern: kick_pattern(&song.plan),
            level: 1.0,
        }],
        &clock(&song.plan),
        song.voice.bars,
        song.voice.sample_rate,
    );
    let quiet = at(song.voice.sample_rate, 0.001);
    (0..beat.len())
        .filter(|&i| beat[i] != 0.0 && beat[i.saturating_sub(quiet)..i].iter().all(|s| *s == 0.0))
        .collect()
}

/// SPEC-0033 §2.4: one note per kick, lasting until the next kick and the last
/// until the loop ends, at `hz`, rendered at `drive`.
fn hits_until_next_rendered(
    kicks: &[usize],
    len: usize,
    hz: f64,
    rate: u32,
    drive: f32,
) -> Vec<f32> {
    let r = f64::from(rate);
    let notes: Vec<BassNote> = kicks
        .iter()
        .enumerate()
        .map(|(k, &at)| BassNote {
            hz,
            onset_secs: at as f64 / r,
            duration_secs: (kicks.get(k + 1).copied().unwrap_or(len) - at) as f64 / r,
        })
        .collect();
    render_808(
        &notes,
        rate,
        len,
        &Bass808 {
            drive,
            ..Bass808::default()
        },
    )
}

/// The waveform the studio draws: the peak of each of 600 equal chunks.
fn envelope(samples: &[f32]) -> Vec<f32> {
    let chunk = samples.len().div_ceil(600).max(1);
    samples.chunks(chunk).map(peak).collect()
}

/// What export writes: voice, drums and bass at their levels, summed, scaled
/// down together only if the sum passes full scale.
fn export_mix(song: &Accompaniment, with_bass: bool) -> Vec<f32> {
    let bass = song.bass.as_ref().filter(|_| with_bass);
    let mut sum: Vec<f32> = song
        .voice
        .samples
        .iter()
        .zip(&song.track.samples)
        .map(|(v, d)| v * VOICE_LEVEL + d * DRUMS_LEVEL)
        .collect();
    if let Some(b) = bass {
        for (slot, s) in sum.iter_mut().zip(&b.samples) {
            *slot += s * BASS_LEVEL;
        }
    }
    let top = peak(&sum);
    if top > 1.0 {
        sum.iter_mut().for_each(|s| *s /= top);
    }
    sum
}

fn assert_close(got: &[f32], want: &[f32], tolerance: f32, what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: lengths");
    let (worst, i) = got
        .iter()
        .zip(want)
        .enumerate()
        .map(|(i, (g, w))| ((g - w).abs(), i))
        .fold((0.0f32, 0), |best, e| if e.0 > best.0 { e } else { best });
    assert!(
        worst <= tolerance,
        "{what}: off by {worst} at sample {i} ({} against {})",
        got[i],
        want[i]
    );
}

fn temp_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!("gooz_qa_r0033_{tag}_{}", std::process::id()))
}

// ---------------------------------------------------------------------------
// AC7 — singing over trap plays it under you.
// ---------------------------------------------------------------------------

#[test]
fn ac7_only_trap_brings_a_bass() {
    let take = sing(260.0, 126.0, 6);
    for style in style_names() {
        let song = accompany(&take, style);
        assert_eq!(
            song.bass.is_some(),
            style == "trap",
            "over {style} the accompaniment {}",
            if song.bass.is_some() {
                "has a bass part"
            } else {
                "has no bass part"
            }
        );
    }
}

#[test]
fn ac7_the_808_is_its_own_track_in_step_with_the_voice_and_drums() {
    let song = accompany(&sing(260.0, 126.0, 6), "trap");
    let bass = bass_of(&song, "a take at 126 BPM");
    let len = song.voice.samples.len();
    assert_eq!(bass.voice, BassVoice::Sub808);
    assert_eq!(
        bass.sample_rate, song.voice.sample_rate,
        "rate against the voice"
    );
    assert_eq!(
        bass.sample_rate, song.track.sample_rate,
        "rate against the drums"
    );
    assert_eq!(bass.samples.len(), len, "length against the voice");
    assert_eq!(
        bass.samples.len(),
        song.track.samples.len(),
        "length against the drums"
    );
    assert_eq!(bass.bars, song.voice.bars, "bars against the voice");
    assert_eq!(bass.bars, song.track.bars, "bars against the drums");
    let bar = (240.0 / song.plan.tempo_bpm * f64::from(RATE)).round() as usize;
    assert_eq!(len, bass.bars as usize * bar, "not whole bars of the clock");
    assert!(
        (bass.seconds - len as f64 / f64::from(RATE)).abs() < 1e-9,
        "seconds {}",
        bass.seconds
    );
    assert_eq!(
        bass.wave,
        envelope(&bass.samples),
        "what is drawn is what plays"
    );
    assert!(peak(&bass.samples) > 0.3, "the 808 is silent");
    for (i, s) in bass.samples.iter().enumerate() {
        assert!(s.is_finite() && s.abs() <= 1.0, "sample {i} is {s}");
    }
}

#[test]
fn ac7_every_808_note_starts_on_a_kick_and_lasts_until_the_next() {
    let song = accompany(&sing(260.0, 126.0, 6), "trap");
    let bass = bass_of(&song, "a take at 126 BPM");
    let x = &bass.samples;
    let len = x.len();
    let kicks = kicks_played(&song);
    assert!(
        kicks.len() >= 2,
        "harness: the trap loop has kicks: {kicks:?}"
    );
    assert_eq!(
        pattern_onsets(
            &kick_pattern(&song.plan),
            &clock(&song.plan),
            song.voice.bars,
            RATE
        ),
        kicks,
        "pattern_onsets is not where the kick plays"
    );

    // Each note starts from silence on the kick's sample, and sounds after it.
    let ms = |secs: f64| at(RATE, secs);
    assert!(
        x[..kicks[0]].iter().all(|s| *s == 0.0),
        "the 808 sounds before the first kick"
    );
    for &kick in &kicks {
        assert_eq!(
            x[kick], 0.0,
            "the note on the kick at {kick} does not start there"
        );
        if kick > 0 {
            assert_eq!(
                x[kick - 1],
                0.0,
                "the note before the kick at {kick} did not end there"
            );
        }
        let after = peak(&x[kick + ms(0.002)..kick + ms(0.027)]);
        assert!(
            after > 0.3,
            "the 808 does not hit on the kick at {kick} ({after})"
        );
    }
    // And lasts until the next kick: still sounding just before it.
    let ends: Vec<usize> = kicks[1..].iter().copied().chain([len]).collect();
    for (&kick, &end) in kicks.iter().zip(&ends) {
        let before = peak(&x[end - ms(0.030)..end - ms(0.005)]);
        assert!(
            before > 0.1,
            "the note from {kick} is not still sounding before {end} ({before})"
        );
    }
    assert_eq!(
        x[len - 1],
        0.0,
        "the last note does not release at the loop's end"
    );
}

#[test]
fn ac7_the_808_plays_the_grid_root_folded_into_its_register() {
    // Sung roots that fold to the bottom, the middle and the top of [40, 80).
    for hz in [330.0, 260.0, 311.0] {
        let song = accompany(&sing(hz, 118.0, 6), "trap");
        let what = format!("a take sung at {hz} Hz");
        let bass = bass_of(&song, &what);
        let grid_root = song.voice.followed_root_hz.unwrap_or(DEFAULT_ROOT_HZ);
        assert!(
            (40.0..80.0).contains(&bass.root_hz),
            "{what}: the 808 is at {} Hz, outside [40, 80)",
            bass.root_hz
        );
        let octaves = (grid_root / bass.root_hz).log2();
        assert!(
            (octaves - octaves.round()).abs() < 1e-9,
            "{what}: {} Hz is not whole octaves under the grid root {grid_root} Hz",
            bass.root_hz
        );
        // What plays is that pitch: the render of those hits at root_hz.
        let want = hits_until_next_rendered(
            &kicks_played(&song),
            bass.samples.len(),
            bass.root_hz,
            RATE,
            song.plan.drive,
        );
        assert!(
            bass.samples == want,
            "{what}: the 808 is not render_808 at root_hz"
        );
    }
}

#[test]
fn ac7_the_808_is_render_808_of_the_kicks_at_the_plans_drive() {
    // The style chips send no description, so the plan's drive is the intent
    // default (0.40, owner decision). A style text that asks for more or less
    // drive moves the 808 with it.
    let take = sing(260.0, 126.0, 6);
    let mut rendered: Vec<(String, f32, Vec<f32>)> = Vec::new();
    for style in ["trap", "trap distorsionado hasta que cruje", "trap limpio"] {
        let song = accompany(&take, style);
        let bass = bass_of(&song, style);
        let kicks = kicks_played(&song);
        let len = bass.samples.len();
        let want = hits_until_next_rendered(&kicks, len, bass.root_hz, RATE, song.plan.drive);
        assert!(
            bass.samples == want,
            "{style}: the 808 is not render_808 of the kicks at the plan's drive {}",
            song.plan.drive
        );
        if song.plan.drive > 0.0 {
            let undriven = hits_until_next_rendered(&kicks, len, bass.root_hz, RATE, 0.0);
            assert!(bass.samples != undriven, "{style}: the drive is not heard");
        }
        rendered.push((style.to_string(), song.plan.drive, bass.samples.clone()));
    }
    assert_eq!(
        rendered[0].1,
        gooz_model::DEFAULT_DRIVE,
        "a style chip's drive"
    );
    assert!(
        rendered[1].1 > rendered[0].1 && rendered[2].1 < rendered[0].1,
        "harness: the texts move the drive: {:?}",
        rendered.iter().map(|r| r.1).collect::<Vec<_>>()
    );
    assert!(
        rendered[0].2 != rendered[1].2 && rendered[0].2 != rendered[2].2,
        "the plan's drive does not change the 808"
    );
}

// ---------------------------------------------------------------------------
// AC8 — played, saved, exported, without clipping.
// ---------------------------------------------------------------------------

#[test]
fn ac8_the_song_holds_the_808_as_its_own_third_stem() {
    let song = accompany(&sing(260.0, 126.0, 6), "trap");
    let bass = bass_of(&song, "trap");
    let session = build_song(
        "qa",
        TENSE,
        55,
        Some(&song.voice),
        Some(&song.track),
        Some(bass),
    );
    session.validate().expect("a valid song");
    let stems: Vec<(&str, StemKind, u32, u32)> = session
        .stems
        .iter()
        .map(|s| (s.name.as_str(), s.kind, s.sample_rate, s.bars))
        .collect();
    assert_eq!(
        stems,
        [
            ("voice", StemKind::Other, RATE, song.voice.bars),
            ("drums", StemKind::Beat, RATE, song.track.bars),
            ("808", StemKind::Other, RATE, bass.bars),
        ]
    );
    assert!(
        session.stems[2].samples == bass.samples,
        "the 808 stem is not the 808"
    );
    let placement = session
        .arrangement
        .placements
        .iter()
        .find(|p| p.stem == 2)
        .expect("the 808 is placed");
    assert_eq!(
        (placement.start_bar, placement.muted, placement.level),
        (0, false, BASS_LEVEL),
        "the 808 is placed at bar 0, unmuted, at level 1"
    );
    let span = session
        .arrangement
        .sections
        .iter()
        .map(|s| s.start_bar + s.length_bars)
        .max()
        .unwrap_or(0);
    assert!(
        span >= bass.bars,
        "the song's span ({span}) leaves the 808 out"
    );
}

#[test]
fn ac8_the_808_counts_toward_the_songs_span_and_an_empty_one_is_not_a_stem() {
    let song = accompany(&sing(260.0, 126.0, 6), "trap");
    let bass = bass_of(&song, "trap");

    let alone = build_song("qa", TENSE, 55, None, None, Some(bass));
    assert_eq!(
        alone
            .stems
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["808"]
    );
    assert_eq!(
        alone
            .arrangement
            .sections
            .iter()
            .map(|s| s.length_bars)
            .max(),
        Some(bass.bars),
        "the 808 alone does not set the song's span"
    );

    let empty = BassView {
        samples: Vec::new(),
        wave: Vec::new(),
        bars: 0,
        seconds: 0.0,
        ..bass.clone()
    };
    let session = build_song(
        "qa",
        TENSE,
        55,
        Some(&song.voice),
        Some(&song.track),
        Some(&empty),
    );
    assert_eq!(
        session
            .stems
            .iter()
            .map(|s| s.name.as_str())
            .collect::<Vec<_>>(),
        ["voice", "drums"],
        "an empty bass became a stem"
    );
}

#[test]
fn ac8_the_mixdown_contains_the_808() {
    let song = accompany(&sing(260.0, 126.0, 6), "trap");
    let bass = bass_of(&song, "trap");
    let with = build_song(
        "qa",
        TENSE,
        55,
        Some(&song.voice),
        Some(&song.track),
        Some(bass),
    )
    .mixdown()
    .expect("one rate, one length: it mixes");
    assert_close(
        &with.samples,
        &export_mix(&song, true),
        1e-5,
        "voice + drums + 808",
    );
    let without = build_song("qa", TENSE, 55, Some(&song.voice), Some(&song.track), None)
        .mixdown()
        .expect("it mixes");
    assert!(
        with.samples != without.samples,
        "the 808 is not in the mixdown"
    );
}

#[test]
fn ac8_save_and_export_write_the_808() {
    let song = accompany(&sing(260.0, 126.0, 6), "trap");
    let bass = bass_of(&song, "trap");
    let dir = temp_dir("save");
    let json = save_session(
        &dir,
        "bass",
        TENSE,
        55,
        Some(&song.voice),
        Some(&song.track),
        Some(bass),
    )
    .expect("saves");
    let loaded = Song::load(&json).expect("loads");
    let wav = export_master(
        &dir,
        "bass",
        TENSE,
        55,
        Some(&song.voice),
        Some(&song.track),
        Some(bass),
    )
    .expect("exports");
    let bytes = std::fs::read(&wav).expect("the master is written");
    let stems = loaded
        .export_stems(dir.join("stems"))
        .expect("stems export");
    std::fs::remove_dir_all(&dir).ok();

    let names: Vec<(&str, StemKind)> = loaded
        .stems
        .iter()
        .map(|s| (s.name.as_str(), s.kind))
        .collect();
    assert_eq!(
        names,
        [
            ("voice", StemKind::Other),
            ("drums", StemKind::Beat),
            ("808", StemKind::Other)
        ],
        "the saved session"
    );
    assert_close(
        &loaded.stems[2].samples,
        &bass.samples,
        1e-6,
        "the saved 808 stem",
    );
    let files: Vec<String> = stems
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .collect();
    assert_eq!(files, ["00-voice.wav", "01-drums.wav", "02-808.wav"]);

    // The master is the mix with the 808 in it, as 16-bit PCM.
    let len = song.voice.samples.len();
    assert_eq!(
        bytes.len(),
        44 + 2 * len,
        "a mono 16-bit master as long as the stems"
    );
    let mix = export_mix(&song, true);
    let (pcm, rest) = bytes[44..].as_chunks::<2>();
    assert!(rest.is_empty(), "harness: whole 16-bit samples");
    for (i, pair) in pcm.iter().enumerate() {
        let written = i16::from_le_bytes(*pair);
        let want = (mix[i].clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
        assert!(
            (i32::from(written) - i32::from(want)).abs() <= 1,
            "the master's sample {i} is {written}, the mix with the 808 is {want}"
        );
    }
}

#[test]
fn ac8_what_the_studio_plays_is_what_export_writes_over_every_style() {
    // Owner decision: each track at its export level, all scaled down together
    // only when their sum would pass full scale — for every style, so the
    // clipping voice + drums already had under corrido, metal and free ends too.
    let take = sing(260.0, 126.0, 6);
    let mut clipped_before = false;
    for style in style_names() {
        let song = accompany(&take, style);
        let levels = song.levels;
        for (name, gain) in [
            ("voice", levels.voice),
            ("track", levels.track),
            ("bass", levels.bass),
        ] {
            assert!(
                gain.is_finite() && gain > 0.0 && gain <= 1.0,
                "{style}: the {name} level is {gain}"
            );
        }
        let mut played: Vec<f32> = song
            .voice
            .samples
            .iter()
            .zip(&song.track.samples)
            .map(|(v, d)| v * levels.voice + d * levels.track)
            .collect();
        if let Some(bass) = &song.bass {
            for (slot, s) in played.iter_mut().zip(&bass.samples) {
                *slot += s * levels.bass;
            }
        }
        let exported = build_song(
            "qa",
            TENSE,
            55,
            Some(&song.voice),
            Some(&song.track),
            song.bass.as_ref(),
        )
        .mixdown()
        .expect("it mixes");
        assert_close(
            &played,
            &exported.samples,
            1e-5,
            &format!("{style}: what the studio plays against what export writes"),
        );
        assert!(
            peak(&played) <= 1.0 + 1e-6,
            "{style}: playback clips at {}",
            peak(&played)
        );
        let at_unity = song
            .voice
            .samples
            .iter()
            .zip(&song.track.samples)
            .map(|(v, d)| (v + d).abs())
            .fold(0.0f32, f32::max);
        clipped_before |= at_unity > 1.0;
    }
    assert!(
        clipped_before,
        "harness: voice + drums at unity gain never clipped, so nothing was shown to be fixed"
    );
}

#[test]
fn ac8_the_shell_receives_the_bass_and_levels_and_hands_the_bass_back() {
    // The webview holds the result as JSON and sends `bass` back to
    // save_session / export_master.
    let song = accompany(&sing(260.0, 126.0, 6), "trap");
    let bass = bass_of(&song, "trap");
    let json = serde_json::to_value(&song).expect("serializes");
    for key in ["voice", "track", "bass"] {
        assert!(
            json["levels"][key].is_number(),
            "the result's levels have no {key}: {}",
            json["levels"]
        );
    }
    let sent = &json["bass"];
    assert_eq!(sent["voice"], "808");
    for key in ["sampleRate", "bars", "seconds", "rootHz", "wave", "samples"] {
        assert!(
            sent.get(key).is_some(),
            "the bass sent to the shell has no {key}"
        );
    }
    let back: BassView = serde_json::from_value(sent.clone()).expect("the shell's bass reads");
    assert_eq!(back.voice, bass.voice);
    assert_eq!((back.sample_rate, back.bars), (bass.sample_rate, bass.bars));
    assert!(
        back.samples == bass.samples,
        "the samples did not survive the round trip"
    );
    let same = |a: f64, b: f64| (a / b - 1.0).abs() < 1e-12;
    assert!(same(back.root_hz, bass.root_hz) && same(back.seconds, bass.seconds));
    let session = build_song(
        "qa",
        TENSE,
        55,
        Some(&song.voice),
        Some(&song.track),
        Some(&back),
    );
    assert_eq!(session.stems.len(), 3, "the bass handed back is not saved");

    let corrido =
        serde_json::to_value(accompany(&sing(260.0, 126.0, 6), "corrido")).expect("serializes");
    assert!(
        corrido["bass"].is_null(),
        "a style with no bass sends one: {}",
        corrido["bass"]
    );
}
