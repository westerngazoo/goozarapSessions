//! [`MusicalIntent`] — the inspectable seam between a text description and the
//! ratio engine (R-0025 / SPEC-0025).
//!
//! Every field has a neutral default, so an empty or unparseable description
//! still yields a valid intent (the honesty rule: the language layer *biases*
//! the math, it never gates it). The struct is deliberately small, readable, and
//! editable — the UI shows it and the user may override any field before it
//! drives generation.

use serde::{Deserialize, Serialize};

/// Easy Mode's neutral tempo, in BPM.
pub const DEFAULT_BPM: f64 = 92.0;
/// Neutral harmonic tension (smooth↔tense).
pub const DEFAULT_TENSION: f32 = 0.30;
/// Neutral rhythmic density (sparse↔busy).
pub const DEFAULT_DENSITY: f32 = 0.55;
/// Neutral distortion amount (clean↔driven).
pub const DEFAULT_DRIVE: f32 = 0.40;
/// The most beats a bar may hold. Anything larger is not a musical request, and
/// would overflow the step-count arithmetic the plan layer does (R-0026).
pub(crate) const MAX_BEATS_PER_BAR: u32 = 32;
/// Slowest tempo a description may request, in BPM.
pub(crate) const MIN_BPM: f64 = 40.0;
/// Fastest tempo a description may request, in BPM.
pub(crate) const MAX_BPM: f64 = 250.0;
/// Beat units a meter may use (the "/4" in 6/8, 3/4, 7/8 …).
const VALID_UNITS: [u32; 5] = [1, 2, 4, 8, 16];

/// A time signature: `beats` per bar over a beat `unit` (6/8 is `{6, 8}`).
///
/// `Default` is 4/4 — note this is a **manual** impl: a derived one would give
/// the invalid `{0, 0}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Meter {
    /// Beats in a bar (the `6` in 6/8).
    pub beats: u32,
    /// The beat unit (the `8` in 6/8).
    pub unit: u32,
}

impl Default for Meter {
    fn default() -> Meter {
        Meter { beats: 4, unit: 4 }
    }
}

impl Meter {
    /// True when the meter is playable: a beat count the grid can lay out, over
    /// a power-of-two beat unit it understands.
    ///
    /// The upper bound on `beats` is not cosmetic. Downstream, a bar's step
    /// count is `beats · steps_per_beat` (R-0026), so an unbounded beat count
    /// overflows that multiply — reachable both from an ordinary prompt
    /// ("compás de 4000000000/4") and from a language model emitting
    /// `{"beats": 4000000000}`. Repairing it here keeps `normalized` the single
    /// choke point that guarantees a usable intent.
    pub fn is_valid(&self) -> bool {
        (1..=MAX_BEATS_PER_BAR).contains(&self.beats) && VALID_UNITS.contains(&self.unit)
    }
}

/// One named span of the song the description asked for ("intro", "hook").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionIntent {
    /// What the section is called.
    pub name: String,
    /// How many bars it lasts.
    pub bars: u32,
}

/// What a description asked for, in engine terms.
///
/// Produced by a [`crate::Parser`]; consumed by the preset library (R-0026) and
/// generation (R-0027). Container-level `serde(default)` means a partial JSON
/// (all an LM managed to fill in) leaves every other field neutral.
///
/// ```
/// use gooz_model::MusicalIntent;
///
/// let neutral = MusicalIntent::default();
/// assert_eq!(neutral.meter.beats, 4);
/// assert!(neutral.genre.is_empty());
///
/// // A partial description only moves what it mentions.
/// let partial: MusicalIntent = serde_json::from_str(r#"{"tempoBpm": 135}"#).unwrap();
/// assert_eq!(partial.tempo_bpm, 135.0);
/// assert_eq!(partial.meter, neutral.meter); // silent fields stay neutral
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MusicalIntent {
    /// Requested tempo in BPM.
    pub tempo_bpm: f64,
    /// Requested time signature.
    pub meter: Meter,
    /// Harmonic tension, `0..=1` (smooth ↔ tense) — drives ratio complexity.
    pub tension: f32,
    /// Rhythmic density, `0..=1` (sparse ↔ busy) — drives `E(k, n)` onset counts.
    pub density: f32,
    /// Distortion amount, `0..=1` (clean ↔ driven).
    pub drive: f32,
    /// Genre tags found in the description ("trap", "corrido", "metal").
    pub genre: Vec<String>,
    /// Mood tags found in the description ("dark", "spooky").
    pub mood: Vec<String>,
    /// Requested song structure, if the description gave one.
    pub structure: Vec<SectionIntent>,
}

impl Default for MusicalIntent {
    fn default() -> MusicalIntent {
        MusicalIntent {
            tempo_bpm: DEFAULT_BPM,
            meter: Meter::default(),
            tension: DEFAULT_TENSION,
            density: DEFAULT_DENSITY,
            drive: DEFAULT_DRIVE,
            genre: Vec::new(),
            mood: Vec::new(),
            structure: Vec::new(),
        }
    }
}

impl MusicalIntent {
    /// Returns the intent with every field forced into a usable range: sliders
    /// clamped to `0..=1`, tempo clamped to a playable BPM (non-finite → the
    /// default), an invalid meter reset to 4/4, and tags lowercased, trimmed,
    /// de-duplicated, and stripped of empties.
    ///
    /// Every parser ends here, so no source — rules, a language model, or hand
    /// -written JSON — can produce an intent the engine cannot use.
    ///
    /// ```
    /// use gooz_model::{Meter, MusicalIntent};
    ///
    /// let wild = MusicalIntent {
    ///     tempo_bpm: f64::NAN,
    ///     meter: Meter { beats: 0, unit: 7 }, // nonsense
    ///     tension: 9.0,
    ///     genre: vec!["Trap".into(), "trap".into(), "  ".into()],
    ///     ..MusicalIntent::default()
    /// };
    /// let ok = wild.normalized();
    /// assert_eq!(ok.meter, Meter::default()); // reset to 4/4
    /// assert_eq!(ok.tension, 1.0); // clamped
    /// assert_eq!(ok.genre, vec!["trap".to_string()]); // lowercased + deduped
    /// assert!(ok.tempo_bpm.is_finite());
    /// ```
    #[must_use]
    pub fn normalized(mut self) -> MusicalIntent {
        self.tempo_bpm = if self.tempo_bpm.is_finite() {
            self.tempo_bpm.clamp(MIN_BPM, MAX_BPM)
        } else {
            DEFAULT_BPM
        };
        if !self.meter.is_valid() {
            self.meter = Meter::default();
        }
        self.tension = clamp_unit(self.tension, DEFAULT_TENSION);
        self.density = clamp_unit(self.density, DEFAULT_DENSITY);
        self.drive = clamp_unit(self.drive, DEFAULT_DRIVE);
        self.genre = clean_tags(self.genre);
        self.mood = clean_tags(self.mood);
        self.structure
            .retain(|s| s.bars > 0 && !s.name.trim().is_empty());
        self
    }
}

/// Clamps a slider into `0..=1`, substituting `fallback` for a non-finite value.
fn clamp_unit(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        fallback
    }
}

/// Lowercases and trims tags, dropping empties and duplicates (order preserved).
fn clean_tags(tags: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(tags.len());
    for tag in tags {
        let tag = tag.trim().to_lowercase();
        if !tag.is_empty() && !out.contains(&tag) {
            out.push(tag);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_intent_is_neutral_and_playable() {
        let intent = MusicalIntent::default();
        assert_eq!(intent.tempo_bpm, DEFAULT_BPM);
        assert_eq!(intent.meter, Meter { beats: 4, unit: 4 });
        assert!(intent.meter.is_valid());
        assert!(intent.genre.is_empty() && intent.mood.is_empty());
    }

    #[test]
    fn derived_default_trap_is_avoided_for_meter() {
        // A derived Default would be {0, 0}; ours must be playable 4/4.
        assert!(Meter::default().is_valid());
    }

    #[test]
    fn normalize_clamps_sliders_and_tempo() {
        let intent = MusicalIntent {
            tempo_bpm: 5_000.0,
            tension: -3.0,
            density: f32::NAN,
            drive: 2.5,
            ..MusicalIntent::default()
        }
        .normalized();
        assert_eq!(intent.tempo_bpm, MAX_BPM);
        assert_eq!(intent.tension, 0.0);
        assert_eq!(intent.density, DEFAULT_DENSITY); // NaN → fallback
        assert_eq!(intent.drive, 1.0);
    }

    #[test]
    fn normalize_resets_an_invalid_meter() {
        for bad in [Meter { beats: 0, unit: 4 }, Meter { beats: 6, unit: 7 }] {
            let intent = MusicalIntent {
                meter: bad,
                ..MusicalIntent::default()
            }
            .normalized();
            assert_eq!(intent.meter, Meter::default());
        }
        // A valid compound meter survives.
        let six_eight = Meter { beats: 6, unit: 8 };
        let intent = MusicalIntent {
            meter: six_eight,
            ..MusicalIntent::default()
        }
        .normalized();
        assert_eq!(intent.meter, six_eight);
    }

    #[test]
    fn normalize_cleans_tags_and_sections() {
        let intent = MusicalIntent {
            genre: vec!["Trap".into(), " trap ".into(), "".into(), "Corrido".into()],
            structure: vec![
                SectionIntent {
                    name: "hook".into(),
                    bars: 8,
                },
                SectionIntent {
                    name: " ".into(),
                    bars: 4,
                }, // no name
                SectionIntent {
                    name: "outro".into(),
                    bars: 0,
                }, // no length
            ],
            ..MusicalIntent::default()
        }
        .normalized();
        assert_eq!(
            intent.genre,
            vec!["trap".to_string(), "corrido".to_string()]
        );
        assert_eq!(intent.structure.len(), 1);
        assert_eq!(intent.structure[0].name, "hook");
    }

    #[test]
    fn partial_json_leaves_other_fields_neutral() {
        let intent: MusicalIntent =
            serde_json::from_str(r#"{"tempoBpm": 135, "genre": ["trap"]}"#).expect("partial parse");
        assert_eq!(intent.tempo_bpm, 135.0);
        assert_eq!(intent.genre, vec!["trap".to_string()]);
        assert_eq!(intent.meter, Meter::default());
        assert_eq!(intent.tension, DEFAULT_TENSION);
    }

    #[test]
    fn round_trips_through_json() {
        let intent = MusicalIntent {
            tempo_bpm: 135.0,
            meter: Meter { beats: 6, unit: 8 },
            genre: vec!["corrido".into()],
            ..MusicalIntent::default()
        };
        let json = serde_json::to_string(&intent).expect("serialize");
        assert!(json.contains("\"tempoBpm\""), "camelCase for the frontend");
        assert_eq!(
            serde_json::from_str::<MusicalIntent>(&json).expect("deserialize"),
            intent
        );
    }
}
