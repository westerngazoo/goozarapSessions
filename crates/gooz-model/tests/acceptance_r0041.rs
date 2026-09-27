//! R-0041's footprint on R-0015 — `estimate_bpm` moved to `gooz-dsp` and now
//! answers `Option`; `extract_features` turns "no pulse" into the feature
//! format's `0.0` sentinel at its own boundary (SPEC-0041, "Tempo").
//!
//! The unit test that pinned this lived beside the old estimator and was
//! deleted with it, and R-0015's own test only asks for a finite, non-negative
//! tempo — so replacing the sentinel with 92 left every test green
//! (mutation-tested on 39a9257).

use std::f64::consts::TAU;

use gooz_dsp::Config;
use gooz_model::extract_features;
use gooz_ratio::PitchGrid;

const SR: u32 = 48_000;

fn grid() -> PitchGrid {
    PitchGrid::harmonic(220.0, 9).expect("a valid grid")
}

/// Plucked 220 Hz notes starting at `times`.
fn plucks(times: &[f64], total_secs: f64) -> Vec<f32> {
    let rate = f64::from(SR);
    let mut out = vec![0.0f32; (total_secs * rate) as usize];
    let len = (0.35 * rate) as usize;
    for &start_secs in times {
        let start = (start_secs * rate) as usize;
        for i in 0..len.min(out.len().saturating_sub(start)) {
            let t = i as f64 / rate;
            let env = (-6.0 * i as f64 / len as f64).exp();
            out[start + i] += (0.8 * env * (TAU * 220.0 * t).sin()) as f32;
        }
    }
    out
}

#[test]
fn a_reference_with_no_pulse_is_written_with_the_formats_zero_tempo() {
    let held: Vec<f32> = (0..2 * SR as usize)
        .map(|i| 0.8 * (TAU * 260.0 * i as f64 / f64::from(SR)).sin() as f32)
        .collect();
    let profile = extract_features(&held, SR, &grid(), &Config::default()).expect("extracts");
    assert_eq!(profile.tempo_bpm, 0.0, "one held note was given a tempo");
}

#[test]
fn a_reference_with_a_pulse_is_written_with_its_tempo() {
    let times: Vec<f64> = (0..8).map(|beat| 0.2 + f64::from(beat) * 0.5).collect();
    let profile =
        extract_features(&plucks(&times, 4.6), SR, &grid(), &Config::default()).expect("extracts");
    assert!(
        (profile.tempo_bpm - 120.0).abs() < 6.0,
        "a 120 BPM reference was written as {:.1}",
        profile.tempo_bpm
    );
}
