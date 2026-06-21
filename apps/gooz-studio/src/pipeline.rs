//! The hum→riff pipeline (R-0008): record → track → quantize → render → stem.

use gooz_dsp::{
    Config, DspError, PitchGrid, QuantizedNote, Tempo, Transcription, analyze, quantize_notes,
};
use gooz_synth::{RenderConfig, render_notes};

/// A rendered riff, bar-aligned so it loops cleanly on the beat.
///
/// Invariant: `bars == 0` iff `samples` is empty; otherwise
/// `samples.len() == bars · bar_samples`.
#[derive(Debug, Clone, PartialEq)]
pub struct RiffStem {
    /// The rendered audio, padded to a whole number of bars.
    pub samples: Vec<f32>,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// The stem's length in whole bars.
    pub bars: u32,
}

/// The full result of [`hum_to_riff`]: the stem plus what the pipeline heard.
#[derive(Debug, Clone, PartialEq)]
pub struct RiffOutcome {
    /// The loopable rendered riff.
    pub stem: RiffStem,
    /// The grid-locked notes (what the hum snapped to).
    pub notes: Vec<QuantizedNote>,
    /// The raw transcription (pitch track + onsets the analyzer heard).
    pub transcription: Transcription,
}

/// Parameters for the three pipeline stages.
#[derive(Debug, Clone, PartialEq)]
pub struct PipelineConfig {
    /// R-0005 analysis parameters.
    pub analyze: Config,
    /// Beat-grid subdivision for R-0006 quantization.
    pub subdivision: u32,
    /// R-0007 instrument / FX settings.
    pub render: RenderConfig,
}

impl Default for PipelineConfig {
    fn default() -> PipelineConfig {
        PipelineConfig {
            analyze: Config::default(),
            subdivision: 2,
            render: RenderConfig::default(),
        }
    }
}

/// Turns a recorded monophonic take into a loopable guitar riff: transcribe
/// (R-0005), snap to the grids (R-0006), render (R-0007), then pad the render to
/// a whole number of bars. Returns the stem plus the transcription and the
/// grid-locked notes.
///
/// Errors from analysis (empty / zero sample rate / non-finite samples /
/// window-too-large) are propagated as a typed [`DspError`]; the pipeline never
/// panics. Deterministic: the same inputs always produce the same outcome.
///
/// ```
/// use gooz_studio::{hum_to_riff, PipelineConfig};
/// use gooz_dsp::{PitchGrid, Tempo};
///
/// let sr = 48_000;
/// // 0.3 s of 220 Hz (a grid pitch).
/// let hum: Vec<f32> = (0..(0.3 * sr as f64) as usize)
///     .map(|i| 0.6 * (std::f64::consts::TAU * 220.0 * i as f64 / sr as f64).sin() as f32)
///     .collect();
/// let grid = PitchGrid::harmonic(220.0, 9).unwrap();
/// let tempo = Tempo::new(120.0, 4.0).unwrap();
///
/// let outcome = hum_to_riff(&hum, sr, &grid, &tempo, &PipelineConfig::default()).unwrap();
/// assert!(!outcome.stem.samples.is_empty());
/// assert!(outcome.stem.bars >= 1); // bar-aligned, at least one bar
/// ```
pub fn hum_to_riff(
    samples: &[f32],
    sample_rate: u32,
    pitch_grid: &PitchGrid,
    tempo: &Tempo,
    cfg: &PipelineConfig,
) -> Result<RiffOutcome, DspError> {
    let transcription = analyze(samples, sample_rate, &cfg.analyze)?;
    let notes = quantize_notes(&transcription.notes, pitch_grid, tempo, cfg.subdivision);
    let raw = render_notes(&notes, sample_rate, &cfg.render);

    let bar_samples = ((tempo.bar_seconds() * f64::from(sample_rate)).round() as usize).max(1);
    let stem = if raw.is_empty() {
        RiffStem {
            samples: Vec::new(),
            sample_rate,
            bars: 0,
        }
    } else {
        let bars = raw.len().div_ceil(bar_samples);
        let mut samples = raw;
        samples.resize(bars * bar_samples, 0.0); // padding only — len >= raw, tails preserved
        RiffStem {
            samples,
            sample_rate,
            bars: bars as u32,
        }
    };
    Ok(RiffOutcome {
        stem,
        notes,
        transcription,
    })
}
