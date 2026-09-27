//! [`instrument_from_take`] — play a recorded take across the ratio grid.
//!
//! R-0039 made any recording playable; this is the wire that reaches it. The
//! sound inside a take becomes a [`Sampler`], the grid's degrees become a
//! rising figure played through it, and the result arrives as an ordinary
//! [`RiffView`] — so the shell's existing playback, waveform, save, and export
//! paths carry it unchanged. Realizes R-0040 / SPEC-0040.

use gooz_dsp::{DspError, PitchGrid, QuantizedNote, Ratio, Tempo, sound_span};
use gooz_synth::{Distortion, RenderConfig, Sampler, render_sampled_notes};

use crate::pipeline::{bar_samples, pad_to_bars};
use crate::view::{
    NoteView, Part, RiffView, WAVE_BUCKETS, easy_mode_grid, easy_mode_tempo, peak_envelope,
};

/// The octave the recording sits at: degree `1:1` plays it exactly as recorded.
///
/// Stated rather than inferred. A [`QuantizedNote`]'s octave counts octaves
/// above the *grid's* root, so leaving it implicit is what let a song's root
/// setting silently transpose a sampled part (R-0039's decision log).
const TAKE_OCTAVE: i32 = 0;

/// Fade applied where the sound is cut in, in seconds. The cut lands inside the
/// pre-roll, which is quiet, so this only has to stop a step from clicking.
const FADE_IN_SECS: f64 = 0.002;

/// Fade applied where the sound is cut off, in seconds — at the end of a beat
/// for a long sound, where a hard stop would click on every hit.
const FADE_OUT_SECS: f64 = 0.010;

/// Plays the sound in a take once per grid degree, ascending, one per beat.
///
/// The take is the instrument and the grid is the figure: degree `1:1` is the
/// sound exactly as recorded, and every other degree is that sound shifted by a
/// ratio. Nothing here detects a pitch, so a knock on the table plays as
/// readily as a hum.
///
/// Three things happen to the take before it is an instrument:
///
/// - **it is cut down to the sound.** A take is a capture window, not a sound;
///   played whole, every hit replays the silence before it first, and a knock
///   0.4 s into the window lands at beats 0.61, 1.49, 2.41, 3.35 instead of on
///   the beat. [`sound_span`] finds where the sound is.
/// - **it is capped at one beat.** Every degree in the figure is at or above
///   `1:1`, so no hit is longer than the sound itself; capping the sound caps
///   every hit. Without it a held hum stacks up into a chord rather than
///   climbing a scale (owner decision, R-0040).
/// - **it is faded** at both cuts, so neither end clicks.
///
/// `tense` (`0..=100`, the smooth↔tense slider) picks the harmonic-series
/// odd-limit, which decides **which degrees exist** — so the slider chooses the
/// scale the sound is played over. The render bypasses the distortion stage:
/// this is the user's own sound.
///
/// # Errors
///
/// - [`DspError::EmptySignal`] for an empty take and
///   [`DspError::InvalidSampleRate`] for a zero rate;
/// - [`DspError::NonFiniteSample`] or [`DspError::SampleOutOfRange`] if any
///   sample of the take is not audio — checked across the *whole* take, so a
///   corrupt sample is reported even if it sits in the silence that gets cut;
/// - [`DspError::Silent`] if nothing in the take rises above the silence floor:
///   a muted microphone, a denied permission, or room hiss alone. That is a
///   failed recording, and the user needs to hear so rather than hear their
///   hiss amplified to full scale.
pub fn instrument_from_take(
    samples: &[f32],
    sample_rate: u32,
    tense: u8,
) -> Result<RiffView, DspError> {
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

    let tempo = easy_mode_tempo();
    let sampler =
        Sampler::new(the_sound(samples, sample_rate, &tempo)?)?.rooted_at_octave(TAKE_OCTAVE);
    let grid = easy_mode_grid(tense);
    let notes = figure(&grid, &tempo);

    let mut audio = render_sampled_notes(
        &sampler,
        &notes,
        sample_rate,
        &RenderConfig {
            distortion: Distortion::Bypass,
            ..RenderConfig::default()
        },
    );
    let bars = pad_to_bars(&mut audio, bar_samples(&tempo, sample_rate));
    Ok(RiffView {
        // This path does not listen to the take for tempo or key (R-0040's
        // decision log): the figure runs on Easy Mode's clock, and the grid
        // root has no audible effect on a sampled figure.
        followed_bpm: None,
        followed_root_hz: None,
        bpm: tempo.bpm(),
        beats_per_bar: tempo.beats_per_bar(),
        part: Part::Instrument,
        sample_rate,
        bars,
        seconds: audio.len() as f64 / f64::from(sample_rate),
        notes: notes.iter().map(sampled_card).collect(),
        wave: peak_envelope(&audio, WAVE_BUCKETS),
        samples: audio,
    })
}

/// The sound inside a take: cut to where it is, capped at one beat, faded.
fn the_sound(take: &[f32], sample_rate: u32, tempo: &Tempo) -> Result<Vec<f32>, DspError> {
    let span = sound_span(take, sample_rate).ok_or(DspError::Silent)?;
    let beat = ((tempo.seconds_per_beat() * f64::from(sample_rate)).round() as usize).max(1);
    let end = span.end.min(span.start + beat);
    let mut sound = take[span.start..end].to_vec();
    fade(&mut sound, sample_rate);
    Ok(sound)
}

/// Linear fades at both ends, each at most half the sound so they never cross.
fn fade(sound: &mut [f32], sample_rate: u32) {
    let at_most = sound.len() / 2;
    let fade_in = ((FADE_IN_SECS * f64::from(sample_rate)) as usize).min(at_most);
    let fade_out = ((FADE_OUT_SECS * f64::from(sample_rate)) as usize).min(at_most);
    for (i, sample) in sound.iter_mut().take(fade_in).enumerate() {
        *sample *= i as f32 / fade_in as f32;
    }
    for (i, sample) in sound.iter_mut().rev().take(fade_out).enumerate() {
        *sample *= i as f32 / fade_out as f32;
    }
}

/// The rising figure: every grid degree, ascending, one per beat.
fn figure(grid: &PitchGrid, tempo: &Tempo) -> Vec<QuantizedNote> {
    grid.degrees()
        .iter()
        .enumerate()
        .map(|(beat, &degree)| figure_note(degree, beat, tempo))
        .collect()
}

/// One step of the figure: a grid degree, on its own beat.
///
/// `freq_hz` is not used to play — the sampler tunes to the recording, never to
/// an absolute frequency (R-0039) — and it is not shown either; see
/// [`sampled_card`]. It carries the degree's nominal ratio only so the note is a
/// well-formed [`QuantizedNote`].
fn figure_note(degree: Ratio, beat: usize, tempo: &Tempo) -> QuantizedNote {
    QuantizedNote {
        degree,
        octave: TAKE_OCTAVE,
        freq_hz: degree.num() as f64 / degree.den() as f64,
        cents_offset: 0.0,
        onset_step: beat as u64,
        onset_secs: beat as f64 * tempo.seconds_per_beat(),
        duration_secs: tempo.seconds_per_beat(),
    }
}

/// A card for a sampled degree: the ratio, and no pitch.
///
/// Degree `1:1` is whatever pitch the recording had, which nothing measured —
/// so the card shows the ratio the sound was moved by, and says nothing about a
/// frequency it cannot know.
fn sampled_card(note: &QuantizedNote) -> NoteView {
    NoteView {
        num: note.degree.num(),
        den: note.degree.den(),
        octave: note.octave,
        hz: None,
        cents: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn knock() -> Vec<f32> {
        let mut state = 7u64;
        (0..SR as usize / 10)
            .map(|i| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                ((state >> 40) as f32 / 8_388_608.0 - 1.0) * (-(i as f32) / 240.0).exp() * 0.8
            })
            .collect()
    }

    #[test]
    fn the_cards_are_exactly_the_grids_degrees_in_order() {
        // Not "ascending and at least three": exactly the grid, so a dropped or
        // invented degree fails.
        for tense in [0, 30, 100] {
            let view = instrument_from_take(&knock(), SR, tense).expect("a knock");
            let cards: Vec<(u64, u64)> = view.notes.iter().map(|n| (n.num, n.den)).collect();
            let grid: Vec<(u64, u64)> = easy_mode_grid(tense)
                .degrees()
                .iter()
                .map(|d| (d.num(), d.den()))
                .collect();
            assert_eq!(cards, grid, "tense {tense}");
        }
    }

    #[test]
    fn the_figure_steps_one_beat_per_degree() {
        let tempo = easy_mode_tempo();
        let notes = figure(&easy_mode_grid(30), &tempo);
        for (i, note) in notes.iter().enumerate() {
            assert!(
                (note.onset_secs - i as f64 * tempo.seconds_per_beat()).abs() < 1e-12,
                "degree {i} is not on beat {i}"
            );
        }
    }

    #[test]
    fn a_sampled_card_states_no_pitch_it_did_not_hear() {
        let view = instrument_from_take(&knock(), SR, 30).expect("a knock");
        assert!(
            view.notes
                .iter()
                .all(|n| n.hz.is_none() && n.cents.is_none())
        );
    }

    #[test]
    fn a_long_sound_is_capped_at_one_beat_and_faded() {
        let tempo = easy_mode_tempo();
        let held = vec![0.5f32; 3 * SR as usize];
        let sound = the_sound(&held, SR, &tempo).expect("a held tone");
        let beat = (tempo.seconds_per_beat() * f64::from(SR)).round() as usize;
        assert!(
            sound.len() <= beat,
            "{} samples, a beat is {beat}",
            sound.len()
        );
        assert_eq!(sound[0], 0.0, "no fade in");
        assert!(sound[sound.len() - 1].abs() < 1e-3, "no fade out");
    }
}
