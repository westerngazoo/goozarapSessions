//! R-0039 acceptance tests — any recorded sound becomes an instrument.
//!
//! The shifts are verified by **measuring the rendered audio with YIN**, never
//! by reading back the ratio that was asked for: if the recording does not
//! actually sound a fifth higher at degree `3:2`, these fail.

use gooz_dsp::{Config, pitch_track};
use gooz_ratio::PitchGrid;
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

/// A sampler over a known-finite recording.
fn sampler(recording: Vec<f32>) -> Sampler {
    Sampler::new(recording).expect("the test recordings are finite")
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
        distortion: Distortion::Bypass,
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
    let sampler = sampler(recording(220.0));
    let root = measured_hz(&render_one(&sampler, Ratio::UNISON, 0));
    for (num, den) in [(3, 2), (5, 4), (15, 8)] {
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
    let rendered = render_one(&sampler(source.clone()), Ratio::UNISON, 0);
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
    let sampler = sampler(noise_burst());
    let degrees = [(1, 1), (9, 8), (5, 4), (4, 3), (3, 2), (5, 3), (15, 8)];
    let notes: Vec<QuantizedNote> = degrees
        .iter()
        .enumerate()
        .map(|(i, &(n, d))| note(Ratio::new(n, d).expect("ratio"), 0, i as f64 * 0.25))
        .collect();
    let out = render_sampled_notes(&sampler, &notes, SR, &clean());
    assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-6));
    // Every degree must actually sound. `!out.is_empty()` would pass with six
    // of the seven notes silently dropped — and silent dropping is exactly this
    // renderer's failure mode.
    let window = SR as usize / 20;
    for (i, (num, den)) in degrees.iter().enumerate() {
        let onset = (i as f64 * 0.25 * f64::from(SR)) as usize;
        let energy: f32 = out[onset..onset + window].iter().map(|s| s * s).sum();
        assert!(
            energy > 1e-6,
            "degree {num}:{den} never sounded — {energy:e} energy at its onset"
        );
    }
}

#[test]
fn ac4_octaves_are_ratio_arithmetic() {
    let sampler = sampler(recording(220.0));
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
    let source = recording(220.0);
    let sampler = sampler(source.clone());
    // The bad note sits a second later than the good one: if it rendered
    // instead of being skipped, the mix would be twice as long. Both at onset
    // zero would prove nothing — `normalize_peak` erases a doubled mix.
    let notes = vec![note(Ratio::UNISON, 64, 1.0), note(Ratio::UNISON, 0, 0.0)];
    let out = render_sampled_notes(&sampler, &notes, SR, &clean());
    assert_eq!(
        out.len(),
        source.len(),
        "the overflowing note was not skipped"
    );
    assert!(out.iter().all(|s| s.is_finite()));
}

#[test]
fn ac5_nothing_to_play_is_silence_not_an_error() {
    let loaded = sampler(recording(220.0));
    let one = [note(Ratio::UNISON, 0, 0.0)];
    assert!(render_sampled_notes(&sampler(Vec::new()), &one, SR, &clean()).is_empty());
    assert!(render_sampled_notes(&loaded, &[], SR, &clean()).is_empty());
    assert!(render_sampled_notes(&loaded, &one, 0, &clean()).is_empty());
}

#[test]
fn ac5_a_full_scale_recording_stays_inside_the_unit_interval() {
    let square: Vec<f32> = (0..SR as usize)
        .map(|i| if (i / 100) % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let sampler = sampler(square);
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
    let sampler = sampler(recording(220.0));
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
    let sampler = sampler(recording(220.0));
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
    let sampler = sampler(source.clone());
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
    let sampler = sampler(recording(220.0));
    let notes = vec![note(Ratio::UNISON, 0, 1e18), note(Ratio::UNISON, 0, 0.0)];
    let out = render_sampled_notes(&sampler, &notes, SR, &clean());
    assert_eq!(
        out.len(),
        recording(220.0).len(),
        "only the real note should sound"
    );
}

#[test]
fn the_instrument_says_where_it_lives_not_the_songs_pitch_grid() {
    // A `QuantizedNote`'s octave counts octaves above the *pitch grid's* root,
    // and that root is a per-song setting the user can change. Without
    // `root_octave` the sampler would silently mean "the grid's root pitch is
    // the recording's pitch", and re-rooting a song would transpose every
    // sampled part. Both halves are pinned: the hazard, and the fix.
    let source = recording(220.0);
    let (mut naive, mut rooted) = (Vec::new(), Vec::new());
    for root_hz in [440.0, 55.0] {
        let grid = PitchGrid::harmonic(root_hz, 9).expect("grid");
        let snapped = grid.snap(440.0).expect("440 Hz snaps");
        let mut played = note(snapped.degree, snapped.octave, 0.0);
        played.freq_hz = snapped.hz;
        naive.push(render_sampled_notes(&sampler(source.clone()), &[played], SR, &clean()).len());
        let instrument = sampler(source.clone()).rooted_at_octave(snapped.octave);
        rooted.push(render_sampled_notes(&instrument, &[played], SR, &clean()));
    }
    assert_ne!(
        naive[0], naive[1],
        "the hazard is gone — this test no longer proves anything"
    );
    assert_eq!(
        rooted[0], rooted[1],
        "the song's pitch grid transposed the instrument"
    );
}

#[test]
fn a_recording_with_a_nan_is_refused_when_it_is_still_fixable() {
    // One NaN makes every shift of the recording fail, which would silence the
    // whole part with no indication of why. It is rejected at the point where
    // the user could simply record again.
    let mut broken = recording(220.0);
    broken[12_345] = f32::NAN;
    assert!(Sampler::new(broken).is_err());
    assert!(Sampler::new(vec![0.1, f32::INFINITY]).is_err());
    assert!(
        Sampler::new(Vec::new()).is_ok(),
        "an empty instrument is silence, not an error"
    );
}
