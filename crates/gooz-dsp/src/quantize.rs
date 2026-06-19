//! Snap note events onto the ratio grids (R-0006).
//!
//! Quantizes R-0005's [`NoteEvent`]s onto a frequency grid ([`PitchGrid`]) and a
//! beat grid (a [`Tempo`] plus a subdivision): each note's pitch snaps to the
//! nearest grid degree (correct octave), and its onset and end snap to whole
//! beat-grid steps. The take's `t = 0` is the grid origin (the downbeat).

use gooz_ratio::{PitchGrid, Tempo};

use crate::transcribe::NoteEvent;

/// A note quantized onto the frequency and beat grids.
///
/// The pitch fields come from [`PitchGrid::snap`]; the timing fields are the
/// beat-grid snap. The `_secs` fields are strictly derived from the `_step`
/// fields, so they never drift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantizedNote {
    /// Grid degree the pitch snapped to (octave-reduced ratio).
    pub degree: gooz_ratio::Ratio,
    /// Octaves above the grid root.
    pub octave: i32,
    /// The exact snapped grid frequency, in Hz.
    pub freq_hz: f64,
    /// Original pitch minus snapped, in cents (signed: sharp +, flat −).
    pub cents_offset: f64,
    /// Snapped onset as a global beat-grid step index.
    pub onset_step: u64,
    /// Snapped onset time (`onset_step · step_secs`), in seconds.
    pub onset_secs: f64,
    /// Snapped duration (`(end_step − onset_step) · step_secs`, ≥ 1 step), in seconds.
    pub duration_secs: f64,
}

/// Quantizes note events onto the pitch grid and the beat grid.
///
/// The beat grid's step is `tempo.seconds_per_beat() / subdivision` seconds (a
/// `subdivision` of `0` is treated as `1`). Pitch, onset, and duration are all
/// snapped; a note's duration is always at least one step. A note whose pitch is
/// non-finite or non-positive is skipped (it cannot occur for a voiced R-0005
/// note); empty input yields empty output. Total and panic-free.
///
/// ```
/// use gooz_dsp::{quantize_notes, NoteEvent, PitchGrid, Tempo};
///
/// let grid = PitchGrid::harmonic(220.0, 9).unwrap(); // 220 Hz root
/// let tempo = Tempo::new(120.0, 4.0).unwrap();        // 0.5 s/beat
/// let notes = vec![NoteEvent { onset_secs: 0.0, pitch_hz: 446.0, duration_secs: 0.5 }];
///
/// let q = quantize_notes(&notes, &grid, &tempo, 2); // step = 0.25 s
/// assert_eq!(q.len(), 1);
/// assert_eq!(q[0].freq_hz, 440.0); // 446 Hz snaps to the unison one octave up
/// assert!(q[0].cents_offset > 0.0); // the hum was sharp of 440
/// ```
pub fn quantize_notes(
    notes: &[NoteEvent],
    pitch_grid: &PitchGrid,
    tempo: &Tempo,
    subdivision: u32,
) -> Vec<QuantizedNote> {
    let step_secs = tempo.seconds_per_beat() / f64::from(subdivision.max(1));
    notes
        .iter()
        .filter_map(|note| {
            let snapped = pitch_grid.snap(f64::from(note.pitch_hz)).ok()?;
            let onset_step = (note.onset_secs / step_secs).round().max(0.0) as u64;
            let end_raw = ((note.onset_secs + note.duration_secs) / step_secs)
                .round()
                .max(0.0) as u64;
            let end_step = end_raw.max(onset_step + 1); // duration always >= 1 step
            Some(QuantizedNote {
                degree: snapped.degree,
                octave: snapped.octave,
                freq_hz: snapped.hz,
                cents_offset: snapped.cents_offset,
                onset_step,
                onset_secs: onset_step as f64 * step_secs,
                duration_secs: (end_step - onset_step) as f64 * step_secs,
            })
        })
        .collect()
}
