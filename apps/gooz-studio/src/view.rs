//! Serializable riff views for the Easy Mode studio shell (R-0013 v0).
//!
//! The desktop shell's Tauri commands return these DTOs across the IPC boundary;
//! the web frontend renders the notes, draws the waveform envelope, and plays
//! the samples. Keeping the logic here (not in the Tauri crate) means it is
//! device-free and unit-tested by the workspace gate.

use serde::Serialize;

use gooz_dsp::{DspError, PitchGrid, Tempo};

use crate::{PipelineConfig, RiffOutcome, hum_to_riff};

/// How many points the waveform is downsampled to for drawing.
const WAVE_BUCKETS: usize = 600;
/// Easy Mode's default grid root (Hz) and harmonic-series size.
const GRID_ROOT_HZ: f64 = 220.0;
const GRID_HARMONICS: u64 = 9;
/// Easy Mode's default tempo (BPM) and beats per bar.
const TEMPO_BPM: f64 = 92.0;
const BEATS_PER_BAR: f64 = 4.0;

/// One grid-locked note, as shown on a note card.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NoteView {
    /// Ratio numerator (the `3` in `3:2`).
    pub num: u64,
    /// Ratio denominator (the `2` in `3:2`).
    pub den: u64,
    /// Octave offset from the grid root.
    pub octave: i32,
    /// Snapped frequency in Hz.
    pub hz: f64,
    /// Cents offset from the hummed pitch (how far the snap moved it).
    pub cents: f64,
}

/// A riff prepared for the UI: what it heard, a waveform envelope to draw, and
/// the raw samples to play.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RiffView {
    /// Sample rate of `samples`, in Hz.
    pub sample_rate: u32,
    /// Length of the loop in whole bars.
    pub bars: u32,
    /// Duration of the loop in seconds.
    pub seconds: f64,
    /// The grid-locked notes ("esto escuché").
    pub notes: Vec<NoteView>,
    /// A peak-envelope downsample of the riff, for the waveform canvas.
    pub wave: Vec<f32>,
    /// The raw mono riff samples, for Web Audio playback.
    pub samples: Vec<f32>,
}

impl RiffView {
    /// Builds a UI view from a pipeline outcome, downsampling the stem to a
    /// [`WAVE_BUCKETS`]-point peak envelope.
    pub fn from_outcome(outcome: &RiffOutcome) -> RiffView {
        let stem = &outcome.stem;
        let notes = outcome
            .notes
            .iter()
            .map(|n| NoteView {
                num: n.degree.num(),
                den: n.degree.den(),
                octave: n.octave,
                hz: n.freq_hz,
                cents: n.cents_offset,
            })
            .collect();
        let seconds = if stem.sample_rate == 0 {
            0.0
        } else {
            stem.samples.len() as f64 / f64::from(stem.sample_rate)
        };
        RiffView {
            sample_rate: stem.sample_rate,
            bars: stem.bars,
            seconds,
            notes,
            wave: peak_envelope(&stem.samples, WAVE_BUCKETS),
            samples: stem.samples.clone(),
        }
    }
}

/// Runs Easy Mode's standard pipeline (220 Hz harmonic grid, 92 BPM) on a
/// recorded take and returns a UI view. Propagates analysis errors (empty /
/// zero-rate / non-finite input) as a typed [`DspError`].
pub fn riff_from_take(samples: &[f32], sample_rate: u32) -> Result<RiffView, DspError> {
    let outcome = hum_to_riff(
        samples,
        sample_rate,
        &easy_mode_grid(),
        &easy_mode_tempo(),
        &PipelineConfig::default(),
    )?;
    Ok(RiffView::from_outcome(&outcome))
}

/// The built-in demo: a synthesized four-tone hum through the standard pipeline.
/// Deterministic and device-free — powers the shell's "hear a demo" button and
/// makes the shell previewable without a microphone.
pub fn demo_riff() -> RiffView {
    let sample_rate = 48_000u32;
    let hum = demo_hum(sample_rate);
    riff_from_take(&hum, sample_rate).expect("the demo hum is a valid, finite, non-empty signal")
}

fn easy_mode_grid() -> PitchGrid {
    PitchGrid::harmonic(GRID_ROOT_HZ, GRID_HARMONICS).expect("220 Hz harmonic grid is valid")
}

fn easy_mode_tempo() -> Tempo {
    Tempo::new(TEMPO_BPM, BEATS_PER_BAR).expect("92 BPM / 4 beats-per-bar is valid")
}

/// Four tones (slightly off-grid) separated by gaps — like a person humming.
fn demo_hum(sample_rate: u32) -> Vec<f32> {
    let sr = f64::from(sample_rate);
    let tones = [223.0f64, 333.0, 278.0, 438.0];
    let tone_len = (0.35 * sr) as usize;
    let gap_len = (0.12 * sr) as usize;
    let mut hum = Vec::with_capacity(tones.len() * (tone_len + gap_len));
    for &f in &tones {
        for n in 0..tone_len {
            let t = n as f64 / sr;
            let fade = (std::f64::consts::PI * n as f64 / tone_len as f64).sin();
            hum.push((0.6 * fade * (std::f64::consts::TAU * f * t).sin()) as f32);
        }
        hum.resize(hum.len() + gap_len, 0.0);
    }
    hum
}

/// Peak-envelope downsample: the max absolute sample in each of `buckets`
/// contiguous chunks. Empty in → empty out.
fn peak_envelope(samples: &[f32], buckets: usize) -> Vec<f32> {
    if samples.is_empty() || buckets == 0 {
        return Vec::new();
    }
    let chunk = samples.len().div_ceil(buckets);
    samples
        .chunks(chunk)
        .map(|c| c.iter().fold(0.0f32, |peak, &x| peak.max(x.abs())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_riff_hears_the_four_hummed_tones() {
        let view = demo_riff();
        assert_eq!(view.notes.len(), 4, "four tones were hummed");
        // First tone (223 Hz) snaps to the 1:1 root.
        assert_eq!((view.notes[0].num, view.notes[0].den), (1, 1));
        assert!(view.bars >= 1, "a non-empty riff is at least one bar");
        assert!(view.seconds > 0.0);
    }

    #[test]
    fn view_is_bounded_and_downsampled() {
        let view = demo_riff();
        assert!(!view.samples.is_empty());
        assert!(view.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert!(!view.wave.is_empty() && view.wave.len() <= WAVE_BUCKETS);
        assert!(
            view.wave
                .iter()
                .all(|w| w.is_finite() && (0.0..=1.0).contains(w))
        );
    }

    #[test]
    fn demo_riff_is_deterministic() {
        assert_eq!(demo_riff(), demo_riff());
    }

    #[test]
    fn serializes_to_camel_case_json_for_the_frontend() {
        let json = serde_json::to_string(&demo_riff()).expect("RiffView serializes");
        assert!(json.contains("\"sampleRate\""));
        assert!(json.contains("\"num\"") && json.contains("\"cents\""));
    }

    #[test]
    fn riff_from_take_rejects_empty_input() {
        assert!(matches!(
            riff_from_take(&[], 48_000),
            Err(DspError::EmptySignal)
        ));
    }
}
