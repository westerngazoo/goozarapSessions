//! [`accompany_take`] — sing, pick a style, hear yourself over a track in that
//! style: at your tempo, entering on a downbeat. Realizes R-0042 / SPEC-0042.
//!
//! The voice and the track come back as a [`RiffView`] and a [`BeatView`]
//! because those are what the studio already plays, saves and exports. That
//! only works if the two agree on three things, which this module guarantees
//! and its tests pin: **the same sample rate, the same length, and a whole
//! number of bars** of 4/4.

use gooz_dsp::{DspError, analyze, first_sung_note, follow, quantize_notes};
use gooz_model::{Meter, SoundPlan, parse_intent, plan_sound};
use serde::Serialize;

use crate::describe::{beat_from_plan, plan_tempo};
use crate::pipeline::{PipelineConfig, bar_samples, pad_to_bars};
use crate::view::{BeatView, NoteView, Part, RiffView, WAVE_BUCKETS, followed_grid, peak_envelope};

/// The level the voice is brought to: −1 dBFS at its peak.
///
/// The drums are normalized to full scale; a laptop take peaking at −20 dBFS
/// otherwise sits about 19 dB under them.
const VOICE_PEAK: f32 = 0.891_251;

/// Fades at both ends of the take, in seconds: it starts wherever recording
/// began and ends wherever the user tapped stop, and either cut can click.
const EDGE_FADE_SECS: f64 = 0.005;

/// Your take, with a track in a style, at your tempo.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Accompaniment {
    /// What the style became, on the clock it was rendered at.
    pub plan: SoundPlan,
    /// The take, delayed so its first sung note enters on a downbeat after a
    /// bar of drums. Its notes are what was sung; its `followed_*` fields say
    /// which of tempo and key came from the singer.
    pub voice: RiffView,
    /// The styled drums: same rate, same length.
    pub track: BeatView,
}

/// Puts a sung take over a drum track in `style`.
///
/// - **The clock** is the take's tempo when [`follow`] heard one, and the
///   style's own tempo ([`SoundPlan::style_bpm`]) when it did not. The style
///   *text* is never read for a tempo: the studio's style chips send a preset
///   name and no number, and `MusicalIntent` cannot tell a stated 92 BPM from
///   its default one.
/// - **The meter** is 4/4 whatever the style text says (R-0042 §4).
/// - **The entry** is a count-in: the voice is *delayed*, never trimmed, so its
///   first sung note lands on the downbeat of the bar after the one it started
///   in. The anchor is the first transcribed *note*, not the first onset — an
///   onset fires on room noise at t = 0 and on a breath before the note.
///
/// `tense` sets the grid the note cards are shown on, as in the other modes.
///
/// # Errors
///
/// - [`DspError::EmptySignal`], [`DspError::InvalidSampleRate`];
/// - [`DspError::NonFiniteSample`] or [`DspError::SampleOutOfRange`] for any
///   sample of the take that is not audio;
/// - [`DspError::WindowTooLarge`] for a take shorter than the analysis window;
/// - [`DspError::Silent`] when nothing in the take is a sung note.
pub fn accompany_take(
    samples: &[f32],
    sample_rate: u32,
    style: &str,
    tense: u8,
) -> Result<Accompaniment, DspError> {
    if samples.is_empty() {
        return Err(DspError::EmptySignal);
    }
    if sample_rate == 0 {
        return Err(DspError::InvalidSampleRate);
    }
    if samples.iter().any(|s| !s.is_finite()) {
        return Err(DspError::NonFiniteSample);
    }
    if samples.iter().any(|s| s.abs() > 1.0) {
        return Err(DspError::SampleOutOfRange);
    }

    let cfg = PipelineConfig::default();
    let transcription = analyze(samples, sample_rate, &cfg.analyze)?;
    let anchor = first_sung_note(samples, sample_rate, &transcription, &cfg.analyze)
        .ok_or(DspError::Silent)?;
    let heard = follow(samples, sample_rate, &transcription);

    let plan = styled_plan(style, heard.bpm);
    let tempo = plan_tempo(&plan);
    let bar = bar_samples(&tempo, sample_rate);

    let entry = one_bar_in(anchor, bar);
    let mut voice = vec![0.0; entry.pad];
    voice.extend(level_and_fade(&samples[entry.kept_from..], sample_rate));
    let bars = pad_to_bars(&mut voice, bar);

    let grid = followed_grid(tense, &heard);
    let notes = quantize_notes(&transcription.notes, &grid, &tempo, cfg.subdivision);

    Ok(Accompaniment {
        track: beat_from_plan(&plan, bars, sample_rate),
        voice: RiffView {
            sample_rate,
            bars,
            seconds: voice.len() as f64 / f64::from(sample_rate),
            notes: notes.iter().map(NoteView::from).collect(),
            wave: peak_envelope(&voice, WAVE_BUCKETS),
            samples: voice,
            followed_bpm: heard.bpm,
            followed_root_hz: heard.root_hz,
            bpm: tempo.bpm(),
            beats_per_bar: tempo.beats_per_bar(),
            part: Part::Voice,
        },
        plan,
    })
}

/// The style's lanes in 4/4, on the voice's clock or the style's own.
fn styled_plan(style: &str, followed_bpm: Option<f64>) -> SoundPlan {
    let mut intent = parse_intent(style);
    intent.meter = Meter::default();
    let mut plan = plan_sound(&intent.normalized());
    plan.tempo_bpm = followed_bpm.unwrap_or(plan.style_bpm);
    plan
}

/// How the take is re-timed so its first sung note enters on a downbeat.
struct Entry {
    /// Where in the take the voice begins: whole bars of lead-in before the
    /// note's own bar are dropped.
    kept_from: usize,
    /// How much silence goes before it.
    pad: usize,
}

/// Exactly one bar of drums before the singer, however long they waited to
/// start (owner decision, R-0042): whole bars of lead-in before the note's own
/// bar are dropped — silence and room, which only ever lengthened the loop —
/// and the rest is delayed by the remainder of a bar, so the note lands on the
/// downbeat of bar 2. Only whole bars are dropped, so the phase is untouched.
///
/// `bar` is at least one sample (`bar_samples` guarantees it).
fn one_bar_in(anchor: usize, bar: usize) -> Entry {
    let kept_from = anchor / bar * bar;
    Entry {
        kept_from,
        pad: bar - (anchor - kept_from),
    }
}

/// The take brought to [`VOICE_PEAK`], with [`EDGE_FADE_SECS`] fades.
fn level_and_fade(take: &[f32], sample_rate: u32) -> Vec<f32> {
    let peak = take.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let gain = if peak > 0.0 { VOICE_PEAK / peak } else { 0.0 };
    let mut voice: Vec<f32> = take.iter().map(|s| s * gain).collect();
    let fade = ((EDGE_FADE_SECS * f64::from(sample_rate)) as usize).min(voice.len() / 2);
    for (i, sample) in voice.iter_mut().take(fade).enumerate() {
        *sample *= i as f32 / fade as f32;
    }
    for (i, sample) in voice.iter_mut().rev().take(fade).enumerate() {
        *sample *= i as f32 / fade as f32;
    }
    voice
}
