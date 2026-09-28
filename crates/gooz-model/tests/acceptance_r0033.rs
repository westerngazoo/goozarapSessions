//! R-0033 AC6 — a style's plan says whether it brings a bass: trap brings an
//! 808, corrido, metal and free bring none, and a plan saved before the field
//! existed still reads, as "no bass" (SPEC-0033 §2.3).

use gooz_model::{BassVoice, MusicalIntent, SoundPlan, parse_intent, plan_sound, style_names};

fn bass_of(text: &str) -> Option<BassVoice> {
    plan_sound(&parse_intent(text)).bass
}

#[test]
fn ac6_trap_brings_an_808_and_no_other_style_does() {
    let styles = style_names();
    assert!(styles.contains(&"trap"), "harness: trap is a style");
    for style in styles {
        let want = (style == "trap").then_some(BassVoice::Sub808);
        assert_eq!(bass_of(style), want, "the {style} style's bass");
    }
}

#[test]
fn ac6_the_preset_decides_the_bass_not_the_word() {
    // Every word that selects trap brings the 808; every word that selects
    // another preset does not — including "trap tumbado", which the table
    // resolves to corrido (R-0026: the first preset in priority order wins).
    for (text, want) in [
        ("trap", Some(BassVoice::Sub808)),
        ("drill", Some(BassVoice::Sub808)),
        ("TRAP oscuro a 150 bpm", Some(BassVoice::Sub808)),
        ("trap limpio", Some(BassVoice::Sub808)),
        (
            "trap con distorsión hasta que cruje",
            Some(BassVoice::Sub808),
        ),
        ("trap metal", Some(BassVoice::Sub808)),
        ("trap tumbado", None),
        ("corrido", None),
        ("tumbado", None),
        ("bélico", None),
        ("regional", None),
        ("metal", None),
        ("black metal", None),
        ("punk", None),
        ("rock", None),
        ("free", None),
        ("bossa nova", None),
        ("", None),
    ] {
        let plan = plan_sound(&parse_intent(text));
        assert_eq!(
            plan.bass, want,
            "{text:?} selected the {} preset",
            plan.preset
        );
    }
}

#[test]
fn ac6_how_busy_or_tense_a_trap_is_does_not_take_its_808_away() {
    for density in [0.0, 0.5, 1.0] {
        for tension in [0.0, 0.5, 1.0] {
            for drive in [0.0, 0.4, 1.0] {
                let intent = MusicalIntent {
                    density,
                    tension,
                    drive,
                    genre: vec!["trap".into()],
                    ..MusicalIntent::default()
                };
                assert_eq!(
                    plan_sound(&intent).bass,
                    Some(BassVoice::Sub808),
                    "density {density}, tension {tension}, drive {drive}"
                );
            }
        }
    }
}

#[test]
fn ac6_the_bass_is_part_of_the_plan_the_ui_is_shown_and_it_round_trips() {
    let plan = plan_sound(&parse_intent("trap"));
    let json = serde_json::to_value(&plan).expect("serializes");
    assert_eq!(json["bass"], "808", "the trap plan the UI is shown");
    let back: SoundPlan = serde_json::from_value(json).expect("deserializes");
    assert_eq!(back, plan, "the trap plan does not round-trip");

    assert_eq!(
        serde_json::to_value(BassVoice::Sub808).expect("serializes"),
        "808"
    );
    assert_eq!(
        serde_json::from_value::<BassVoice>("808".into()).expect("deserializes"),
        BassVoice::Sub808
    );

    let corrido = serde_json::to_value(plan_sound(&parse_intent("corrido"))).expect("serializes");
    assert!(
        corrido["bass"].is_null(),
        "a style with no bass says so: {}",
        corrido["bass"]
    );
}

#[test]
fn ac6_a_plan_saved_before_the_bass_existed_reads_as_no_bass() {
    let mut json = serde_json::to_value(plan_sound(&parse_intent("trap"))).expect("serializes");
    assert_eq!(
        json["bass"], "808",
        "harness: today's trap plan carries a bass, so removing it means something"
    );
    let object = json.as_object_mut().expect("a plan is an object");
    object.remove("bass");
    let old: SoundPlan = serde_json::from_value(json.clone()).expect("an older plan reads");
    assert_eq!(old.bass, None, "a missing bass is no bass");

    json.as_object_mut()
        .expect("a plan is an object")
        .insert("bass".into(), serde_json::Value::Null);
    let explicit: SoundPlan = serde_json::from_value(json).expect("a null bass reads");
    assert_eq!(explicit.bass, None, "a null bass is no bass");
}
