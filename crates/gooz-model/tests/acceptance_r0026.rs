//! R-0026 acceptance tests — `MusicalIntent` → `SoundPlan`.
//!
//! Drives the public crate seam only. One test per acceptance criterion.

use gooz_model::{Meter, MusicalIntent, SoundPlan, VoiceRole, parse_intent, plan_sound};

const REFERENCE_PROMPT: &str = "Pon el tempo a 135 BPM. La batería trap + tumbado en un compás \
     de 6/8, snare seco en el tercer tiempo, y satura los hi-hats para que hagan tresillos \
     rápidos. El bajo: un 808 largo con distorsión. La guitarra: black metal, segundas menores.";

fn intent(density: f32, tension: f32, genre: &[&str]) -> MusicalIntent {
    MusicalIntent {
        density,
        tension,
        genre: genre.iter().map(|g| (*g).to_string()).collect(),
        ..MusicalIntent::default()
    }
}

fn lane(plan: &SoundPlan, role: VoiceRole) -> &gooz_model::VoicePlan {
    plan.voices
        .iter()
        .find(|v| v.role == role)
        .expect("every plan carries all three lanes")
}

#[test]
fn ac1_a_plan_carries_everything_the_engine_needs() {
    let plan = plan_sound(&parse_intent(REFERENCE_PROMPT));
    assert_eq!(plan.tempo_bpm, 135.0);
    assert_eq!(plan.meter, Meter { beats: 6, unit: 8 });
    assert!(plan.odd_limit >= 3 && plan.drive >= 0.0);
    assert_eq!(plan.voices.len(), 3, "kick, snare and hat");
    // Deterministic against a pinned expectation, not merely against itself.
    assert_eq!(plan, plan_sound(&parse_intent(REFERENCE_PROMPT)));
    assert_eq!(plan.preset, "corrido");
}

#[test]
fn ac2_presets_are_selected_by_tag_and_fall_back_to_free() {
    for (tags, expected) in [
        (&["trap"][..], "trap"),
        (&["drill"], "trap"),
        (&["tumbado"], "corrido"),
        (&["black metal"], "metal"),
        (&["rock"], "metal"),
    ] {
        assert_eq!(plan_sound(&intent(0.5, 0.3, tags)).preset, expected);
    }
    // Unknown or absent: still playable, never an error.
    assert_eq!(plan_sound(&intent(0.5, 0.3, &[])).preset, "free");
    assert_eq!(
        plan_sound(&intent(0.5, 0.3, &["polka-espacial"])).preset,
        "free"
    );
}

#[test]
fn ac3_the_sliders_actually_modulate_the_preset() {
    for genre in [&[][..], &["trap"], &["corrido"], &["metal"]] {
        let sparse = plan_sound(&intent(0.0, 0.3, genre));
        let busy = plan_sound(&intent(1.0, 0.3, genre));
        assert!(
            busy.total_onsets() > sparse.total_onsets(),
            "density must change {genre:?}: {} → {}",
            sparse.total_onsets(),
            busy.total_onsets()
        );
        let smooth = plan_sound(&intent(0.5, 0.0, genre));
        let tense = plan_sound(&intent(0.5, 1.0, genre));
        assert!(
            tense.odd_limit > smooth.odd_limit,
            "tension must change the grid: {} → {}",
            smooth.odd_limit,
            tense.odd_limit
        );
    }
    let driven = MusicalIntent {
        drive: 0.83,
        ..MusicalIntent::default()
    };
    assert_eq!(plan_sound(&driven).drive, 0.83, "drive passes through");
}

#[test]
fn ac4_the_bar_is_laid_out_in_the_requested_meter() {
    for (beats, unit) in [(4, 4), (6, 8), (3, 4), (7, 8), (12, 8)] {
        let plan = plan_sound(&MusicalIntent {
            meter: Meter { beats, unit },
            genre: vec!["corrido".into()],
            ..MusicalIntent::default()
        });
        assert_eq!(
            plan.steps() % plan.meter.beats,
            0,
            "{beats}/{unit} must give whole steps per beat"
        );
    }
    // The corrido accent lands on beat 3: two beats in, at two steps per beat.
    let plan = plan_sound(&MusicalIntent {
        meter: Meter { beats: 6, unit: 8 },
        genre: vec!["corrido".into()],
        ..MusicalIntent::default()
    });
    assert_eq!(plan.steps(), 12);
    assert_eq!(lane(&plan, VoiceRole::Snare).rotate, 4);
}

#[test]
fn ac5_every_reachable_intent_yields_a_playable_plan() {
    // The meter axis is the one that used to overflow the step arithmetic.
    let meters = [
        Meter { beats: 4, unit: 4 },
        Meter { beats: 6, unit: 8 },
        Meter { beats: 7, unit: 8 },
        Meter {
            beats: 32,
            unit: 16,
        },
        Meter {
            beats: u32::MAX,
            unit: 4,
        },
        Meter { beats: 0, unit: 0 },
    ];
    for meter in meters {
        for density in [0.0, 0.5, 1.0] {
            for genre in [&[][..], &["trap"], &["corrido"], &["metal"]] {
                let plan = plan_sound(
                    &MusicalIntent {
                        meter,
                        density,
                        genre: genre.iter().map(|g| (*g).to_string()).collect(),
                        ..MusicalIntent::default()
                    }
                    .normalized(),
                );
                assert!(plan.meter.is_valid(), "unplayable meter from {meter:?}");
                assert!(plan.steps() > 0 && plan.steps() <= 32 * 4);
                for voice in &plan.voices {
                    assert!(
                        voice.onsets > 0 && voice.onsets <= voice.steps,
                        "0 < k <= n violated from {meter:?}: {voice:?}"
                    );
                    assert!((0.0..=1.0).contains(&voice.level));
                }
                assert!((0.0..=1.0).contains(&plan.drive));
            }
        }
    }
}

#[test]
fn ac6_the_reference_prompt_plans_the_described_song() {
    let plan = plan_sound(&parse_intent(REFERENCE_PROMPT));
    assert_eq!(plan.tempo_bpm, 135.0);
    assert_eq!(plan.meter, Meter { beats: 6, unit: 8 });
    assert_eq!(plan.preset, "corrido");
    assert!(plan.odd_limit >= 11, "minor seconds read strongly tense");
    assert!(plan.drive >= 0.7, "distortion reads strongly driven");
    assert!(
        lane(&plan, VoiceRole::Hat).onsets > lane(&plan, VoiceRole::Kick).onsets,
        "saturated hats are the busy lane"
    );
    assert_eq!(lane(&plan, VoiceRole::Snare).rotate, 4, "snare on beat 3");
}
