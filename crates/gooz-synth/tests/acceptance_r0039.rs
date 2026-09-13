//! R-0039 acceptance tests — any recorded sound becomes an instrument.
//!
//! The shifts are verified by **measuring the rendered audio with YIN**, never
//! by reading back the ratio that was asked for: if the recording does not
//! actually sound a fifth higher at degree `3:2`, these fail.

use gooz_dsp::{Config, pitch_track};
use gooz_synth::{Distortion, QuantizedNote, Ratio, RenderConfig, Sampler, render_sampled_notes};

const SR: u32 = 48_000;

/// A recording: one second of a steady sine, standing in for whatever the user
/// hit, hummed, or knocked.
fn recording(hz: f64) -> Vec<f32> {
    (0..SR as usize)
        .map(|i| 0.8 * (std::f64::consts::TAU * hz * i as f64 / f64::from(SR)).sin() as f32)
        .collect()
}

/// A recording with no pitch in it at all — a knock, a click, a door.
fn noise_burst() -> Vec<f32> {
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    (0..SR as usize / 4)
        .map(|i| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let white = (state >> 40) as f32 / 8_388_608.0 - 1.0;
            // A short percussive envelope so it reads as a hit, not as hiss.
            let decay = (-12.0 * i as f32 / (SR as f32 / 4.0)).exp();
            white * decay * 0.9
        })
        .collect()
}

fn note(degree: Ratio, octave: i32, onset_secs: f64) -> QuantizedNote {
    QuantizedNote {
        degree,
        octave,
        // Deliberately wrong and deliberately ignored: under R-0039 the
        // recording is the root, so the sampler must not read this.
        freq_hz: 1.0,
        cents_offset: 0.0,
        onset_step: 0,
        onset_secs,
        duration_secs: 0.5,
    }
}

/// Render with the FX out of the way, so a test measures the sampler and not
/// the distortion curve.
fn clean() -> RenderConfig {
    RenderConfig {
        distortion: Distortion::None,
        ..RenderConfig::default()
    }
}

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

fn cents_apart(a: f64, b: f64) -> f64 {
    1200.0 * (a / b).log2().abs()
}

fn render_one(sampler: &Sampler, degree: Ratio, octave: i32) -> Vec<f32> {
    render_sampled_notes(sampler, &[note(degree, octave, 0.0)], SR, &clean())
}

#[test]
fn ac1_the_grid_plays_the_recording_at_the_asked_for_ratio() {
    let sampler = Sampler::new(recording(220.0));
    let root = measured_hz(&render_one(&sampler, Ratio::UNISON, 0));
    for (num, den) in [(3, 2), (5, 4), (2, 1)] {
        let ratio = Ratio::new(num, den).expect("ratio");
        let measured = measured_hz(&render_one(&sampler, ratio, 0));
        let expected = root * num as f64 / den as f64;
        assert!(
            cents_apart(measured, expected) < 20.0,
            "degree {num}:{den} should sound at {expected:.1} Hz, measured {measured:.1} Hz"
        );
    }
}

#[test]
fn ac2_at_unison_the_recording_is_placed_unshifted() {
    // Not "close to" the source — the source times one gain factor. A resampled
    // approximation would drift sample by sample; a placement cannot.
    let source = recording(220.0);
    let rendered = render_one(&Sampler::new(source.clone()), Ratio::UNISON, 0);
    assert_eq!(
        rendered.len(),
        source.len(),
        "unison must not change length"
    );

    let loud: Vec<usize> = (0..source.len())
        .filter(|&i| source[i].abs() > 0.1)
        .collect();
    let gain = rendered[loud[0]] / source[loud[0]];
    for &i in &loud {
        let here = rendered[i] / source[i];
        assert!(
            (here - gain).abs() < 1e-4,
            "sample {i}: gain {here} drifted from {gain} — this is a resample, not a placement"
        );
    }
}

#[test]
fn ac3_a_pitchless_recording_is_still_an_instrument() {
    // A knock has no fundamental to detect. It must still play across the whole
    // grid — nothing in this path may require a pitch.
    let sampler = Sampler::new(noise_burst());
    let degrees = [(1, 1), (9, 8), (5, 4), (4, 3), (3, 2), (5, 3), (15, 8)];
    let notes: Vec<QuantizedNote> = degrees
        .iter()
        .enumerate()
        .map(|(i, &(n, d))| note(Ratio::new(n, d).expect("ratio"), 0, i as f64 * 0.25))
        .collect();
    let out = render_sampled_notes(&sampler, &notes, SR, &clean());
    assert!(!out.is_empty(), "a knock must produce audio");
    assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-6));
}

#[test]
fn ac4_octaves_are_ratio_arithmetic() {
    let sampler = Sampler::new(recording(220.0));
    let root = measured_hz(&render_one(&sampler, Ratio::UNISON, 0));
    let up = measured_hz(&render_one(&sampler, Ratio::UNISON, 1));
    let down = measured_hz(&render_one(&sampler, Ratio::UNISON, -1));
    assert!(cents_apart(up, root * 2.0) < 20.0, "octave up: {up:.1} Hz");
    assert!(
        cents_apart(down, root / 2.0) < 20.0,
        "octave down: {down:.1} Hz"
    );
}

#[test]
fn ac4_an_octave_too_extreme_to_form_is_skipped_not_a_panic() {
    // 64 octaves overflows the ratio long before it overflows the ear. The note
    // drops out; the rest of the song still renders.
    let sampler = Sampler::new(recording(220.0));
    let notes = vec![note(Ratio::UNISON, 64, 0.0), note(Ratio::UNISON, 0, 0.0)];
    let out = render_sampled_notes(&sampler, &notes, SR, &clean());
    assert!(!out.is_empty(), "the renderable note must still sound");
    assert!(out.iter().all(|s| s.is_finite()));
}

#[test]
fn ac5_nothing_to_play_is_silence_not_an_error() {
    let sampler = Sampler::new(recording(220.0));
    let one = [note(Ratio::UNISON, 0, 0.0)];
    assert!(render_sampled_notes(&Sampler::new(Vec::new()), &one, SR, &clean()).is_empty());
    assert!(render_sampled_notes(&sampler, &[], SR, &clean()).is_empty());
    assert!(render_sampled_notes(&sampler, &one, 0, &clean()).is_empty());
}

#[test]
fn ac5_a_full_scale_recording_stays_inside_the_unit_interval() {
    let square: Vec<f32> = (0..SR as usize)
        .map(|i| if (i / 100) % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let sampler = Sampler::new(square);
    let notes: Vec<QuantizedNote> = (0..8)
        .map(|i| note(Ratio::new(3, 2).expect("ratio"), 0, i as f64 * 0.1))
        .collect();
    for cfg in [clean(), RenderConfig::default()] {
        let out = render_sampled_notes(&sampler, &notes, SR, &cfg);
        assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-6));
    }
}

#[test]
fn ac6_the_same_recording_and_notes_render_identically() {
    let sampler = Sampler::new(recording(220.0));
    let notes = [note(Ratio::new(5, 4).expect("ratio"), 0, 0.0)];
    let first = render_sampled_notes(&sampler, &notes, SR, &clean());
    let second = render_sampled_notes(&sampler, &notes, SR, &clean());
    assert_eq!(first, second);
}

#[test]
fn the_notes_frequency_is_ignored_the_recording_is_the_root() {
    // Pins R-0039's central decision. Two notes at the same degree whose
    // `freq_hz` disagree wildly must render the same audio — the sampler tunes
    // to the recording, never to an absolute frequency.
    let sampler = Sampler::new(recording(220.0));
    let fifth = Ratio::new(3, 2).expect("ratio");
    let mut quiet_lie = note(fifth, 0, 0.0);
    quiet_lie.freq_hz = 40.0;
    let mut loud_lie = note(fifth, 0, 0.0);
    loud_lie.freq_hz = 12_000.0;
    assert_eq!(
        render_sampled_notes(&sampler, &[quiet_lie], SR, &clean()),
        render_sampled_notes(&sampler, &[loud_lie], SR, &clean())
    );
}

#[test]
fn a_note_lands_where_its_onset_says() {
    // Without this, a renderer that stacked every note at time zero would pass
    // every other test in this file.
    let source = recording(220.0);
    let sampler = Sampler::new(source.clone());
    let half = SR as usize / 2;
    let late = render_sampled_notes(&sampler, &[note(Ratio::UNISON, 0, 0.5)], SR, &clean());
    assert_eq!(
        late.len(),
        half + source.len(),
        "the note did not start at 0.5 s"
    );
    assert!(
        late[..half].iter().all(|sample| *sample == 0.0),
        "audio sounded before the note's onset"
    );
}

#[test]
fn an_onset_past_the_end_of_any_song_is_skipped_not_a_panic() {
    // `onset_secs` is caller data. `1e18 · sample_rate` saturates a `usize`
    // cast, and resizing to that aborts the process.
    let sampler = Sampler::new(recording(220.0));
    let notes = vec![note(Ratio::UNISON, 0, 1e18), note(Ratio::UNISON, 0, 0.0)];
    let out = render_sampled_notes(&sampler, &notes, SR, &clean());
    assert_eq!(
        out.len(),
        recording(220.0).len(),
        "only the real note should sound"
    );
}
