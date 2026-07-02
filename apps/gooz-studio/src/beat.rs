//! Beat builder integration (R-0009): Euclidean patterns → loopable stem.

use gooz_ratio::{BeatError, Pattern, Tempo};
use gooz_synth::{BeatVoice, DrumKind, render_beat};

/// A rendered beat, bar-aligned so it loops cleanly on the beat.
///
/// Invariant: `bars == 0` iff `samples` is empty.
#[derive(Debug, Clone, PartialEq)]
pub struct BeatStem {
    /// The rendered audio.
    pub samples: Vec<f32>,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// The stem's length in whole bars.
    pub bars: u32,
}

/// One drum lane specified as `E(onsets, steps)` plus optional rotation.
#[derive(Debug, Clone, PartialEq)]
pub struct BeatVoiceSpec {
    /// Which kit voice to render.
    pub kind: DrumKind,
    /// Euclidean onset count `k`.
    pub onsets: u32,
    /// Euclidean step count `n`.
    pub steps: u32,
    /// Cyclic rotation applied after `E(k, n)`.
    pub rotate: i64,
    /// Lane level in `[0, 1]`.
    pub level: f32,
}

/// Parameters for [`build_beat`].
#[derive(Debug, Clone, PartialEq)]
pub struct BeatConfig {
    /// The drum lanes to mix.
    pub voices: Vec<BeatVoiceSpec>,
    /// How many bars to render.
    pub bars: u32,
}

impl Default for BeatConfig {
    fn default() -> BeatConfig {
        BeatConfig {
            voices: vec![
                BeatVoiceSpec {
                    kind: DrumKind::Kick,
                    onsets: 4,
                    steps: 16,
                    rotate: 0,
                    level: 1.0,
                },
                BeatVoiceSpec {
                    kind: DrumKind::Snare,
                    onsets: 2,
                    steps: 16,
                    rotate: 4,
                    level: 0.9,
                },
                BeatVoiceSpec {
                    kind: DrumKind::HiHat,
                    onsets: 7,
                    steps: 16,
                    rotate: 0,
                    level: 0.7,
                },
            ],
            bars: 4,
        }
    }
}

/// Builds a bar-aligned beat stem from Euclidean templates at `tempo` and
/// `sample_rate`. Returns an empty stem when `bars == 0` or `sample_rate == 0`.
/// Propagates [`BeatError`] from invalid `E(k, n)` construction.
///
/// ```
/// use gooz_dsp::Tempo;
/// use gooz_studio::{build_beat, BeatConfig};
///
/// let tempo = Tempo::new(92.0, 4.0).unwrap();
/// let stem = build_beat(&tempo, 48_000, &BeatConfig::default()).unwrap();
/// assert_eq!(stem.bars, 4);
/// assert!(!stem.samples.is_empty());
/// ```
pub fn build_beat(
    tempo: &Tempo,
    sample_rate: u32,
    cfg: &BeatConfig,
) -> Result<BeatStem, BeatError> {
    if cfg.bars == 0 || sample_rate == 0 {
        return Ok(BeatStem {
            samples: Vec::new(),
            sample_rate,
            bars: 0,
        });
    }

    let voices: Vec<BeatVoice> = cfg
        .voices
        .iter()
        .map(|spec| {
            let pattern = Pattern::euclidean(spec.onsets, spec.steps)?.rotate(spec.rotate);
            Ok(BeatVoice {
                kind: spec.kind,
                pattern,
                level: spec.level,
            })
        })
        .collect::<Result<Vec<_>, BeatError>>()?;

    let samples = render_beat(&voices, tempo, cfg.bars, sample_rate);
    Ok(BeatStem {
        samples,
        sample_rate,
        bars: cfg.bars,
    })
}
