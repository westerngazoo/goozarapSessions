//! Acceptance tests for R-0015 — ingest & feature extraction, realized by
//! SPEC-0015.

use std::f64::consts::TAU;
use std::path::PathBuf;

use gooz_dsp::Config;
use gooz_model::{FEATURE_FORMAT_VERSION, ModelError, ModelKind, ModelRegistry, extract_features};
use gooz_ratio::PitchGrid;

const SR: u32 = 48_000;

fn temp_root(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("gooz_feat_{}_{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn grid() -> PitchGrid {
    PitchGrid::harmonic(220.0, 9).unwrap()
}

/// A reference "hum": two on-grid tones (220 Hz then 330 Hz) with a gap, each
/// faded, so the analyzer finds two notes with clear pitches.
fn reference() -> Vec<f32> {
    let sr = f64::from(SR);
    let tone_len = (0.4 * sr) as usize;
    let gap_len = (0.12 * sr) as usize;
    let mut out = Vec::new();
    for freq in [220.0_f64, 330.0] {
        for n in 0..tone_len {
            let t = n as f64 / sr;
            let fade = (std::f64::consts::PI * n as f64 / tone_len as f64).sin();
            out.push((0.6 * fade * (TAU * freq * t).sin()) as f32);
        }
        out.resize(out.len() + gap_len, 0.0);
    }
    out
}

// AC1/AC2 — extract a profile with finite overall stats.
#[test]
fn ac1_ac2_profile_has_finite_overall_stats() {
    let p = extract_features(&reference(), SR, &grid(), &Config::default()).expect("extracts");
    assert_eq!(p.format_version, FEATURE_FORMAT_VERSION);
    assert_eq!(p.sample_rate, SR);
    assert!(p.duration_secs > 0.0 && p.duration_secs.is_finite());
    assert!(p.rms > 0.0 && p.rms.is_finite());
    assert!(p.brightness.is_finite() && (0.0..=1.0).contains(&p.brightness));
}

// AC3 — rhythm profile.
#[test]
fn ac3_reports_tempo_and_onset_density() {
    let p = extract_features(&reference(), SR, &grid(), &Config::default()).unwrap();
    assert!(p.tempo_bpm.is_finite() && p.tempo_bpm >= 0.0);
    assert!(p.onset_density.is_finite() && p.onset_density >= 0.0);
}

// AC4 — ratio histogram: non-negative weights summing to ~1.
#[test]
fn ac4_ratio_histogram_is_a_normalized_distribution() {
    let p = extract_features(&reference(), SR, &grid(), &Config::default()).unwrap();
    assert!(
        !p.ratios.is_empty(),
        "two clear tones should populate the histogram"
    );
    assert!(
        p.ratios
            .iter()
            .all(|r| r.weight >= 0.0 && r.weight.is_finite())
    );
    let sum: f64 = p.ratios.iter().map(|r| r.weight).sum();
    assert!((sum - 1.0).abs() < 1e-9, "weights sum to 1, got {sum}");
    // 220 Hz is the grid root → a 1:1 degree should be present.
    assert!(p.ratios.iter().any(|r| r.num == 1 && r.den == 1));
}

// AC5 — persist to the model and read back losslessly.
#[test]
fn ac5_write_and_read_features_round_trip() {
    let root = temp_root("io");
    let reg = ModelRegistry::open(&root).unwrap();
    reg.create("ref song", ModelKind::Timbre).unwrap();
    let p = extract_features(&reference(), SR, &grid(), &Config::default()).unwrap();

    reg.write_features("ref-song", &p).expect("writes");
    let back = reg.read_features("ref-song").expect("reads");
    assert_eq!(back, p);
    // Recorded in the manifest.
    assert!(
        reg.get("ref-song")
            .unwrap()
            .files
            .iter()
            .any(|f| f == "features.json")
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn ac5_read_features_missing_is_not_found() {
    let root = temp_root("missing");
    let reg = ModelRegistry::open(&root).unwrap();
    reg.create("empty", ModelKind::Timbre).unwrap();
    assert!(matches!(
        reg.read_features("empty"),
        Err(ModelError::NotFound(_))
    ));
    // Writing features for a non-existent model is NotFound too.
    let p = extract_features(&reference(), SR, &grid(), &Config::default()).unwrap();
    assert!(matches!(
        reg.write_features("nope", &p),
        Err(ModelError::NotFound(_))
    ));
    let _ = std::fs::remove_dir_all(&root);
}

// AC6 — typed error on bad input + determinism.
#[test]
fn ac6_empty_input_is_typed_extract_error() {
    let err = extract_features(&[], SR, &grid(), &Config::default()).unwrap_err();
    assert!(matches!(err, ModelError::Extract(_)));
}

#[test]
fn ac6_extraction_is_deterministic() {
    let a = extract_features(&reference(), SR, &grid(), &Config::default()).unwrap();
    let b = extract_features(&reference(), SR, &grid(), &Config::default()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn ac6_silence_yields_empty_ratio_histogram() {
    // A finite, long-enough silent signal analyzes fine but finds no pitch.
    let silence = vec![0.0f32; SR as usize];
    let p = extract_features(&silence, SR, &grid(), &Config::default()).unwrap();
    assert!(
        p.ratios.is_empty(),
        "silence has no notes → empty histogram"
    );
    assert_eq!(p.rms, 0.0);
}
