//! Genre presets: [`MusicalIntent`] → [`SoundPlan`] (R-0026 / SPEC-0026).
//!
//! Presets are **data, not model weights** — a reviewable table anyone can
//! extend without retraining, keeping the description→sound path deterministic.
//! The plan is deliberately engine-agnostic (ratio and rhythm primitives only),
//! so this crate never learns the app's synth types; the app adapts the plan to
//! its own beat config.

use serde::{Deserialize, Serialize};

use crate::intent::{Meter, MusicalIntent};

/// The harmonic-series odd-limit walked by the smooth↔tense control.
const TENSE_MIN_ODD: u64 = 3;
const TENSE_MAX_ODD: u64 = 15;

/// Which kit role a lane plays. Mirrors the drum kit without importing synth
/// types (which live in a crate that depends on this one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceRole {
    /// The low drum.
    Kick,
    /// The backbeat drum.
    Snare,
    /// The cymbal lane.
    Hat,
}

/// One drum lane, ratio-native: `E(onsets, steps)` rotated, played at a level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoicePlan {
    /// Which kit role this lane is.
    pub role: VoiceRole,
    /// Euclidean onset count `k`.
    pub onsets: u32,
    /// Euclidean step count `n` (one bar).
    pub steps: u32,
    /// Cyclic rotation, in steps — how the pattern's accents are placed.
    pub rotate: i64,
    /// Lane level in `[0, 1]`.
    pub level: f32,
}

/// A description made concrete: what to play, in engine terms.
///
/// Serializable and inspectable for the same reason [`MusicalIntent`] is — the
/// UI can show "this is what I'm going to play" and let the user adjust it
/// before anything is rendered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundPlan {
    /// Tempo in BPM.
    pub tempo_bpm: f64,
    /// Time signature the rhythm is laid out in.
    pub meter: Meter,
    /// Harmonic-series odd-limit for the pitch grid (from `tension`).
    pub odd_limit: u64,
    /// Distortion amount in `[0, 1]`, passed through from the intent.
    pub drive: f32,
    /// The drum lanes to play.
    pub voices: Vec<VoicePlan>,
    /// Which preset produced this plan (`"trap"`, `"corrido"`, `"free"`, …).
    pub preset: String,
}

/// A lane's shape within a preset: how sparse/busy it can get, where its accent
/// sits (in beats), and how loud it plays.
struct Lane {
    /// Onset count at `density = 0`, as a fraction of the bar's steps.
    min: f32,
    /// Onset count at `density = 1`, as a fraction of the bar's steps.
    max: f32,
    /// Accent offset in beats (a snare on beat 3 is `2.0`).
    rotate_beats: f32,
    /// Lane level in `[0, 1]`.
    level: f32,
}

/// A genre's rhythmic shape. Selected by tag, then modulated by the intent.
struct Preset {
    name: &'static str,
    /// Intent genre tags that select this preset.
    tags: &'static [&'static str],
    /// Grid resolution: a bar has `meter.beats * steps_per_beat` steps.
    steps_per_beat: u32,
    kick: Lane,
    snare: Lane,
    hat: Lane,
}

/// The preset library. Order is priority: the first entry whose tags the intent
/// mentions wins. `free` is the fallback and matches nothing by tag.
const PRESETS: &[Preset] = &[
    Preset {
        name: "corrido",
        tags: &["corrido", "tumbado", "bélico", "belico", "regional"],
        steps_per_beat: 2, // eighth-note grid: 6/8 → 12 steps
        kick: Lane {
            min: 0.15,
            max: 0.35,
            rotate_beats: 0.0,
            level: 1.0,
        },
        // The corrido accent: snare on beat 3.
        snare: Lane {
            min: 0.08,
            max: 0.20,
            rotate_beats: 2.0,
            level: 0.9,
        },
        hat: Lane {
            min: 0.40,
            max: 1.00,
            rotate_beats: 0.0,
            level: 0.6,
        },
    },
    Preset {
        name: "trap",
        tags: &["trap", "drill"],
        steps_per_beat: 4, // sixteenth grid
        kick: Lane {
            min: 0.12,
            max: 0.30,
            rotate_beats: 0.0,
            level: 1.0,
        },
        // Half-time backbeat: the snare lands on beat 3 of a 4/4 bar.
        snare: Lane {
            min: 0.06,
            max: 0.15,
            rotate_beats: 2.0,
            level: 0.9,
        },
        hat: Lane {
            min: 0.35,
            max: 1.00,
            rotate_beats: 0.0,
            level: 0.6,
        },
    },
    Preset {
        name: "metal",
        tags: &["metal", "black metal", "punk", "rock"],
        steps_per_beat: 4,
        kick: Lane {
            min: 0.25,
            max: 0.60,
            rotate_beats: 0.0,
            level: 1.0,
        },
        snare: Lane {
            min: 0.12,
            max: 0.25,
            rotate_beats: 1.0,
            level: 0.9,
        },
        hat: Lane {
            min: 0.50,
            max: 1.00,
            rotate_beats: 0.0,
            level: 0.6,
        },
    },
    Preset {
        name: "free",
        tags: &[], // fallback only
        steps_per_beat: 4,
        kick: Lane {
            min: 0.12,
            max: 0.50,
            rotate_beats: 0.0,
            level: 1.0,
        },
        snare: Lane {
            min: 0.12,
            max: 0.25,
            rotate_beats: 1.0,
            level: 0.9,
        },
        hat: Lane {
            min: 0.25,
            max: 1.00,
            rotate_beats: 0.0,
            level: 0.6,
        },
    },
];

/// Turns an intent into a concrete, playable plan.
///
/// Total: a normalized [`MusicalIntent`] can always be planned, so there is no
/// error case. An unrecognized or absent genre falls back to the neutral `free`
/// preset (the honesty rule — an unknown description still plays).
///
/// ```
/// use gooz_model::{parse_intent, plan_sound};
///
/// let plan = plan_sound(&parse_intent("corrido tumbado en 6/8 a 135 bpm"));
/// assert_eq!(plan.preset, "corrido");
/// assert_eq!(plan.tempo_bpm, 135.0);
/// // A 6/8 bar is laid out so every beat gets whole steps.
/// assert_eq!(plan.steps() % plan.meter.beats, 0);
///
/// // Nothing recognizable still yields a playable plan.
/// let neutral = plan_sound(&Default::default());
/// assert_eq!(neutral.preset, "free");
/// assert!(neutral.voices.iter().all(|v| v.onsets > 0 && v.onsets <= v.steps));
/// ```
pub fn plan_sound(intent: &MusicalIntent) -> SoundPlan {
    let preset = select_preset(&intent.genre);
    let steps = (intent.meter.beats * preset.steps_per_beat).max(1);
    let plan_lane = |lane: &Lane, role: VoiceRole| VoicePlan {
        role,
        onsets: onsets_for(lane, intent.density, steps),
        steps,
        rotate: (lane.rotate_beats * preset.steps_per_beat as f32).round() as i64,
        level: lane.level.clamp(0.0, 1.0),
    };

    SoundPlan {
        tempo_bpm: intent.tempo_bpm,
        meter: intent.meter,
        odd_limit: odd_limit_for(intent.tension),
        drive: intent.drive.clamp(0.0, 1.0),
        voices: vec![
            plan_lane(&preset.kick, VoiceRole::Kick),
            plan_lane(&preset.snare, VoiceRole::Snare),
            plan_lane(&preset.hat, VoiceRole::Hat),
        ],
        preset: preset.name.to_string(),
    }
}

impl SoundPlan {
    /// The bar's step resolution (every lane shares it).
    pub fn steps(&self) -> u32 {
        self.voices.first().map_or(0, |v| v.steps)
    }

    /// Total onsets across every lane — the plan's overall busyness.
    pub fn total_onsets(&self) -> u32 {
        self.voices.iter().map(|v| v.onsets).sum()
    }
}

/// Picks the first preset the intent's genre tags mention, else `free`.
fn select_preset(genre: &[String]) -> &'static Preset {
    PRESETS
        .iter()
        .find(|p| p.tags.iter().any(|tag| genre.iter().any(|g| g == tag)))
        .unwrap_or_else(|| {
            PRESETS
                .last()
                .expect("the preset table always holds the `free` fallback")
        })
}

/// Scales a lane's onset count by density, always leaving a playable pattern.
fn onsets_for(lane: &Lane, density: f32, steps: u32) -> u32 {
    let fraction = lane.min + (lane.max - lane.min) * density.clamp(0.0, 1.0);
    ((fraction * steps as f32).round() as u32).clamp(1, steps)
}

/// Maps smooth↔tense onto the harmonic-series odd-limit: `0` is the simplest
/// grid, `1` the densest, stepping through the odd harmonics between.
pub fn odd_limit_for(tension: f32) -> u64 {
    let t = f64::from(tension.clamp(0.0, 1.0));
    let rungs = (TENSE_MAX_ODD - TENSE_MIN_ODD) / 2;
    TENSE_MIN_ODD + 2 * (t * rungs as f64).round() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_intent;

    /// The owner's north-star prompt (R-0026 AC6), trimmed to its musical content.
    const NORTH_STAR: &str = "Pon el tempo a 135 BPM. La batería trap + tumbado en un compás de \
         6/8, snare seco en el tercer tiempo, y satura los hi-hats para que hagan tresillos \
         rápidos. El bajo: un 808 largo con distorsión hasta que cruje. La guitarra: black metal \
         + requinto, segundas menores para dar tensión.";

    fn intent_with(density: f32, tension: f32, genre: &[&str]) -> MusicalIntent {
        MusicalIntent {
            density,
            tension,
            genre: genre.iter().map(|g| (*g).to_string()).collect(),
            ..MusicalIntent::default()
        }
    }

    #[test]
    fn ac1_plan_is_deterministic() {
        let intent = parse_intent(NORTH_STAR);
        assert_eq!(plan_sound(&intent), plan_sound(&intent));
    }

    #[test]
    fn ac2_genre_tags_select_a_preset() {
        assert_eq!(plan_sound(&intent_with(0.5, 0.3, &["trap"])).preset, "trap");
        assert_eq!(
            plan_sound(&intent_with(0.5, 0.3, &["tumbado"])).preset,
            "corrido"
        );
        assert_eq!(
            plan_sound(&intent_with(0.5, 0.3, &["black metal"])).preset,
            "metal"
        );
    }

    #[test]
    fn ac2_unknown_or_empty_genre_falls_back_to_free() {
        assert_eq!(plan_sound(&intent_with(0.5, 0.3, &[])).preset, "free");
        assert_eq!(
            plan_sound(&intent_with(0.5, 0.3, &["polka-espacial"])).preset,
            "free"
        );
    }

    #[test]
    fn ac3_density_never_lowers_the_onset_count() {
        let mut previous = 0;
        for step in 0..=10 {
            let density = step as f32 / 10.0;
            let total = plan_sound(&intent_with(density, 0.3, &["trap"])).total_onsets();
            assert!(
                total >= previous,
                "density {density} lowered onsets: {total} < {previous}"
            );
            previous = total;
        }
    }

    #[test]
    fn ac3_tension_never_lowers_the_odd_limit_and_drive_passes_through() {
        let mut previous = 0;
        for step in 0..=10 {
            let tension = step as f32 / 10.0;
            let limit = plan_sound(&intent_with(0.5, tension, &["trap"])).odd_limit;
            assert!(limit >= previous, "tension {tension} lowered the odd-limit");
            assert!((TENSE_MIN_ODD..=TENSE_MAX_ODD).contains(&limit));
            assert_eq!(limit % 2, 1, "the odd-limit stays odd");
            previous = limit;
        }
        let driven = MusicalIntent {
            drive: 0.87,
            ..MusicalIntent::default()
        };
        assert_eq!(plan_sound(&driven).drive, 0.87);
    }

    #[test]
    fn ac4_steps_follow_the_meter_so_accents_land_on_beats() {
        let six_eight = MusicalIntent {
            meter: Meter { beats: 6, unit: 8 },
            genre: vec!["corrido".into()],
            ..MusicalIntent::default()
        };
        let plan = plan_sound(&six_eight);
        assert_eq!(plan.steps(), 12, "6 beats × 2 steps per beat");
        assert_eq!(plan.steps() % plan.meter.beats, 0);
        // The corrido snare accent sits on beat 3 → 2 beats in, 2 steps per beat.
        let snare = plan
            .voices
            .iter()
            .find(|v| v.role == VoiceRole::Snare)
            .expect("a snare lane");
        assert_eq!(snare.rotate, 4);
    }

    #[test]
    fn ac5_every_plan_is_playable() {
        for density in [0.0, 0.5, 1.0] {
            for tension in [0.0, 0.5, 1.0] {
                for genre in [&[][..], &["trap"], &["corrido"], &["metal"]] {
                    let plan = plan_sound(&intent_with(density, tension, genre));
                    for voice in &plan.voices {
                        assert!(voice.steps > 0);
                        assert!(
                            voice.onsets > 0 && voice.onsets <= voice.steps,
                            "0 < k <= n violated: {voice:?}"
                        );
                        assert!((0.0..=1.0).contains(&voice.level));
                    }
                    assert!((0.0..=1.0).contains(&plan.drive));
                }
            }
        }
    }

    #[test]
    fn ac5_a_neutral_intent_plans_neutral_defaults() {
        let plan = plan_sound(&MusicalIntent::default());
        assert_eq!(plan.preset, "free");
        assert_eq!(plan.meter, Meter::default());
        assert_eq!(plan.tempo_bpm, crate::intent::DEFAULT_BPM);
    }

    #[test]
    fn ac6_north_star_prompt_plans_the_described_song() {
        let plan = plan_sound(&parse_intent(NORTH_STAR));
        assert_eq!(plan.tempo_bpm, 135.0);
        assert_eq!((plan.meter.beats, plan.meter.unit), (6, 8));
        // "trap + tumbado" — the corrido preset wins (table priority) and lays
        // the bar out in the compound meter.
        assert_eq!(plan.preset, "corrido");
        assert_eq!(plan.steps() % plan.meter.beats, 0);
        // Saturated hats: the busiest lane, and tense/driven from the cues.
        let hat = plan
            .voices
            .iter()
            .find(|v| v.role == VoiceRole::Hat)
            .expect("a hat lane");
        let kick = plan
            .voices
            .iter()
            .find(|v| v.role == VoiceRole::Kick)
            .expect("a kick lane");
        assert!(hat.onsets > kick.onsets, "hats are the busy lane");
        assert!(plan.odd_limit > TENSE_MIN_ODD, "minor seconds read tense");
        assert!(plan.drive > 0.5, "distortion reads driven");
    }

    #[test]
    fn plan_round_trips_through_json() {
        let plan = plan_sound(&parse_intent(NORTH_STAR));
        let json = serde_json::to_string(&plan).expect("serialize");
        assert!(json.contains("\"tempoBpm\""), "camelCase for the frontend");
        assert_eq!(
            serde_json::from_str::<SoundPlan>(&json).expect("deserialize"),
            plan
        );
    }
}
