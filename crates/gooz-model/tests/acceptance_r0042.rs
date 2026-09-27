//! R-0042's footprint on the preset table (R-0026): every style has a tempo of
//! its own, and the set of styles is something a UI can ask for rather than
//! hard-code.

use gooz_model::{parse_intent, plan_sound, style_names};

#[test]
fn every_style_has_a_tempo_of_its_own() {
    // Owner decision (R-0042): corrido 105, trap 140, metal 160, free 92.
    // Before this, every style chip gave 92 — Easy Mode's default wearing the
    // style's name.
    for (style, bpm) in [
        ("corrido", 105.0),
        ("trap", 140.0),
        ("metal", 160.0),
        ("free", 92.0),
    ] {
        let plan = plan_sound(&parse_intent(style));
        assert_eq!(plan.style_bpm, bpm, "{style}");
    }
}

#[test]
fn a_styles_tempo_does_not_overrule_one_the_text_stated() {
    // `tempo_bpm` is still the intent's; `style_bpm` is the preset's natural
    // tempo alongside it. The describe path (R-0027) is unchanged.
    let plan = plan_sound(&parse_intent("trap 128 bpm"));
    assert_eq!(plan.tempo_bpm, 128.0);
    assert_eq!(plan.style_bpm, 140.0);
}

#[test]
fn the_styles_a_ui_offers_are_exactly_the_preset_table() {
    let styles = style_names();
    assert_eq!(styles, vec!["corrido", "trap", "metal", "free"]);
    // And each one selects itself.
    for style in styles {
        assert_eq!(plan_sound(&parse_intent(style)).preset, style);
    }
}

// ---------------------------------------------------------------------------
// QA sign-off additions (R-0042, step 7).
// ---------------------------------------------------------------------------

#[test]
fn qa_a_styles_tempo_follows_the_preset_not_the_word_that_chose_it() {
    // Every tag selects its preset's tempo; anything the table does not know
    // is the neutral style, at its tempo — never 0, never the intent's.
    for (text, preset, bpm) in [
        ("tumbado", "corrido", 105.0),
        ("bélico", "corrido", 105.0),
        ("drill", "trap", 140.0),
        ("rock", "metal", 160.0),
        ("punk", "metal", 160.0),
        ("bossa nova", "free", 92.0),
        ("", "free", 92.0),
    ] {
        let plan = plan_sound(&parse_intent(text));
        assert_eq!(plan.preset, preset, "{text:?}");
        assert_eq!(plan.style_bpm, bpm, "{text:?}");
    }
    assert_eq!(plan_sound(&Default::default()).style_bpm, 92.0);
}

#[test]
fn qa_every_style_tempo_is_one_the_engine_can_play() {
    // The style tempo is the clock whenever a take has no pulse, so it must be
    // inside the range the rest of the engine lays audio out in.
    for style in style_names() {
        let bpm = plan_sound(&parse_intent(style)).style_bpm;
        assert!(
            (gooz_dsp::MIN_BPM..=gooz_dsp::MAX_BPM).contains(&bpm),
            "{style}: {bpm}"
        );
    }
}

#[test]
fn qa_the_style_tempo_is_part_of_the_plan_the_ui_is_shown() {
    let json = serde_json::to_value(plan_sound(&parse_intent("metal"))).expect("serializes");
    assert_eq!(json["styleBpm"], 160.0);
    assert_eq!(
        json["tempoBpm"], 92.0,
        "the description's tempo is unchanged"
    );
}

#[test]
fn a_plan_saved_before_styles_had_a_tempo_still_reads() {
    let mut plan = serde_json::to_value(plan_sound(&parse_intent("trap"))).expect("serializes");
    plan.as_object_mut().expect("an object").remove("styleBpm");
    let read: gooz_model::SoundPlan = serde_json::from_value(plan).expect("an older plan reads");
    assert_eq!(read.style_bpm, gooz_model::DEFAULT_BPM);
}
