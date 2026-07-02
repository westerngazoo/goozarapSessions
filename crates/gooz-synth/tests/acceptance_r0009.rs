//! Acceptance tests for R-0009: Beat builder.

use gooz_ratio::Tempo;
use gooz_synth::{BeatVoice, DrumVoiceConfig, build_beat};

#[test]
fn test_ac1_ac3_euclidean_placement_and_controls() {
    let tempo = Tempo::new(120.0, 4.0).unwrap();
    let voices = vec![
        DrumVoiceConfig {
            voice: BeatVoice::Kick,
            k: 2,
            n: 4,
            rotation: 0,
        },
        DrumVoiceConfig {
            voice: BeatVoice::Hat,
            k: 3,
            n: 8,
            rotation: 1, // AC3: shifted
        },
        DrumVoiceConfig {
            voice: BeatVoice::Snare,
            k: 0, // AC1: silent voice
            n: 4,
            rotation: 0,
        },
    ];

    let outcome = build_beat(&voices, &tempo, 48_000).unwrap();

    // AC5: Returns mixed stem and per-voice patterns
    assert_eq!(outcome.patterns.len(), 3);

    // Kick: E(2, 4) -> onsets at 0, 2
    assert_eq!(outcome.patterns[0].len(), 4);
    assert_eq!(outcome.patterns[0].onset_count(), 2);
    assert_eq!(outcome.patterns[0].onsets(), vec![0, 2]);

    // Hat: E(3, 8) rotated by +1 -> E(3, 8) = [1, 0, 0, 1, 0, 0, 1, 0] -> onsets at 0, 3, 6. Shifted by 1 -> 1, 4, 7
    assert_eq!(outcome.patterns[1].len(), 8);
    assert_eq!(outcome.patterns[1].onset_count(), 3);
    assert_eq!(outcome.patterns[1].onsets(), vec![1, 4, 7]);

    // Snare: E(0, 4) -> silent
    assert_eq!(outcome.patterns[2].len(), 4);
    assert_eq!(outcome.patterns[2].onset_count(), 0);
    assert_eq!(outcome.patterns[2].onsets(), vec![]);

    // AC4: Loopable (bar-aligned)
    assert_eq!(outcome.bars, 1);
    assert_eq!(
        outcome.samples.len(),
        (tempo.bar_seconds() * 48_000.0).round() as usize
    );
}

#[test]
fn test_ac4_empty_stem() {
    let tempo = Tempo::new(120.0, 4.0).unwrap();
    let voices = vec![DrumVoiceConfig {
        voice: BeatVoice::Kick,
        k: 0,
        n: 4,
        rotation: 0,
    }];

    let outcome = build_beat(&voices, &tempo, 48_000).unwrap();

    // AC4: All-silent voice set yields empty stem with bars == 0
    assert_eq!(outcome.bars, 0);
    assert!(outcome.samples.is_empty());
}

#[test]
fn test_ac2_ac6_deterministic_synthesis() {
    let tempo = Tempo::new(100.0, 4.0).unwrap();
    let voices = vec![
        DrumVoiceConfig {
            voice: BeatVoice::Kick,
            k: 1,
            n: 4,
            rotation: 0,
        },
        DrumVoiceConfig {
            voice: BeatVoice::Snare,
            k: 1,
            n: 4,
            rotation: 1,
        },
        DrumVoiceConfig {
            voice: BeatVoice::Hat,
            k: 4,
            n: 16,
            rotation: 0,
        },
    ];

    let outcome1 = build_beat(&voices, &tempo, 44_100).unwrap();
    let outcome2 = build_beat(&voices, &tempo, 44_100).unwrap();

    // AC6: Deterministic
    assert_eq!(outcome1.samples, outcome2.samples);
    assert_eq!(outcome1.patterns, outcome2.patterns);

    // Ensure we actually rendered sound
    assert!(outcome1.samples.iter().any(|&s| s != 0.0));
}

#[test]
fn test_ac3_rotation_preserves_hit_count() {
    let tempo = Tempo::new(120.0, 4.0).unwrap();
    let base = DrumVoiceConfig {
        voice: BeatVoice::Hat,
        k: 3,
        n: 8,
        rotation: 0,
    };
    let shifted = DrumVoiceConfig {
        rotation: 2,
        ..base.clone()
    };

    let base_out = build_beat(&[base], &tempo, 48_000).unwrap();
    let shifted_out = build_beat(&[shifted], &tempo, 48_000).unwrap();

    assert_eq!(
        base_out.patterns[0].onset_count(),
        shifted_out.patterns[0].onset_count()
    );
    assert_ne!(
        base_out.patterns[0].onsets(),
        shifted_out.patterns[0].onsets()
    );
}

#[test]
fn test_ac1_k_equals_n_fires_every_step() {
    let tempo = Tempo::new(120.0, 4.0).unwrap();
    let voices = vec![DrumVoiceConfig {
        voice: BeatVoice::Hat,
        k: 4,
        n: 4,
        rotation: 0,
    }];

    let outcome = build_beat(&voices, &tempo, 48_000).unwrap();
    assert_eq!(outcome.patterns[0].onsets(), vec![0, 1, 2, 3]);
}

#[test]
fn test_ac4_decay_tail_wraps_at_loop_boundary() {
    // 240 BPM → 1 s bar. Kick on the last step (7/8) starts at 0.875 s; its
    // 0.5 s tail crosses the bar line and wraps into the first 0.375 s.
    let tempo = Tempo::new(240.0, 4.0).unwrap();
    let voices = vec![DrumVoiceConfig {
        voice: BeatVoice::Kick,
        k: 1,
        n: 8,
        rotation: 7,
    }];

    let outcome = build_beat(&voices, &tempo, 48_000).unwrap();
    assert_eq!(outcome.patterns[0].onsets(), vec![7]);
    assert!(
        outcome.samples[0].abs() > 1e-6,
        "kick tail wrapped to loop start"
    );
}

#[test]
fn test_ac7_bounded_clean_audio() {
    let tempo = Tempo::new(160.0, 4.0).unwrap();
    let voices = vec![
        DrumVoiceConfig {
            voice: BeatVoice::Kick,
            k: 4,
            n: 4,
            rotation: 0,
        },
        DrumVoiceConfig {
            voice: BeatVoice::Snare,
            k: 4,
            n: 4,
            rotation: 0,
        },
        DrumVoiceConfig {
            voice: BeatVoice::Hat,
            k: 16,
            n: 16,
            rotation: 0,
        },
    ];

    let outcome = build_beat(&voices, &tempo, 48_000).unwrap();

    // Check bounds and validity
    for &sample in &outcome.samples {
        assert!(sample.is_finite());
        assert!((-1.0..=1.0).contains(&sample));
    }
}
