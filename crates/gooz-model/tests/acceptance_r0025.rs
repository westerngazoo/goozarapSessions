//! R-0025 acceptance tests — describe → `MusicalIntent`.
//!
//! Drives the **public crate seam** only, with the owner's reference prompt in
//! its raw form (DAW steps, plugin names and artists included) so the "ignore
//! the un-actionable" clause is exercised end to end rather than on a tidied
//! fixture. One test per acceptance criterion.

use gooz_model::{DefaultParser, Meter, MusicalIntent, Parser, SectionIntent, parse_intent};

/// The owner's prompt as written, including everything the parser must ignore.
const REFERENCE_PROMPT: &str = "Abre tu DAW y pon el tempo a 135 BPM. La batería (Trap + \
     Tumbado): en lugar del típico ritmo de trap en 4/4, programa el patrón en un compás de 6/8, \
     pon un snare seco en el tercer tiempo y satura los hi-hats para que hagan tresillos rápidos. \
     El bajo: carga un sampler con un bajo 808 largo y agrégale un plugin de distorsión (como \
     Blood Overdrive o CamelCrusher) y sube el Drive hasta que el bajo empiece a crujir. La \
     guitarra (Black Metal + Requinto): toca notas consecutivas muy juntas, segundas menores para \
     dar tensión. Busca a Sematary y a Natanael Cano para escuchar la mezcla.";

#[test]
fn ac1_intent_is_neutral_by_default_and_round_trips() {
    let neutral = MusicalIntent::default();
    assert_eq!(parse_intent(""), neutral, "an empty prompt is neutral");

    // Round-trip every field, including the two the unit tests leave untouched.
    let rich = MusicalIntent {
        mood: vec!["dark".into()],
        structure: vec![SectionIntent {
            name: "hook".into(),
            bars: 8,
        }],
        ..parse_intent(REFERENCE_PROMPT)
    };
    let json = serde_json::to_string(&rich).expect("serializes");
    assert_eq!(
        serde_json::from_str::<MusicalIntent>(&json).expect("deserializes"),
        rich
    );
}

#[test]
fn ac2_the_parser_seam_is_deterministic_and_total() {
    // The trait and the free helper are the same contract.
    assert_eq!(
        DefaultParser.parse(REFERENCE_PROMPT),
        parse_intent(REFERENCE_PROMPT)
    );
    // A pinned golden value: this catches silent drift, which comparing a call
    // with itself cannot.
    let intent = parse_intent("corrido tumbado en 6/8 a 135 bpm, limpio");
    assert_eq!(intent.tempo_bpm, 135.0);
    assert_eq!(intent.meter, Meter { beats: 6, unit: 8 });
    assert_eq!(
        intent.genre,
        vec!["corrido".to_string(), "tumbado".to_string()]
    );
    assert!(
        intent.drive < MusicalIntent::default().drive,
        "'limpio' cleans"
    );
}

#[test]
fn ac4_the_reference_prompt_yields_its_actionable_parameters() {
    let intent = parse_intent(REFERENCE_PROMPT);
    assert_eq!(intent.tempo_bpm, 135.0, "'135 BPM', not the 808 or the 4/4");
    assert_eq!(intent.meter, Meter { beats: 4, unit: 4 }, "first pair wins");
    assert!(intent.tension >= 0.7, "segundas menores read tense");
    assert!(intent.density >= 0.7, "saturated triplet hats read busy");
    assert!(intent.drive >= 0.7, "distortion reads driven");
    for tag in ["trap", "tumbado", "black metal", "metal"] {
        assert!(intent.genre.contains(&tag.to_string()), "missing {tag}");
    }
    assert!(
        !intent.genre.contains(&"corrido".to_string()),
        "the prompt says tumbado, never corrido — a genre is never invented"
    );
}

#[test]
fn ac5_every_field_is_independently_overridable() {
    let parsed = parse_intent(REFERENCE_PROMPT);

    let edited = MusicalIntent {
        tempo_bpm: 90.0,
        meter: Meter { beats: 3, unit: 4 },
        tension: 0.1,
        density: 0.2,
        drive: 0.3,
        genre: vec!["lofi".into()],
        mood: vec!["chill".into()],
        structure: vec![SectionIntent {
            name: "intro".into(),
            bars: 4,
        }],
    }
    .normalized();

    assert_ne!(edited, parsed, "the edit takes effect");
    assert_eq!(edited.tempo_bpm, 90.0);
    assert_eq!(edited.meter, Meter { beats: 3, unit: 4 });
    assert_eq!(edited.genre, vec!["lofi".to_string()]);
    assert_eq!(edited.mood, vec!["chill".to_string()]);
    assert_eq!(edited.structure[0].bars, 4);
    assert!(edited.tension == 0.1 && edited.density == 0.2 && edited.drive == 0.3);
}

#[test]
fn ac6_hostile_input_is_survived_not_trusted() {
    // Nothing a description can say may panic or escape the valid ranges.
    for prompt in [
        "",
        "asdf qwerty",
        "9000 bpm",
        "tempo 39",
        "en un compás de 4000000000/4 a 135 bpm", // would overflow the plan layer
        "compás de 0/0",
        "😀🎵",
    ] {
        let intent = parse_intent(prompt);
        assert!(
            intent.meter.is_valid(),
            "prompt {prompt:?} produced an unplayable meter: {:?}",
            intent.meter
        );
        assert!(
            (40.0..=250.0).contains(&intent.tempo_bpm),
            "prompt {prompt:?}"
        );
        for slider in [intent.tension, intent.density, intent.drive] {
            assert!((0.0..=1.0).contains(&slider), "prompt {prompt:?}");
        }
    }
}
