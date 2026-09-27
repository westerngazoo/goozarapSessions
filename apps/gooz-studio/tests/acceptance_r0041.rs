//! R-0041 AC7 — the studio follows the take, and the demo does not.
//!
//! The library half of R-0041 is tested in `gooz-dsp`; this covers the wiring:
//! a take at its own tempo and key gets a riff at that tempo and key, while the
//! demo stays exactly where it was.

use gooz_studio::{beat_view, build_song, demo_riff, describe_song, riff_from_take};

const SR: u32 = 48_000;
const TENSE: u8 = 30;

/// Plucked tones at `bpm`, each at `hz` — a stand-in for someone singing.
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

fn cents_apart(a: f64, b: f64) -> f64 {
    1200.0 * (a / b).log2().abs()
}

#[test]
fn ac7_a_take_gets_a_riff_in_its_own_key() {
    // 260 Hz, deliberately: it is *not* a simple ratio of Easy Mode's 220, so
    // an unfollowed grid would snap it to 264 (6:5) or 275 (5:4). An earlier
    // version of this test used 330 Hz — which is 3:2 of 220, so it came back
    // as 330 either way and the test proved nothing.
    let view = riff_from_take(&sung(120.0, 260.0, 8), SR, TENSE).expect("a valid take");
    assert!(!view.notes.is_empty(), "nothing was heard");

    let root = view
        .followed_root_hz
        .expect("a sung take has a pitch to follow");
    assert!(
        cents_apart(root, 260.0) < 40.0,
        "followed {root:.1} Hz, not the take's 260"
    );

    let lowest = view
        .notes
        .iter()
        .map(|n| n.hz.expect("a sung note has a pitch"))
        .fold(f64::INFINITY, f64::min);
    assert!(
        cents_apart(lowest, 260.0) < 40.0,
        "the riff's lowest note is {lowest:.1} Hz — not the take's own pitch"
    );
}

#[test]
fn ac7_the_beat_plays_at_the_tempo_the_riff_followed() {
    // The beat goes under the riff. A loop at 92 BPM beneath a riff at 140 is
    // not a session, it is two recordings played at once.
    let followed = beat_view(55, Some(140.0));
    let unfollowed = beat_view(55, None);
    let bar_secs = followed.seconds / f64::from(followed.bars);
    assert!(
        (240.0 / bar_secs - 140.0).abs() < 1.0,
        "the beat played at {:.1} BPM",
        240.0 / bar_secs
    );
    assert_ne!(
        followed.seconds, unfollowed.seconds,
        "following the riff changed nothing about the beat"
    );
}

#[test]
fn ac7_a_saved_session_says_what_the_song_is_actually_in() {
    // A file claiming 92 BPM for a riff rendered at 126 is a file that lies.
    let take = riff_from_take(&sung(126.0, 260.0, 12), SR, TENSE).expect("a valid take");
    let bpm = take.followed_bpm.expect("a plucked take has a pulse");
    let root = take.followed_root_hz.expect("and a pitch");

    let song = build_song("session 001", TENSE, 55, Some(&take), None);
    assert_eq!(
        song.settings.bpm, bpm,
        "the session's tempo is not the riff's"
    );
    assert_eq!(
        song.settings.root_hz, root,
        "the session's key is not the riff's"
    );

    // With nothing followed, Easy Mode's own constants are what gets written.
    let plain = build_song("session 002", TENSE, 55, None, None);
    assert_eq!(plain.settings.bpm, 92.0);
    assert_eq!(plain.settings.root_hz, 220.0);
}

#[test]
fn ac7_a_take_gets_a_riff_at_its_own_tempo() {
    // `seconds / bars` is the bar length, and a bar is four beats: a 140 BPM
    // take must not be laid out on Easy Mode's 92 BPM clock.
    let view = riff_from_take(&sung(140.0, 220.0, 12), SR, TENSE).expect("a valid take");
    assert!(view.bars >= 1 && view.seconds > 0.0);
    let bar_secs = view.seconds / f64::from(view.bars);
    let bpm = 240.0 / bar_secs;
    assert!(
        (bpm - 140.0).abs() < 20.0,
        "the riff was laid out at {bpm:.1} BPM, not the take's 140"
    );
    assert!(
        (bpm - 92.0).abs() > 20.0,
        "the riff is still on Easy Mode's 92 BPM clock"
    );
}

#[test]
fn ac7_the_demo_did_not_move() {
    // Golden, captured before R-0041 landed. The demo is a fixed showcase of
    // Easy Mode's own grid and clock; following would re-tune a reference
    // nobody asked to move (owner decision).
    let view = demo_riff();
    assert_eq!(view.bars, 3, "the demo's bar count moved");
    assert!(
        (view.seconds - 7.826062).abs() < 1e-3,
        "the demo's length moved: {:.6}",
        view.seconds
    );
    let heard: Vec<(u64, u64, i32, f64)> = view
        .notes
        .iter()
        .map(|n| {
            (
                n.num,
                n.den,
                n.octave,
                n.hz.expect("a sung note has a pitch"),
            )
        })
        .collect();
    assert_eq!(
        heard,
        vec![
            (1, 1, 0, 220.0),
            (3, 2, 0, 330.0),
            (5, 4, 0, 275.0),
            (1, 1, 1, 440.0),
        ],
        "the demo is no longer on Easy Mode's 220 Hz grid"
    );
}

// ---------------------------------------------------------------------------
// QA sign-off additions (R-0041, step 7).
//
// The tests above prove the take path follows when the take *said*. AC7 has a
// second half — "and its existing constants when they were not" — and a third
// door, the described song, which follows a prompt and must report nothing.
// Mutation-tested on 39a9257: replacing either fallback constant, making
// `describe_song` claim a follow, or re-quantizing the demo left every test
// green.
// ---------------------------------------------------------------------------

/// `secs` of a steady sine at `hz`, with `before` and `after` seconds of
/// silence around it.
fn tone_between(hz: f64, before: f64, secs: f64, after: f64) -> Vec<f32> {
    let rate = f64::from(SR);
    let mut out = vec![0.0f32; (before * rate) as usize];
    out.extend(
        (0..(secs * rate) as usize)
            .map(|i| 0.8 * (std::f64::consts::TAU * hz * i as f64 / rate).sin() as f32),
    );
    out.extend(vec![0.0f32; (after * rate) as usize]);
    out
}

/// The bar length of a view, as the tempo it was laid out at.
fn laid_out_bpm(view: &gooz_studio::RiffView) -> f64 {
    assert!(view.bars >= 1, "nothing was rendered");
    240.0 / (view.seconds / f64::from(view.bars))
}

#[test]
fn ac7_a_take_with_no_pulse_is_laid_out_on_easy_modes_own_clock() {
    // One held note has a pitch and no pulse. The grid follows the pitch; the
    // clock falls back to Easy Mode's 92 BPM rather than inventing a tempo.
    // (A pure sine, deliberately: see the ignored AC3 tests in gooz-dsp for
    // what a held note with overtones does at 48 kHz.)
    let view =
        riff_from_take(&tone_between(260.0, 0.0, 2.0, 0.0), SR, TENSE).expect("a valid take");
    assert_eq!(view.followed_bpm, None, "one held note reported a tempo");
    assert!(view.followed_root_hz.is_some(), "one held note has a pitch");
    let bpm = laid_out_bpm(&view);
    assert!(
        (bpm - 92.0).abs() < 0.01,
        "laid out at {bpm:.2} BPM, not Easy Mode's 92"
    );
}

#[test]
fn ac7_a_take_that_said_nothing_gets_easy_modes_own_grid_and_clock() {
    // One 0.15 s blip at 260 Hz: one onset is no pulse, and 0.15 s is under
    // SPEC-0041's MIN_VOICED_SECS, so the take says neither. Both constants
    // stand in — so the blip is snapped to 5:4 of 220 Hz (275 Hz) rather than
    // to a 1:1 of its own, and the riff is laid out at 92 BPM.
    let view =
        riff_from_take(&tone_between(260.0, 0.3, 0.15, 0.5), SR, TENSE).expect("a valid take");
    assert_eq!(view.followed_bpm, None, "one blip reported a tempo");
    assert_eq!(view.followed_root_hz, None, "0.15 s reported a key");
    assert!(!view.notes.is_empty(), "nothing was heard");
    for note in &view.notes {
        let hz = note.hz.expect("a sung note has a pitch");
        let on_grid = 220.0 * note.num as f64 / note.den as f64 * 2f64.powi(note.octave);
        assert!(
            (hz / on_grid - 1.0).abs() < 1e-9,
            "{hz:.1} Hz is not on Easy Mode's 220 Hz grid"
        );
    }
    let bpm = laid_out_bpm(&view);
    assert!(
        (bpm - 92.0).abs() < 0.01,
        "laid out at {bpm:.2} BPM, not Easy Mode's 92"
    );
}

#[test]
fn ac7_what_the_view_says_it_followed_is_what_the_riff_was_built_on() {
    // "What was followed is carried in the view, so the UI can say which" —
    // and a saved session copies it — so it has to be the tempo and root the
    // riff was *actually* rendered on, not just near them. The other AC7 tests
    // compare the view's numbers with themselves or allow ±20 BPM: reporting
    // 1% more than the riff was laid out at passed all of them
    // (mutation-tested on 39a9257).
    let view = riff_from_take(&sung(126.0, 260.0, 12), SR, TENSE).expect("a valid take");
    let bpm = view.followed_bpm.expect("a plucked take has a pulse");
    let root = view.followed_root_hz.expect("and a pitch");
    let laid_out = laid_out_bpm(&view);
    assert!(
        (laid_out - bpm).abs() < 0.01,
        "the view says {bpm:.3} BPM; the riff was laid out at {laid_out:.3}"
    );
    assert!(!view.notes.is_empty(), "nothing was heard");
    for note in &view.notes {
        let hz = note.hz.expect("a sung note has a pitch");
        let grid_root = hz / (note.num as f64 / note.den as f64 * 2f64.powi(note.octave));
        assert!(
            (grid_root / root - 1.0).abs() < 1e-9,
            "a {hz:.1} Hz note sits on a grid rooted at {grid_root:.3} Hz, not the reported {root:.3}"
        );
    }
}

#[test]
fn ac7_with_nothing_followed_the_beat_keeps_easy_modes_clock() {
    // `ac7_the_beat_plays_at_the_tempo_the_riff_followed` only checks that
    // following *changes* the beat: with Easy Mode's fallback tempo replaced by
    // 120, the unfollowed beat moved to 120 BPM and every test stayed green
    // (mutation-tested on 39a9257).
    let beat = beat_view(55, None);
    let bar_secs = beat.seconds / f64::from(beat.bars);
    assert!(
        (240.0 / bar_secs - 92.0).abs() < 0.01,
        "the unfollowed beat played at {:.2} BPM",
        240.0 / bar_secs
    );
}

#[test]
fn ac7_the_demo_did_not_move_in_time_either() {
    // `ac7_the_demo_did_not_move` pins the demo's grid, bar count and length,
    // but not *when* its notes sound: re-quantizing the demo to sixteenths
    // instead of eighths moved three of its four notes and still passed it
    // (mutation-tested on 39a9257). Fingerprinted against deaf680, the commit
    // before R-0041, whose demo is bit-identical to this one: the exact sample
    // count, how far each hummed tone was snapped, and the waveform bucket in
    // which each note's attack first crosses half the peak.
    let view = demo_riff();
    assert_eq!(view.samples.len(), 375_651, "the demo's length moved");

    // A thousandth of a cent, not bit-exact: the hum is synthesized with the
    // platform's `sin`, and CI runs on Linux while this was captured on macOS.
    let golden_cents = [23.57734, 15.763845, 18.919415, -7.825668];
    assert_eq!(
        view.notes.len(),
        golden_cents.len(),
        "the demo's notes changed"
    );
    for (note, want) in view.notes.iter().zip(golden_cents) {
        let cents = note.cents.expect("a hummed tone was offset from a pitch");
        assert!(
            (cents - want).abs() < 1e-3,
            "a hummed tone was snapped by {cents:.6} cents, not {want:.6}"
        );
    }

    let peak = view.wave.iter().copied().fold(0.0f32, f32::max);
    let attacks: Vec<usize> = (1..view.wave.len())
        .filter(|&i| view.wave[i] >= 0.5 * peak && view.wave[i - 1] < 0.5 * peak)
        .collect();
    assert_eq!(attacks, [24, 44, 74, 99], "the demo's notes moved in time");
}

#[test]
fn ac7_a_described_song_follows_a_prompt_not_a_take() {
    let described = describe_song("dark slow trap with a heavy kick", 4);
    assert_eq!(described.riff.followed_bpm, None);
    assert_eq!(described.riff.followed_root_hz, None);
}

#[test]
fn ac7_the_beat_survives_any_tempo_it_is_handed() {
    // `beat` is a Tauri command, so this number arrives from the webview. The
    // riff's followed tempo is always inside 60..=180, but nothing holds any
    // other caller to that. On 39a9257, 1e-300 and f64::MIN_POSITIVE overflow
    // `bar_samples * bars` in gooz-synth and panic, and 1e9 renders a
    // two-sample "beat". Tiny-but-finite values such as 1e-3 are deliberately
    // absent: unfixed, they ask the allocator for ~92 GB instead of panicking.
    for bpm in [
        f64::MIN_POSITIVE,
        1e-300,
        1e9,
        1e300,
        0.0,
        -1.0,
        f64::NAN,
        f64::INFINITY,
    ] {
        let beat = beat_view(55, Some(bpm));
        let bar_secs = beat.seconds / f64::from(beat.bars);
        assert!(
            (240.0 / 180.0 - 1e-6..=240.0 / 60.0 + 1e-6).contains(&bar_secs),
            "{bpm} BPM gave a {bar_secs} s bar"
        );
    }
}
