//! Describe → [`MusicalIntent`]: the [`Parser`] seam and its deterministic
//! implementation (R-0025 / SPEC-0025).
//!
//! [`DefaultParser`] is always compiled and never fails: it scans the
//! description for numbers and cue words and leaves everything it does not
//! recognize neutral. A language-model parser (SPEC-0025 §2, cargo feature
//! `llm`) plugs into the same trait and falls back here on any failure, so the
//! app degrades gracefully instead of erroring.

use crate::intent::{MAX_BPM, MIN_BPM, Meter, MusicalIntent};

/// Turns a natural-language description into a [`MusicalIntent`].
///
/// Implementations **never fail**: an empty, garbled, or un-actionable
/// description yields the neutral default intent (the honesty rule).
pub trait Parser {
    /// Parses `prompt` into an intent. Always returns a normalized, usable value.
    fn parse(&self, prompt: &str) -> MusicalIntent;
}

/// How far a single cue word moves a slider, and the level a strong cue sets.
const CUE_STEP: f32 = 0.25;

/// Cue words that push each slider up or down. Matched case-insensitively as
/// **whole words** (a phrase matches a whole-word run), so Spanish and English
/// descriptions both land while fragments inside longer words never fire.
const TENSION_HIGH: &[&str] = &[
    "tenso",
    "tensión",
    "tension",
    "menor",
    "minor",
    "segundas menores",
    "disonante",
    "dissonant",
    "dark",
    "oscuro",
    "black metal",
    "metal",
    "spooky",
    "siniestro",
];
const TENSION_LOW: &[&str] = &[
    "suave",
    "smooth",
    "mayor",
    "major",
    "warm",
    "cálido",
    "consonante",
    "consonant",
    "chill",
];
const DENSITY_HIGH: &[&str] = &[
    "saturado",
    "saturados",
    "busy",
    "denso",
    "rápido",
    "rapido",
    "fast",
    "tresillo",
    "tresillos",
    "triplet",
    "triplets",
    "roll",
    "rolls",
    "hi-hats",
    "hats",
];
const DENSITY_LOW: &[&str] = &["sparse", "lento", "slow", "minimal", "espaciado", "simple"];
const DRIVE_HIGH: &[&str] = &[
    "distorsión",
    "distorsion",
    "distortion",
    "distorsionado",
    "overdrive",
    "drive",
    "satura",
    "saturación",
    "crujir",
    "cruje",
    "fuzz",
    "crush",
    "abrasivo",
];
const DRIVE_LOW: &[&str] = &["limpio", "clean", "suave", "acústico", "acoustic"];

/// Genre and mood vocabularies. Only tags the description actually contains are
/// reported — the parser never invents a genre.
const GENRE_VOCAB: &[&str] = &[
    "trap",
    "corrido",
    "tumbado",
    "bélico",
    "belico",
    "regional",
    "metal",
    "black metal",
    "drill",
    "lofi",
    "house",
    "techno",
    "reggaeton",
    "cumbia",
    "rock",
    "punk",
    "jazz",
    "bolero",
];
const MOOD_VOCAB: &[&str] = &[
    "dark",
    "oscuro",
    "spooky",
    "siniestro",
    "triste",
    "sad",
    "uplifting",
    "alegre",
    "happy",
    "warm",
    "cálido",
    "aggressive",
    "agresivo",
    "chill",
    "relajado",
    "épico",
    "epico",
    "epic",
];

/// A deterministic keyword and number scan. Pure, fast, and fully testable — the
/// parser the toolchain gates exercise, and the fallback for the model-backed one.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultParser;

impl Parser for DefaultParser {
    fn parse(&self, prompt: &str) -> MusicalIntent {
        let text = prompt.to_lowercase();
        let words = tokenize(&text);
        let mut intent = MusicalIntent::default();

        if let Some(bpm) = scan_tempo(&text) {
            intent.tempo_bpm = bpm;
        }
        if let Some(meter) = scan_meter(&text) {
            intent.meter = meter;
        }
        intent.tension = cue_slider(&words, TENSION_HIGH, TENSION_LOW, intent.tension);
        intent.density = cue_slider(&words, DENSITY_HIGH, DENSITY_LOW, intent.density);
        intent.drive = cue_slider(&words, DRIVE_HIGH, DRIVE_LOW, intent.drive);
        intent.genre = tags_present(&words, GENRE_VOCAB);
        intent.mood = tags_present(&words, MOOD_VOCAB);

        intent.normalized()
    }
}

/// Parses a description with the deterministic [`DefaultParser`].
///
/// ```
/// use gooz_model::parse_intent;
///
/// let intent = parse_intent("corrido tumbado en 6/8 a 135 bpm, 808 distorsionado");
/// assert_eq!(intent.tempo_bpm, 135.0);
/// assert_eq!((intent.meter.beats, intent.meter.unit), (6, 8));
/// assert!(intent.drive > 0.5); // "distorsionado" pushes the drive up
/// assert!(intent.genre.contains(&"corrido".to_string()));
///
/// // Nothing recognizable? A neutral intent, never an error.
/// assert_eq!(parse_intent(""), gooz_model::MusicalIntent::default());
/// ```
pub fn parse_intent(prompt: &str) -> MusicalIntent {
    DefaultParser.parse(prompt)
}

/// Splits lowercased text into whole words, so a cue only matches a real word —
/// never a fragment of a longer one ("hats" must not fire inside "whats", and a
/// plugin called "camelcrusher" is not the word "crush").
fn tokenize(text: &str) -> Vec<&str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect()
}

/// True when `cue` — one word or a phrase — appears as a whole-word run in
/// `words`.
fn contains_cue(words: &[&str], cue: &str) -> bool {
    let needle = tokenize(cue);
    match needle.len() {
        0 => false,
        1 => words.contains(&needle[0]),
        _ => words
            .windows(needle.len())
            .any(|run| run == needle.as_slice()),
    }
}

/// Finds a tempo: the first *plausible* number adjacent to "bpm" or "tempo".
///
/// Plausibility matters in this domain: "un 808, bpm 135" must yield 135, not
/// the 808 sitting on the marker's other side.
fn scan_tempo(text: &str) -> Option<f64> {
    // This cannot reuse `tokenize`: a tempo may be decimal, so `.` must stay
    // inside tokens here (and be trimmed when comparing the marker), whereas
    // whole-word cue matching wants it treated as a separator.
    // Keep only real tokens: splitting on punctuation leaves empty strings that
    // would otherwise sit between a number and its marker ("a 135 BPM.").
    let words: Vec<&str> = text
        .split(|c: char| !c.is_alphanumeric() && c != '.')
        .filter(|w| !w.is_empty())
        .collect();
    // `.` stays inside tokens so decimals survive, so trim it before comparing
    // ("135 bpm." tokenizes the marker as "bpm.").
    let is_marker = |w: &str| matches!(w.trim_matches('.'), "bpm" | "tempo");
    for (i, word) in words.iter().enumerate() {
        if !is_marker(word) {
            continue;
        }
        let before = i.checked_sub(1).and_then(|j| words.get(j));
        let after = words.get(i + 1);
        for candidate in [before, after].into_iter().flatten() {
            if let Ok(bpm) = candidate.trim_matches('.').parse::<f64>() {
                // Skip an implausible neighbour (an "808" is a bass, not a
                // tempo) and keep looking on the other side / at later markers.
                if (MIN_BPM..=MAX_BPM).contains(&bpm) {
                    return Some(bpm);
                }
            }
        }
    }
    None
}

/// Finds a meter: the first `beats/unit` pair with a beat unit the grid knows.
fn scan_meter(text: &str) -> Option<Meter> {
    for chunk in text.split(|c: char| c.is_whitespace() || c == ',') {
        // Strip surrounding punctuation, then skip chunks that hold no pair —
        // `?` here would abandon the whole scan at the first plain word.
        let chunk = chunk.trim_matches(|c: char| !c.is_ascii_digit() && c != '/');
        let Some((beats, unit)) = chunk.split_once('/') else {
            continue;
        };
        if let (Ok(beats), Ok(unit)) = (beats.parse::<u32>(), unit.parse::<u32>()) {
            let meter = Meter { beats, unit };
            if meter.is_valid() {
                return Some(meter);
            }
        }
    }
    None
}

/// Moves a slider by how many high/low cues the description uses as words.
///
/// One musical statement counts once: when a matching cue is contained in
/// another matching cue ("hats" inside "hi-hats"), only the longer one scores,
/// so a single phrase cannot move a slider twice.
fn cue_slider(words: &[&str], high: &[&str], low: &[&str], neutral: f32) -> f32 {
    let delta = (distinct_hits(words, high) - distinct_hits(words, low)) * CUE_STEP;
    (neutral + delta).clamp(0.0, 1.0)
}

/// Counts matching cues, discarding any that a longer matching cue subsumes.
fn distinct_hits(words: &[&str], cues: &[&str]) -> f32 {
    let mut matched: Vec<&str> = cues
        .iter()
        .copied()
        .filter(|cue| contains_cue(words, cue))
        .collect();
    matched.sort_by_key(|cue| std::cmp::Reverse(cue.len()));
    let mut counted: Vec<&str> = Vec::new();
    for cue in matched {
        if counted
            .iter()
            .any(|kept| contains_cue(&tokenize(kept), cue))
        {
            continue;
        }
        counted.push(cue);
    }
    counted.len() as f32
}

/// Collects the vocabulary terms the description mentions, in vocabulary order.
///
/// A compound tag and its base are both reported ("black metal" also yields
/// "metal"): the specific term describes the style, the base term is what
/// coarse consumers — the preset library (R-0026) — look up. Deciding that one
/// subsumes the other is the consumer's job, not the parser's.
fn tags_present(words: &[&str], vocab: &[&str]) -> Vec<String> {
    vocab
        .iter()
        .filter(|term| contains_cue(words, term))
        .map(|term| (*term).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::{DEFAULT_DENSITY, DEFAULT_DRIVE, DEFAULT_TENSION};

    /// The owner's north-star prompt (AC4), reduced to its musical sentences.
    /// Kept verbatim where it matters: the tempo arrives as "135 BPM", which is
    /// the marker form the deterministic scanner needs (a bare "a 135" is left
    /// to the language-model parser, AC3).
    const NORTH_STAR: &str = "Pon el tempo a 135 BPM. La batería trap + tumbado en un compás de \
         6/8, snare seco en el tercer tiempo, y satura los hi-hats para que hagan tresillos \
         rápidos. El bajo: un 808 largo con distorsión hasta que cruje. La guitarra: black metal \
         + requinto, notas consecutivas muy juntas (segundas menores para dar tensión), con \
         reverb de 4 segundos.";

    #[test]
    fn ac4_north_star_prompt_yields_the_actionable_params() {
        let intent = parse_intent(NORTH_STAR);
        assert_eq!(intent.tempo_bpm, 135.0, "tempo comes from '135 BPM'");
        assert_eq!(
            (intent.meter.beats, intent.meter.unit),
            (6, 8),
            "6/8 corrido"
        );
        assert!(
            intent.tension > DEFAULT_TENSION,
            "minor seconds / black metal read tense"
        );
        assert!(
            intent.density > DEFAULT_DENSITY,
            "saturated hats + triplets read busy"
        );
        assert!(intent.drive > DEFAULT_DRIVE, "distortion reads driven");
        // The prompt names "trap + tumbado" and "black metal". The compound tag
        // and its base are both reported, so a coarse consumer keying on
        // "metal" still matches (R-0026 presets).
        for tag in ["trap", "tumbado", "black metal", "metal"] {
            assert!(
                intent.genre.contains(&tag.to_string()),
                "genre should include {tag}"
            );
        }
        assert!(
            !intent.genre.contains(&"corrido".to_string()),
            "never invent a genre the prompt did not name"
        );
    }

    #[test]
    fn ac4_unactionable_content_is_ignored_without_error() {
        // A DAW instruction plus artist names: nothing musical to extract, so
        // the whole intent must stay neutral.
        let intent =
            parse_intent("Abre tu DAW y busca a Sematary y a Natanael Cano en Spotify o YouTube.");
        assert_eq!(intent, MusicalIntent::default());
    }

    #[test]
    fn cues_only_fire_on_whole_words() {
        // "whats" contains "hats", "simplemente" contains "simple", and a
        // plugin named "camelcrusher" contains "crush" — none may move a slider.
        for text in ["whats the vibe", "simplemente hazlo", "carga camelcrusher"] {
            assert_eq!(parse_intent(text), MusicalIntent::default(), "text: {text}");
        }
    }

    #[test]
    fn an_808_next_to_a_marker_is_not_a_tempo() {
        // "808" is a bass in this domain, not a plausible BPM: the real tempo
        // on the marker's other side must win.
        assert_eq!(parse_intent("un 808, bpm 135").tempo_bpm, 135.0);
        assert_eq!(parse_intent("bajo 808 con tempo 140").tempo_bpm, 140.0);
    }

    #[test]
    fn a_phrase_does_not_score_twice_through_its_own_words() {
        // "hi-hats" contains "hats"; "black metal" contains "metal". One
        // statement must move a slider once, not twice.
        assert_eq!(
            parse_intent("satura los hi-hats").density,
            parse_intent("satura los hats").density
        );
        assert_eq!(
            parse_intent("black metal").tension,
            parse_intent("metal").tension
        );
    }

    #[test]
    fn a_repeated_cue_is_not_double_counted() {
        // Singular and plural are separate vocabulary words, so one musical
        // statement moves the slider once either way.
        assert_eq!(parse_intent("roll").density, parse_intent("rolls").density);
    }

    #[test]
    fn ac2_empty_or_garbage_prompt_is_neutral() {
        assert_eq!(parse_intent(""), MusicalIntent::default());
        assert_eq!(parse_intent("   \n\t "), MusicalIntent::default());
        assert_eq!(parse_intent("asdf qwerty 🎹"), MusicalIntent::default());
    }

    #[test]
    fn ac2_parsing_is_deterministic() {
        assert_eq!(parse_intent(NORTH_STAR), parse_intent(NORTH_STAR));
    }

    #[test]
    fn tempo_is_read_on_either_side_of_the_marker() {
        assert_eq!(parse_intent("135 bpm").tempo_bpm, 135.0);
        assert_eq!(parse_intent("tempo 140").tempo_bpm, 140.0);
        // An implausible tempo is ignored (the plausibility gate drops it before
        // `normalized` ever sees it), leaving the neutral default.
        assert_eq!(
            parse_intent("9000 bpm").tempo_bpm,
            crate::intent::DEFAULT_BPM
        );
        assert_eq!(
            parse_intent("tempo 39").tempo_bpm,
            crate::intent::DEFAULT_BPM
        );
    }

    #[test]
    fn meter_only_accepts_units_the_grid_understands() {
        assert_eq!(parse_intent("en 7/8").meter, Meter { beats: 7, unit: 8 });
        // 5/7 is not a grid meter → stays 4/4.
        assert_eq!(parse_intent("en 5/7").meter, Meter::default());
    }

    #[test]
    fn opposing_cues_cancel_toward_neutral() {
        let clean = parse_intent("limpio y suave");
        let dirty = parse_intent("distorsión abrasiva");
        assert!(clean.drive < dirty.drive);
    }

    #[test]
    fn the_trait_and_the_helper_agree() {
        assert_eq!(DefaultParser.parse(NORTH_STAR), parse_intent(NORTH_STAR));
    }
}
