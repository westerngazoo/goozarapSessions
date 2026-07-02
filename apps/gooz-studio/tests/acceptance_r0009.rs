//! Acceptance tests for R-0009 — beat builder integration (`build_beat`),
//! realized by SPEC-0009.

use gooz_dsp::Tempo;
use gooz_ratio::BeatError;
use gooz_studio::{BeatConfig, BeatVoiceSpec, build_beat};
use gooz_synth::DrumKind;

const SR: u32 = 48_000;

fn tempo() -> Tempo {
    Tempo::new(120.0, 4.0).unwrap()
}

fn bar_samples(tempo: &Tempo) -> usize {
    (tempo.bar_seconds() * f64::from(SR)).round() as usize
}

#[test]
fn ac1_build_default_beat_is_non_empty() {
    let stem = build_beat(&tempo(), SR, &BeatConfig::default()).unwrap();
    assert_eq!(stem.bars, 4);
    assert!(!stem.samples.is_empty());
    assert!(
        stem.samples
            .iter()
            .all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-6)
    );
}

#[test]
fn ac2_stem_is_bar_aligned() {
    let cfg = BeatConfig {
        bars: 2,
        ..BeatConfig::default()
    };
    let stem = build_beat(&tempo(), SR, &cfg).unwrap();
    assert_eq!(stem.samples.len(), 2 * bar_samples(&tempo()));
}

#[test]
fn ac5_invalid_euclidean_returns_beat_error() {
    let cfg = BeatConfig {
        voices: vec![BeatVoiceSpec {
            kind: DrumKind::Kick,
            onsets: 5,
            steps: 4,
            rotate: 0,
            level: 1.0,
        }],
        bars: 1,
    };
    assert_eq!(
        build_beat(&tempo(), SR, &cfg),
        Err(BeatError::TooManyOnsets)
    );
}

#[test]
fn ac5_zero_bars_yields_empty_stem() {
    let cfg = BeatConfig {
        bars: 0,
        ..BeatConfig::default()
    };
    let stem = build_beat(&tempo(), SR, &cfg).unwrap();
    assert_eq!(stem.bars, 0);
    assert!(stem.samples.is_empty());
}

#[test]
fn ac6_build_is_deterministic() {
    let cfg = BeatConfig::default();
    let a = build_beat(&tempo(), SR, &cfg).unwrap();
    let b = build_beat(&tempo(), SR, &cfg).unwrap();
    assert_eq!(a, b);
}
