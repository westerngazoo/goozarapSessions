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
