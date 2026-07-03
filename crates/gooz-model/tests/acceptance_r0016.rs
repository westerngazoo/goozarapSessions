//! Acceptance tests for R-0016 — on-device DDSP timbre training, realized by
//! SPEC-0016.

use std::f64::consts::TAU;
use std::path::PathBuf;

use gooz_model::{
    ModelError, ModelKind, ModelRegistry, TrainConfig, extract_timbre_target, train_timbre,
};

const SR: u32 = 48_000;

fn temp_root(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("gooz_timbre_{}_{}", std::process::id(), tag));
    let _ = std::fs::remove_dir_all(&p);
    p
}

/// A tone at `f0` with a few decaying harmonics — a signal with real timbre.
fn harmonic_tone(f0: f64, amps: &[f32]) -> Vec<f32> {
    let n = (0.5 * f64::from(SR)) as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / f64::from(SR);
            let mut s = 0.0f64;
            for (h, &a) in amps.iter().enumerate() {
                s += f64::from(a) * (TAU * f0 * (h + 1) as f64 * t).sin();
            }
            (0.5 * s) as f32
        })
        .collect()
}

// AC1 — extract a normalized target; uniform when no pitch.
#[test]
fn ac1_extract_target_is_normalized() {
    let tone = harmonic_tone(220.0, &[1.0, 0.5, 0.25, 0.12]);
    let target = extract_timbre_target(&tone, SR, 8);
    assert_eq!(target.len(), 8);
    assert!((target.iter().sum::<f32>() - 1.0).abs() < 1e-5);
    assert!(target.iter().all(|&x| x >= 0.0 && x.is_finite()));
    // The fundamental should carry the most energy.
    let max_idx = target
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap()
        .0;
    assert_eq!(max_idx, 0, "fundamental dominates the harmonic profile");
}

#[test]
fn ac1_silence_yields_uniform_target() {
    let target = extract_timbre_target(&vec![0.0f32; SR as usize], SR, 4);
    assert_eq!(target, vec![0.25; 4]);
}

// AC2/AC3 — trains on CPU with per-epoch progress; loss decreases.
#[test]
fn ac2_ac3_training_reports_decreasing_loss() {
    let target = vec![0.5, 0.25, 0.15, 0.10];
    let cfg = TrainConfig {
        n_harmonics: 4,
        epochs: 150,
        ..TrainConfig::default()
    };
    let mut seen = Vec::new();
    let (_decoder, history) =
        train_timbre(&target, &cfg, &mut |p| seen.push(p.epoch)).expect("trains");
    assert_eq!(history.len(), cfg.epochs);
    assert_eq!(seen.len(), cfg.epochs, "callback fired once per epoch");
    assert!(
        history.last().unwrap().loss < history.first().unwrap().loss,
        "loss should decrease: {} -> {}",
        history.first().unwrap().loss,
        history.last().unwrap().loss
    );
}

// AC4 — trained decoder approximates the target.
#[test]
fn ac4_trained_harmonics_approximate_target() {
    let target = vec![0.5, 0.25, 0.15, 0.10];
    let cfg = TrainConfig {
        n_harmonics: 4,
        epochs: 600,
        lr: 0.05,
        ..TrainConfig::default()
    };
    let (decoder, _) = train_timbre(&target, &cfg, &mut |_| {}).unwrap();
    let got = decoder.harmonics().unwrap();
    let err: f32 = got.iter().zip(&target).map(|(a, b)| (a - b).abs()).sum();
    assert!(
        err < 0.1,
        "learned {got:?} should approximate {target:?} (L1 {err})"
    );
}

// AC5 — save & reload reproduce the harmonics.
#[test]
fn ac5_save_and_load_reproduce_harmonics() {
    let root = temp_root("io");
    let reg = ModelRegistry::open(&root).unwrap();
    reg.create("voice", ModelKind::Timbre).unwrap();
    let cfg = TrainConfig {
        n_harmonics: 4,
        epochs: 100,
        ..TrainConfig::default()
    };
    let (decoder, _) = train_timbre(&[0.4, 0.3, 0.2, 0.1], &cfg, &mut |_| {}).unwrap();
    let before = decoder.harmonics().unwrap();

    reg.save_timbre("voice", &decoder).expect("saves");
    assert!(
        reg.get("voice")
            .unwrap()
            .files
            .iter()
            .any(|f| f == "timbre.safetensors"),
        "weights recorded in the manifest"
    );
    let loaded = reg.load_timbre("voice", &cfg).expect("loads");
    let after = loaded.harmonics().unwrap();
    for (a, b) in before.iter().zip(&after) {
        assert!((a - b).abs() < 1e-6, "reloaded harmonics match: {a} vs {b}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

// AC6 — deterministic with a fixed seed.
#[test]
fn ac6_training_is_deterministic() {
    let target = vec![0.5, 0.25, 0.15, 0.10];
    let cfg = TrainConfig {
        n_harmonics: 4,
        epochs: 80,
        seed: 42,
        ..TrainConfig::default()
    };
    let (a, _) = train_timbre(&target, &cfg, &mut |_| {}).unwrap();
    let (b, _) = train_timbre(&target, &cfg, &mut |_| {}).unwrap();
    assert_eq!(a.harmonics().unwrap(), b.harmonics().unwrap());
}

// AC7 — typed errors.
#[test]
fn ac7_target_length_mismatch_is_typed_error() {
    let cfg = TrainConfig {
        n_harmonics: 8,
        ..TrainConfig::default()
    };
    assert!(matches!(
        train_timbre(&[0.5, 0.5], &cfg, &mut |_| {}),
        Err(ModelError::Train(_))
    ));
}

#[test]
fn ac7_load_timbre_missing_is_not_found() {
    let root = temp_root("missing");
    let reg = ModelRegistry::open(&root).unwrap();
    reg.create("untrained", ModelKind::Timbre).unwrap();
    assert!(matches!(
        reg.load_timbre("untrained", &TrainConfig::default()),
        Err(ModelError::NotFound(_))
    ));
    let _ = std::fs::remove_dir_all(&root);
}
