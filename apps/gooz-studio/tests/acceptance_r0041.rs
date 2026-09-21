//! R-0041 AC7 — the studio follows the take, and the demo does not.
//!
//! The library half of R-0041 is tested in `gooz-dsp`; this covers the wiring:
//! a take at its own tempo and key gets a riff at that tempo and key, while the
//! demo stays exactly where it was.

use gooz_studio::{beat_view, build_song, demo_riff, riff_from_take};

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
        .map(|n| n.hz)
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
        .map(|n| (n.num, n.den, n.octave, n.hz))
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
