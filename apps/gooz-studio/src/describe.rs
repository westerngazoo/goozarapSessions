//! Description → sound (R-0027 / SPEC-0027).
//!
//! The last leg of M7: a [`SoundPlan`] becomes audible — a beat through the
//! R-0009 builder and a melody generated on the plan's harmonic grid, rendered
//! by the R-0007 instrument. [`describe_song`] runs the whole chain, so a text
//! description turns into a playable song in one call.
//!
//! Generation is integration, which is why it lives here: this is the only crate
//! that may depend on `gooz-model` (the plan), `gooz-dsp` (notes and grids), and
//! `gooz-synth` (rendering) at once. Everything is deterministic — the
//! description biases the math, it never replaces it.

use serde::Serialize;

use gooz_dsp::{PitchGrid, QuantizedNote, Tempo};
use gooz_model::{SoundPlan, VoiceRole, parse_intent, plan_sound};
use gooz_synth::{Pattern, RenderConfig, render_notes};

use crate::DrumKind;
use crate::beat::{BeatConfig, BeatStem, BeatVoiceSpec, build_beat};
use crate::view::{BeatView, GRID_ROOT_HZ, NoteView, RiffView, WAVE_BUCKETS, peak_envelope};

/// The sample rate generated songs are rendered at.
const SAMPLE_RATE: u32 = 48_000;
/// The odd-limit range `tension` walks (mirrors the preset layer, R-0026).
const MIN_ODD: f64 = 3.0;
const MAX_ODD: f64 = 15.0;
/// A melody is sparser than a hat lane: onset count as a fraction of the bar's
/// steps at `density = 0` and `density = 1`.
const MELODY_MIN: f32 = 0.10;
const MELODY_MAX: f32 = 0.35;
/// Steps through the reachable degrees per note — coprime-ish, so the line
/// leaps instead of running up the grid.
const CONTOUR_STRIDE: usize = 3;
/// Lift the line an octave every this many notes, to give it shape.
const OCTAVE_EVERY: usize = 5;
/// How far `drive` pushes the renderer past its clean setting.
const DRIVE_RANGE: f32 = 3.0;

/// A description turned into sound: what was understood, and what it sounds like.
///
/// The plan travels with the audio so the UI (R-0029) can show *what it
/// understood* beside *what it made*, and the user can edit the plan and
/// re-render — the same "return what it heard" contract as the hum→riff pipeline.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribedSong {
    /// What the description became (inspectable and editable).
    pub plan: SoundPlan,
    /// The generated melody, rendered.
    pub riff: RiffView,
    /// The generated beat, rendered.
    pub beat: BeatView,
}

/// Turns a text description into a playable song.
///
/// Parses the description (R-0025), plans it against the genre presets
/// (R-0026), then renders both parts. Total — an empty or unrecognized
/// description still yields a playable song from neutral defaults.
///
/// ```
/// use gooz_studio::describe_song;
///
/// let song = describe_song("corrido tumbado en 6/8 a 135 bpm, distorsionado", 2);
/// assert_eq!(song.plan.preset, "corrido");
/// assert!(!song.beat.samples.is_empty());
/// assert!(!song.riff.notes.is_empty());
/// ```
pub fn describe_song(prompt: &str, bars: u32) -> DescribedSong {
    song_from_plan(&plan_sound(&parse_intent(prompt)), bars)
}

/// Renders a [`SoundPlan`] into a song, without re-parsing — the entry point for
/// a plan the user edited by hand.
pub fn song_from_plan(plan: &SoundPlan, bars: u32) -> DescribedSong {
    let bars = bars.max(1);
    let tempo = tempo_of(plan);
    let specs = beat_specs_from(plan);
    let cfg = BeatConfig {
        voices: specs.clone(),
        bars,
    };
    // `build_beat` only rejects an invalid `E(k, n)`, which R-0026 guarantees
    // the plan never contains; an empty stem is the graceful worst case.
    let stem = build_beat(&tempo, SAMPLE_RATE, &cfg).unwrap_or(BeatStem {
        samples: Vec::new(),
        sample_rate: SAMPLE_RATE,
        bars: 0,
    });

    let notes = melody_notes(plan, bars);
    let audio = render_notes(&notes, SAMPLE_RATE, &render_config_for(plan));

    DescribedSong {
        plan: plan.clone(),
        riff: riff_view_of(audio, &notes, bars),
        beat: BeatView::from_stem(&stem, &specs),
    }
}

/// Maps the plan's lanes onto the beat builder's config (AC1).
fn beat_specs_from(plan: &SoundPlan) -> Vec<BeatVoiceSpec> {
    plan.voices
        .iter()
        .map(|voice| BeatVoiceSpec {
            kind: match voice.role {
                VoiceRole::Kick => DrumKind::Kick,
                VoiceRole::Snare => DrumKind::Snare,
                VoiceRole::Hat => DrumKind::HiHat,
            },
            onsets: voice.onsets,
            steps: voice.steps,
            rotate: voice.rotate,
            level: voice.level,
        })
        .collect()
}

/// Generates the melody: a Euclidean rhythm whose pitches walk the plan's
/// harmonic grid, ordered by ratio complexity (AC2).
///
/// The plan's `tension` (recovered from its odd-limit) decides how deep into
/// that ordering the walk may reach, so "tense" literally means "uses more
/// complex ratios".
pub fn melody_notes(plan: &SoundPlan, bars: u32) -> Vec<QuantizedNote> {
    let Ok(grid) = PitchGrid::harmonic(GRID_ROOT_HZ, plan.odd_limit) else {
        return Vec::new();
    };
    let steps = plan.steps().max(1);
    let Ok(pattern) = Pattern::euclidean(melody_onsets(plan, steps), steps) else {
        return Vec::new();
    };

    // Degrees the line may use, simplest first; tension opens the window.
    let mut degrees: Vec<_> = grid.degrees().to_vec();
    degrees.sort_by(|a, b| {
        a.complexity()
            .partial_cmp(&b.complexity())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let window = reachable_degrees(plan.odd_limit, degrees.len());
    let degrees = &degrees[..window];

    let step_secs = tempo_of(plan).bar_seconds() / f64::from(steps);
    let onsets = pattern.onsets();
    let stride = contour_stride(degrees.len());
    let mut notes = Vec::with_capacity(onsets.len() * bars as usize);
    for bar in 0..bars {
        for (i, step) in onsets.iter().enumerate() {
            let index = i * stride;
            let degree = degrees[index % degrees.len()];
            let octave = ((index / OCTAVE_EVERY) % 2) as i32;
            let Ok(base_hz) = degree.to_hz(GRID_ROOT_HZ) else {
                continue;
            };
            let global_step = u64::from(bar) * u64::from(steps) + *step as u64;
            notes.push(QuantizedNote {
                degree,
                octave,
                freq_hz: base_hz * f64::from(1u32 << octave),
                cents_offset: 0.0, // generated on-grid: nothing to correct
                onset_step: global_step,
                onset_secs: global_step as f64 * step_secs,
                duration_secs: step_secs,
            });
        }
    }
    notes
}

/// The step taken through the reachable degrees per note.
///
/// [`CONTOUR_STRIDE`] only visits every degree when it is coprime with the
/// window — with a window of 3 a stride of 3 would repeat one degree forever —
/// so fall back to a step of 1 when it is not.
fn contour_stride(window: usize) -> usize {
    if window > 1 && gcd(CONTOUR_STRIDE, window) == 1 {
        CONTOUR_STRIDE
    } else {
        1
    }
}

/// Greatest common divisor, for the coprime check above.
fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// How many grid degrees the melody may reach, from the plan's odd-limit.
fn reachable_degrees(odd_limit: u64, available: usize) -> usize {
    if available == 0 {
        return 0;
    }
    let tension = ((odd_limit as f64 - MIN_ODD) / (MAX_ODD - MIN_ODD)).clamp(0.0, 1.0);
    let span = (available - 1) as f64;
    (1 + (tension * span).round() as usize).min(available)
}

/// The melody's onset count for the plan's density.
fn melody_onsets(plan: &SoundPlan, steps: u32) -> u32 {
    // Density is not carried on the plan directly; the hat lane is the busiest
    // voice, so its fill ratio stands in for how busy the description asked to be.
    let density = plan
        .voices
        .iter()
        .find(|v| v.role == VoiceRole::Hat)
        .map_or(0.5, |hat| hat.onsets as f32 / hat.steps.max(1) as f32);
    let fraction = MELODY_MIN + (MELODY_MAX - MELODY_MIN) * density.clamp(0.0, 1.0);
    ((fraction * steps as f32).round() as u32).clamp(1, steps)
}

/// The plan's tempo as a beat-grid tempo. Falls back to Easy Mode's default when
/// the plan asks for something the grid rejects (it never should — R-0026 AC5).
fn tempo_of(plan: &SoundPlan) -> Tempo {
    Tempo::new(plan.tempo_bpm, f64::from(plan.meter.beats))
        .or_else(|_| Tempo::new(92.0, 4.0))
        .expect("92 BPM / 4 beats-per-bar is always valid")
}

/// The renderer settings a plan asks for — `drive` scales the distortion.
fn render_config_for(plan: &SoundPlan) -> RenderConfig {
    RenderConfig {
        drive: 1.0 + plan.drive.clamp(0.0, 1.0) * DRIVE_RANGE,
        ..RenderConfig::default()
    }
}

/// Packages rendered melody audio for the UI, mirroring the hum→riff view.
fn riff_view_of(samples: Vec<f32>, notes: &[QuantizedNote], bars: u32) -> RiffView {
    let seconds = samples.len() as f64 / f64::from(SAMPLE_RATE);
    RiffView {
        sample_rate: SAMPLE_RATE,
        bars: if samples.is_empty() { 0 } else { bars },
        seconds,
        notes: notes
            .iter()
            .map(|n| NoteView {
                num: n.degree.num(),
                den: n.degree.den(),
                octave: n.octave,
                hz: n.freq_hz,
                cents: n.cents_offset,
            })
            .collect(),
        wave: peak_envelope(&samples, WAVE_BUCKETS),
        samples,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gooz_model::{MusicalIntent, plan_sound};

    const NORTH_STAR: &str = "Pon el tempo a 135 BPM. La batería trap + tumbado en un compás de \
         6/8, snare seco en el tercer tiempo, y satura los hi-hats con tresillos rápidos. El bajo: \
         un 808 largo con distorsión hasta que cruje. La guitarra: black metal, segundas menores.";

    fn plan_with(density: f32, tension: f32) -> SoundPlan {
        plan_sound(&MusicalIntent {
            density,
            tension,
            genre: vec!["trap".into()],
            ..MusicalIntent::default()
        })
    }

    #[test]
    fn ac1_plan_maps_onto_the_beat_and_renders() {
        let song = describe_song(NORTH_STAR, 2);
        let plan_hat = song
            .plan
            .voices
            .iter()
            .find(|v| v.role == VoiceRole::Hat)
            .expect("a hat lane");
        let view_hat = song
            .beat
            .voices
            .iter()
            .find(|v| v.name == "hat")
            .expect("a hat lane in the view");
        assert_eq!(
            (view_hat.onsets, view_hat.steps),
            (plan_hat.onsets, plan_hat.steps),
            "the lane survives the mapping unchanged"
        );
        assert!(!song.beat.samples.is_empty(), "the beat renders");
        assert!(song.beat.bars >= 1, "the beat is bar-aligned");
    }

    #[test]
    fn ac2_melody_pitches_come_from_the_plans_grid() {
        let plan = plan_with(0.5, 0.5);
        let grid = PitchGrid::harmonic(GRID_ROOT_HZ, plan.odd_limit).expect("a valid grid");
        let notes = melody_notes(&plan, 1);
        assert!(!notes.is_empty(), "a melody is generated");
        for note in &notes {
            assert!(
                grid.degrees().contains(&note.degree),
                "{:?} is not a grid degree",
                note.degree
            );
            assert!(
                note.onset_step < u64::from(plan.steps()),
                "onsets stay in bar"
            );
        }
    }

    #[test]
    fn ac3_density_never_lowers_the_note_count() {
        let mut previous = 0;
        for step in 0..=10 {
            let count = melody_notes(&plan_with(step as f32 / 10.0, 0.5), 1).len();
            assert!(count >= previous, "density lowered the note count");
            previous = count;
        }
    }

    #[test]
    fn ac3_tension_never_shrinks_the_reachable_ratios() {
        let mut previous = 0;
        for step in 0..=10 {
            let plan = plan_with(1.0, step as f32 / 10.0);
            let distinct: std::collections::BTreeSet<_> = melody_notes(&plan, 1)
                .iter()
                .map(|n| (n.degree.num(), n.degree.den()))
                .collect();
            assert!(distinct.len() >= previous, "tension shrank the ratio set");
            previous = distinct.len();
        }
    }

    #[test]
    fn ac3_drive_reaches_the_renderer() {
        let clean = render_config_for(&plan_sound(&MusicalIntent {
            drive: 0.0,
            ..MusicalIntent::default()
        }));
        let dirty = render_config_for(&plan_sound(&MusicalIntent {
            drive: 1.0,
            ..MusicalIntent::default()
        }));
        assert!(dirty.drive > clean.drive);
    }

    #[test]
    fn ac4_a_description_becomes_a_song() {
        let song = describe_song(NORTH_STAR, 2);
        assert_eq!(song.plan.tempo_bpm, 135.0);
        assert!(!song.riff.samples.is_empty() && !song.beat.samples.is_empty());
        assert!(!song.riff.notes.is_empty());
    }

    #[test]
    fn ac5_generation_is_deterministic() {
        assert_eq!(describe_song(NORTH_STAR, 2), describe_song(NORTH_STAR, 2));
    }

    #[test]
    fn ac6_audio_is_bounded_and_an_empty_description_still_plays() {
        for prompt in [NORTH_STAR, "", "asdf"] {
            let song = describe_song(prompt, 1);
            for samples in [&song.riff.samples, &song.beat.samples] {
                assert!(
                    samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
                    "audio left [-1, 1] for prompt {prompt:?}"
                );
            }
            assert!(
                !song.beat.samples.is_empty(),
                "prompt {prompt:?} still plays"
            );
        }
    }
}
