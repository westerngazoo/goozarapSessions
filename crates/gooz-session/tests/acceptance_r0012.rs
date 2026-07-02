//! Acceptance tests for R-0012 — mixdown & export, realized by SPEC-0012.

use std::path::PathBuf;

use gooz_session::{SessionError, Settings, Song, Stem, StemKind, StemPlacement};

// Tiny tempo so bar math is exact and cheap: 120 bpm, 4 beats/bar → bar_seconds
// = 2 s; at sr = 4 Hz that is 8 samples per bar.
const SR: u32 = 4;

fn settings() -> Settings {
    Settings {
        bpm: 120.0,
        beats_per_bar: 4.0,
        root_hz: 220.0,
        odd_limit: 9,
    }
}

/// A one-bar stem (8 samples) that is a constant value, so placement/level math
/// is easy to read in assertions.
fn const_stem(name: &str, kind: StemKind, value: f32) -> Stem {
    Stem {
        name: name.into(),
        kind,
        sample_rate: SR,
        bars: 1,
        samples: vec![value; 8],
    }
}

fn temp_dir(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("gooz_export_{}_{}", std::process::id(), tag));
    p
}

// AC1/AC2 — mixdown sums placements, honours level, loops to fill.
#[test]
fn ac1_ac2_mixdown_sums_scaled_looped_placements() {
    let song = Song::new("mix", settings())
        .with_stem(const_stem("a", StemKind::Riff, 0.5))
        .with_stem(const_stem("b", StemKind::Beat, 0.25))
        // 2-bar arrangement so the 1-bar stems must loop.
        .with_placement(StemPlacement {
            stem: 0,
            start_bar: 0,
            muted: false,
            level: 1.0,
        })
        .with_placement(StemPlacement {
            stem: 1,
            start_bar: 0,
            muted: false,
            level: 0.5, // 0.25 * 0.5 = 0.125
        });
    // Extend length to 2 bars via a section.
    let song = song.with_section(gooz_session::Section {
        name: "all".into(),
        start_bar: 0,
        length_bars: 2,
    });

    let mix = song.mixdown().expect("mixes");
    assert_eq!(mix.sample_rate, SR);
    assert_eq!(mix.samples.len(), 16, "2 bars * 8 samples");
    // Every sample = 0.5 (a) + 0.125 (b) = 0.625, and it holds across the loop.
    assert!(mix.samples.iter().all(|s| (s - 0.625).abs() < 1e-6));
}

#[test]
fn ac2_muted_placement_contributes_nothing() {
    let song = Song::new("mix", settings())
        .with_stem(const_stem("a", StemKind::Riff, 0.5))
        .with_placement(StemPlacement {
            stem: 0,
            start_bar: 0,
            muted: true,
            level: 1.0,
        });
    let mix = song.mixdown().expect("mixes");
    // Muted → nothing placed → empty master.
    assert!(mix.samples.is_empty());
}

#[test]
fn ac2_placement_start_bar_offsets_the_stem() {
    let song = Song::new("mix", settings())
        .with_stem(const_stem("a", StemKind::Riff, 1.0))
        .with_placement(StemPlacement {
            stem: 0,
            start_bar: 1,
            muted: false,
            level: 1.0,
        });
    let mix = song.mixdown().expect("mixes");
    assert_eq!(mix.samples.len(), 16, "stem at bar 1 → 2 bars total");
    assert!(mix.samples[..8].iter().all(|&s| s == 0.0), "bar 0 silent");
    assert!(mix.samples[8..].iter().all(|&s| s == 1.0), "bar 1 the stem");
}

// AC3 — WAV master export reads back with expected frames + rate.
#[test]
fn ac3_export_master_writes_readable_wav() {
    let dir = temp_dir("master");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("master.wav");
    let song = Song::new("mix", settings())
        .with_stem(const_stem("a", StemKind::Riff, 0.5))
        .with_placement(StemPlacement {
            stem: 0,
            start_bar: 0,
            muted: false,
            level: 1.0,
        });
    song.export_master(&path).expect("exports");

    let reader = hound::WavReader::open(&path).expect("reads back");
    assert_eq!(reader.spec().sample_rate, SR);
    assert_eq!(reader.spec().channels, 1);
    assert_eq!(reader.len(), 8, "one bar = 8 frames");
    let _ = std::fs::remove_dir_all(&dir);
}

// AC4 — per-stem export writes one WAV each and returns paths.
#[test]
fn ac4_export_stems_writes_one_wav_per_stem() {
    let dir = temp_dir("stems");
    let song = Song::new("mix", settings())
        .with_stem(const_stem("guitar", StemKind::Riff, 0.5))
        .with_stem(const_stem("drums", StemKind::Beat, 0.25));
    let paths = song.export_stems(&dir).expect("exports stems");
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().all(|p| p.exists()));
    assert!(
        paths[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("guitar")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// AC5 — bounded output; clip-safety limiter engages when the sum would clip.
#[test]
fn ac5_master_is_bounded_with_clip_safety() {
    let song = Song::new("loud", settings())
        .with_stem(const_stem("a", StemKind::Riff, 0.9))
        .with_stem(const_stem("b", StemKind::Beat, 0.9))
        .with_placement(StemPlacement {
            stem: 0,
            start_bar: 0,
            muted: false,
            level: 1.0,
        })
        .with_placement(StemPlacement {
            stem: 1,
            start_bar: 0,
            muted: false,
            level: 1.0,
        });
    let mix = song.mixdown().expect("mixes");
    assert!(!mix.samples.is_empty());
    assert!(
        mix.samples
            .iter()
            .all(|s| s.is_finite() && s.abs() <= 1.0 + 1e-6)
    );
    // 0.9 + 0.9 = 1.8 would clip → limiter scales the peak to 1.0.
    let peak = mix.samples.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    assert!((peak - 1.0).abs() < 1e-6);
}

// AC6 — typed errors / empty song.
#[test]
fn ac6_empty_song_yields_empty_master_not_error() {
    let mix = Song::new("empty", settings()).mixdown().expect("no error");
    assert!(mix.samples.is_empty());
}

#[test]
fn ac6_mismatched_sample_rates_are_a_typed_export_error() {
    let mut other = const_stem("b", StemKind::Beat, 0.5);
    other.sample_rate = SR * 2;
    let song = Song::new("mix", settings())
        .with_stem(const_stem("a", StemKind::Riff, 0.5))
        .with_stem(other)
        .with_placement(StemPlacement {
            stem: 0,
            start_bar: 0,
            muted: false,
            level: 1.0,
        })
        .with_placement(StemPlacement {
            stem: 1,
            start_bar: 0,
            muted: false,
            level: 1.0,
        });
    assert!(matches!(song.mixdown(), Err(SessionError::Export(_))));
}

#[test]
fn ac6_invalid_arrangement_is_rejected_before_mixing() {
    let song = Song::new("bad", settings()).with_placement(StemPlacement {
        stem: 9,
        start_bar: 0,
        muted: false,
        level: 1.0,
    });
    assert!(matches!(
        song.mixdown(),
        Err(SessionError::InvalidArrangement(_))
    ));
}

// AC7 — deterministic.
#[test]
fn ac7_mixdown_is_deterministic() {
    let song = Song::new("mix", settings())
        .with_stem(const_stem("a", StemKind::Riff, 0.5))
        .with_placement(StemPlacement {
            stem: 0,
            start_bar: 0,
            muted: false,
            level: 0.7,
        });
    assert_eq!(song.mixdown().unwrap(), song.mixdown().unwrap());
}
