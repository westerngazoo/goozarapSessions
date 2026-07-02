//! Acceptance tests for R-0011 — arrangement (sections, loop, placements),
//! realized by SPEC-0011.

use gooz_session::{
    LoopRegion, Section, SessionError, Settings, Song, Stem, StemKind, StemPlacement,
};

fn settings() -> Settings {
    Settings {
        bpm: 92.0,
        beats_per_bar: 4.0,
        root_hz: 220.0,
        odd_limit: 9,
    }
}

fn stem(name: &str, kind: StemKind, bars: u32) -> Stem {
    Stem {
        name: name.into(),
        kind,
        sample_rate: 48_000,
        bars,
        samples: vec![0.1, -0.1, 0.2, -0.2],
    }
}

fn arranged_song() -> Song {
    Song::new("session 001", settings())
        .with_stem(stem("guitar", StemKind::Riff, 2))
        .with_stem(stem("drums", StemKind::Beat, 4))
        .with_section(Section {
            name: "verse".into(),
            start_bar: 0,
            length_bars: 4,
        })
        .with_section(Section {
            name: "hook".into(),
            start_bar: 4,
            length_bars: 4,
        })
        .with_loop(LoopRegion {
            start_bar: 4,
            length_bars: 4,
        })
        .with_placement(StemPlacement {
            stem: 0,
            start_bar: 0,
            muted: false,
            level: 0.8,
        })
        .with_placement(StemPlacement {
            stem: 1,
            start_bar: 0,
            muted: true,
            level: 1.0,
        })
}

// AC1 — sections as bar spans.
#[test]
fn ac1_sections_are_named_bar_spans() {
    let song = arranged_song();
    assert_eq!(song.arrangement.sections.len(), 2);
    assert_eq!(song.arrangement.sections[1].name, "hook");
    assert_eq!(song.arrangement.sections[0].end_bar(), 4);
}

// AC2 — optional loop region.
#[test]
fn ac2_loop_region_is_optional() {
    let song = arranged_song();
    let l = song.arrangement.loop_region.expect("loop set");
    assert_eq!(l.end_bar(), 8);
    assert!(
        Song::new("no loop", settings())
            .arrangement
            .loop_region
            .is_none()
    );
}

// AC3 — placement carries stem index + start + mute + level.
#[test]
fn ac3_placements_carry_mute_and_level() {
    let song = arranged_song();
    assert_eq!(song.arrangement.placements[0].stem, 0);
    assert!(!song.arrangement.placements[0].muted);
    assert_eq!(song.arrangement.placements[0].level, 0.8);
    assert!(song.arrangement.placements[1].muted);
}

// AC4 — validation.
#[test]
fn ac4_valid_arrangement_passes() {
    assert!(arranged_song().validate().is_ok());
}

#[test]
fn ac4_zero_length_section_is_rejected() {
    let song = Song::new("bad", settings()).with_section(Section {
        name: "oops".into(),
        start_bar: 0,
        length_bars: 0,
    });
    assert!(matches!(
        song.validate(),
        Err(SessionError::InvalidArrangement(_))
    ));
}

#[test]
fn ac4_placement_to_missing_stem_is_rejected() {
    let song = Song::new("bad", settings()).with_placement(StemPlacement {
        stem: 7,
        start_bar: 0,
        muted: false,
        level: 1.0,
    });
    assert!(matches!(
        song.validate(),
        Err(SessionError::InvalidArrangement(_))
    ));
}

#[test]
fn ac4_out_of_range_level_is_rejected() {
    let song = Song::new("bad", settings())
        .with_stem(stem("guitar", StemKind::Riff, 1))
        .with_placement(StemPlacement {
            stem: 0,
            start_bar: 0,
            muted: false,
            level: 1.5,
        });
    assert!(matches!(
        song.validate(),
        Err(SessionError::InvalidArrangement(_))
    ));
}

// AC5 — round-trip with arrangement; older sessions load with an empty one.
#[test]
fn ac5_arrangement_round_trips() {
    let song = arranged_song();
    let json = song.to_json().unwrap();
    assert_eq!(Song::from_json(&json).unwrap(), song);
}

#[test]
fn ac5_session_without_arrangement_field_defaults_to_empty() {
    // A pre-arrangement (R-0010) session JSON: no `arrangement` key.
    let json = r#"{
        "format_version": 1,
        "name": "old",
        "settings": { "bpm": 92.0, "beats_per_bar": 4.0, "root_hz": 220.0, "odd_limit": 9 },
        "takes": [],
        "stems": [],
        "model_ref": null
    }"#;
    let song = Song::from_json(json).expect("old session still loads");
    assert!(song.arrangement.sections.is_empty());
    assert!(song.arrangement.loop_region.is_none());
    assert!(song.arrangement.placements.is_empty());
}

// AC6 — total length.
#[test]
fn ac6_total_bars_reports_furthest_end() {
    let song = arranged_song();
    // drums (stem 1, 4 bars) placed at bar 0 → ends at 4; hook section ends at 8;
    // loop ends at 8. Furthest = 8.
    assert_eq!(song.arrangement.total_bars(&song.stems), 8);
    assert_eq!(
        Song::new("empty", settings()).arrangement.total_bars(&[]),
        0
    );
}
