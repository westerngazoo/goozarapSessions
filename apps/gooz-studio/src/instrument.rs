//! [`instrument_from_take`] — play a recorded take across the ratio grid.
//!
//! R-0039 made any recording playable; this is the wire that reaches it. A take
//! becomes a [`Sampler`], the grid's degrees become a rising figure played
//! through it, and the result arrives as an ordinary [`RiffView`] — so the
//! shell's existing playback, waveform, save, and export paths carry it
//! unchanged. Realizes R-0040 / SPEC-0040.

use gooz_dsp::{DspError, QuantizedNote, Ratio};
use gooz_synth::{Distortion, RenderConfig, Sampler, render_sampled_notes};

use crate::view::{
    NoteView, RiffView, WAVE_BUCKETS, easy_mode_grid, easy_mode_tempo, peak_envelope,
};

/// The octave the recording sits at: degree `1:1` plays it exactly as recorded.
///
/// Stated rather than inferred. A [`QuantizedNote`]'s octave counts octaves
/// above the *grid's* root, so leaving it implicit is what let a song's root
/// setting silently transpose a sampled part (R-0039's decision log).
const TAKE_OCTAVE: i32 = 0;

/// Plays a recorded take once per grid degree, ascending, one degree per beat.
///
/// The take is the instrument and the grid is the figure: degree `1:1` is the
/// sound exactly as recorded, and every other degree is that sound shifted by a
/// ratio. Nothing here detects a pitch, so a knock on the table plays as
/// readily as a hum.
///
/// `tense` (`0..=100`, the smooth↔tense slider) picks the harmonic-series
/// odd-limit, which decides **which degrees exist** — so the slider chooses the
/// scale the sound is played over.
///
/// The render bypasses the distortion stage: this is the user's own sound, and
/// saturating it by default would answer a question they did not ask.
///
/// # Errors
///
/// [`DspError::EmptySignal`] for an empty take, [`DspError::InvalidSampleRate`]
/// for a zero rate, and [`DspError::NonFiniteSample`] if the take contains a
/// NaN or infinite sample. An empty take is rejected here rather than in
/// [`Sampler`], which accepts an empty recording on purpose: an instrument with
/// nothing in it is silence, but an empty *take* means the recording failed.
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
    let sampler = Sampler::new(samples.to_vec())?.rooted_at_octave(TAKE_OCTAVE);

    let grid = easy_mode_grid(tense);
    let tempo = easy_mode_tempo();
    let notes: Vec<QuantizedNote> = grid
        .degrees()
        .iter()
        .enumerate()
        .map(|(beat, &degree)| figure_note(degree, beat, grid.root_hz(), &tempo))
        .collect();

    let audio = render_sampled_notes(
        &sampler,
        &notes,
        sample_rate,
        &RenderConfig {
            distortion: Distortion::Bypass,
            ..RenderConfig::default()
        },
    );
    Ok(view_of(audio, &notes, sample_rate, &tempo))
}

/// One step of the rising figure: a grid degree, on its own beat.
fn figure_note(degree: Ratio, beat: usize, root_hz: f64, tempo: &gooz_dsp::Tempo) -> QuantizedNote {
    QuantizedNote {
        degree,
        octave: TAKE_OCTAVE,
        // Shown on the card, not used to play: the sampler tunes to the
        // recording, never to an absolute frequency (R-0039).
        freq_hz: degree.to_hz(root_hz).unwrap_or(root_hz),
        cents_offset: 0.0,
        onset_step: beat as u64,
        onset_secs: beat as f64 * tempo.seconds_per_beat(),
        duration_secs: tempo.seconds_per_beat(),
    }
}

/// Wraps the rendered figure in the view the shell already knows how to play.
fn view_of(
    audio: Vec<f32>,
    notes: &[QuantizedNote],
    sample_rate: u32,
    tempo: &gooz_dsp::Tempo,
) -> RiffView {
    let seconds = audio.len() as f64 / f64::from(sample_rate);
    let bars = (seconds / tempo.bar_seconds()).ceil().max(1.0) as u32;
    RiffView {
        sample_rate,
        bars,
        seconds,
        notes: notes
            .iter()
            .map(|note| NoteView {
                num: note.degree.num(),
                den: note.degree.den(),
                octave: note.octave,
                hz: note.freq_hz,
                cents: note.cents_offset,
            })
            .collect(),
        wave: peak_envelope(&audio, WAVE_BUCKETS),
        samples: audio,
    }
}
