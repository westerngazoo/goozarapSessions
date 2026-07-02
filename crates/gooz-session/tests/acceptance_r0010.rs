//! Acceptance tests for R-0010 — session format (save/load), realized by
//! SPEC-0010. Pure data round-trips; one temp-file save/load.

use std::path::PathBuf;

use gooz_session::{FORMAT_VERSION, SessionError, Settings, Song, Stem, StemKind, Take};

fn settings() -> Settings {
    Settings {
        bpm: 92.0,
        beats_per_bar: 4.0,
        root_hz: 220.0,
        odd_limit: 9,
    }
}

fn full_song() -> Song {
    Song::new("session 001", settings())
        .with_take(Take {
            name: "hum".into(),
            sample_rate: 48_000,
            samples: vec![0.0, 0.25, -0.25, 0.5],
        })
        .with_stem(Stem {
            name: "guitar".into(),
            kind: StemKind::Riff,
            sample_rate: 48_000,
            bars: 2,
            samples: vec![0.1, -0.1, 0.2, -0.2],
        })
        .with_stem(Stem {
            name: "drums".into(),
            kind: StemKind::Beat,
            sample_rate: 48_000,
            bars: 2,
            samples: vec![1.0, 0.0, -1.0, 0.0],
        })
}

fn temp_path(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("gooz_session_{}_{}.json", std::process::id(), tag));
    p
}

// AC1 — lossless text round-trip.
#[test]
fn ac1_json_round_trip_is_lossless() {
    let song = full_song();
    let json = song.to_json().expect("serializes");
    assert_eq!(Song::from_json(&json).expect("deserializes"), song);
}

// AC2 — save then load.
#[test]
fn ac2_save_then_load_round_trips() {
    let song = full_song();
    let path = temp_path("ac2");
    song.save(&path).expect("saves");
    let loaded = Song::load(&path).expect("loads");
    let _ = std::fs::remove_file(&path);
    assert_eq!(loaded, song);
}

// AC3 — stems carry audio + kind, takes carry audio.
#[test]
fn ac3_stems_and_takes_carry_their_audio() {
    let song = full_song();
    assert_eq!(song.takes[0].samples.len(), 4);
    assert_eq!(song.stems[0].kind, StemKind::Riff);
    assert_eq!(song.stems[1].kind, StemKind::Beat);
    assert_eq!(song.stems[1].bars, 2);
    assert_eq!(song.format_version, FORMAT_VERSION);
}

// AC4 — typed errors, no panic.
#[test]
fn ac4_missing_file_is_typed_io_error() {
    let err = Song::load(temp_path("does_not_exist")).unwrap_err();
    assert!(matches!(err, SessionError::Io(_)));
}

#[test]
fn ac4_corrupt_json_is_typed_deserialize_error() {
    let err = Song::from_json("{ not valid session json ]").unwrap_err();
    assert!(matches!(err, SessionError::Deserialize(_)));
}

// AC5 — empty song round-trips and saves/loads.
#[test]
fn ac5_empty_song_round_trips() {
    let song = Song::new("empty", settings());
    let json = song.to_json().unwrap();
    assert_eq!(Song::from_json(&json).unwrap(), song);

    let path = temp_path("ac5");
    song.save(&path).unwrap();
    let loaded = Song::load(&path).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(loaded, song);
    assert!(loaded.takes.is_empty() && loaded.stems.is_empty());
}

// AC6 — deterministic serialization.
#[test]
fn ac6_serialization_is_deterministic() {
    let song = full_song();
    assert_eq!(song.to_json().unwrap(), song.to_json().unwrap());
}

// AC4/typed — Display is non-empty and Error-implementing.
#[test]
fn session_error_displays() {
    let err: &dyn std::error::Error = &SessionError::Io("disk full".into());
    assert!(err.to_string().contains("disk full"));
}
