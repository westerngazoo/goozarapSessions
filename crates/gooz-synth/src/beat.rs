//! Beat builder — synthesize Euclidean rhythms into a drum stem.

use gooz_ratio::{BeatError, Pattern, Tempo};

/// The available synthesized drum voices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeatVoice {
    /// A synthesized kick drum (sine sweep).
    Kick,
    /// A synthesized snare drum (filtered noise + body).
    Snare,
    /// A synthesized hi-hat (high-passed noise).
    Hat,
}

/// A single voice's configuration for the beat builder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrumVoiceConfig {
    /// The voice to synthesize.
    pub voice: BeatVoice,
    /// Number of hits (onsets).
    pub k: u32,
    /// Total number of steps in the pattern.
    pub n: u32,
    /// Rotation to apply to the Euclidean pattern.
    pub rotation: i64,
}

/// The result of building a beat.
#[derive(Debug, Clone, PartialEq)]
pub struct BeatOutcome {
    /// The bar-aligned, mixed, and looped drum stem samples.
    pub samples: Vec<f32>,
    /// The sample rate of the stem.
    pub sample_rate: u32,
    /// The length of the stem in bars (1 if non-empty, 0 if empty).
    pub bars: u32,
    /// The resolved Euclidean patterns, parallel to the input voices.
    pub patterns: Vec<Pattern>,
}

/// Fixed random seed for deterministic noise.
const NOISE_SEED: u64 = 0x8A14_F3B2_9C7E_1D05;

/// Simple LCG for deterministic noise generation.
struct Lcg {
    state: u64,
}

impl Lcg {
    fn new(seed: u64) -> Self {
        Lcg { state: seed }
    }
    fn next_f32(&mut self) -> f32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // Map top 32 bits to [-1.0, 1.0]
        let x = (self.state >> 32) as u32;
        (x as f32 / (u32::MAX as f32 / 2.0)) - 1.0
    }
}

/// Builds a drum loop from a set of voice configurations.
///
/// Converts each `DrumVoiceConfig` into a Euclidean `Pattern` and renders it over
/// a single bar timeline. Renders individual hits using simple synthesis methods,
/// mixes them into a single mono buffer, and wraps decay tails across the loop
/// boundary so the end seamlessly wraps into the beginning.
pub fn build_beat(
    voices: &[DrumVoiceConfig],
    tempo: &Tempo,
    sample_rate: u32,
) -> Result<BeatOutcome, BeatError> {
    if sample_rate == 0 {
        return Ok(BeatOutcome {
            samples: Vec::new(),
            sample_rate,
            bars: 0,
            patterns: voices
                .iter()
                .map(|_| Pattern::euclidean(1, 1).unwrap())
                .collect(), // will be overwritten properly below
        });
    }

    let mut patterns = Vec::with_capacity(voices.len());
    let mut all_silent = true;

    for config in voices {
        let pattern = Pattern::euclidean(config.k, config.n)?.rotate(config.rotation);
        if pattern.onset_count() > 0 {
            all_silent = false;
        }
        patterns.push(pattern);
    }

    if all_silent {
        return Ok(BeatOutcome {
            samples: Vec::new(),
            sample_rate,
            bars: 0,
            patterns,
        });
    }

    let bar_seconds = tempo.bar_seconds();
    let bar_samples = (bar_seconds * sample_rate as f64).round() as usize;
    if bar_samples == 0 {
        return Ok(BeatOutcome {
            samples: Vec::new(),
            sample_rate,
            bars: 0,
            patterns,
        });
    }

    let mut stem = vec![0.0f32; bar_samples];

    for (config, pattern) in voices.iter().zip(&patterns) {
        if config.k == 0 || config.n == 0 {
            continue;
        }

        let hit_samples = render_hit(config.voice, sample_rate);
        let n = config.n as f64;

        for (step_idx, &is_onset) in pattern.steps().iter().enumerate() {
            if !is_onset {
                continue;
            }

            let offset_samples = ((step_idx as f64 / n) * bar_samples as f64).round() as usize;

            // Mix hit_samples into stem, wrapping around bar_samples boundary
            for (i, &sample) in hit_samples.iter().enumerate() {
                let out_idx = (offset_samples + i) % bar_samples;
                stem[out_idx] += sample;
            }
        }
    }

    normalize_peak(&mut stem);

    Ok(BeatOutcome {
        samples: stem,
        sample_rate,
        bars: 1,
        patterns,
    })
}

/// Renders a single hit of the given voice type.
fn render_hit(voice: BeatVoice, sample_rate: u32) -> Vec<f32> {
    match voice {
        BeatVoice::Kick => render_kick(sample_rate),
        BeatVoice::Snare => render_snare(sample_rate),
        BeatVoice::Hat => render_hat(sample_rate),
    }
}

/// Renders an 808-style kick drum (pitch envelope + amplitude envelope on a sine wave).
fn render_kick(sample_rate: u32) -> Vec<f32> {
    let duration_secs = 0.5;
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    let mut out = vec![0.0; num_samples];

    let start_freq = 150.0;
    let end_freq = 50.0;
    let mut phase = 0.0;

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / sample_rate as f32;
        let env = (-t * 8.0).exp(); // Fast decay

        // Pitch envelope: drops fast
        let freq = end_freq + (start_freq - end_freq) * (-t * 15.0).exp();
        phase += freq * std::f32::consts::TAU / sample_rate as f32;

        *sample = phase.sin() * env;
    }
    out
}

/// Renders a snare drum (filtered noise + small tonal body).
fn render_snare(sample_rate: u32) -> Vec<f32> {
    let duration_secs = 0.25;
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    let mut out = vec![0.0; num_samples];

    let mut lcg = Lcg::new(NOISE_SEED ^ 0x1111);
    let mut phase = 0.0;

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / sample_rate as f32;
        let env_noise = (-t * 15.0).exp();
        let env_tone = (-t * 20.0).exp();

        let tone = 180.0; // 180Hz body
        phase += tone * std::f32::consts::TAU / sample_rate as f32;

        let noise = lcg.next_f32() * env_noise;
        let body = phase.sin() * env_tone * 0.5;

        *sample = noise + body;
    }
    out
}

/// Renders a hi-hat (high-passed noise burst).
fn render_hat(sample_rate: u32) -> Vec<f32> {
    let duration_secs = 0.1;
    let num_samples = (duration_secs * sample_rate as f32) as usize;
    let mut out = vec![0.0; num_samples];

    let mut lcg = Lcg::new(NOISE_SEED ^ 0x2222);
    let mut last_noise = 0.0; // rudimentary high-pass filter state

    for (i, sample) in out.iter_mut().enumerate() {
        let t = i as f32 / sample_rate as f32;
        let env = (-t * 40.0).exp(); // Very fast decay

        let raw_noise = lcg.next_f32();
        // Simple high-pass: y[i] = x[i] - x[i-1]
        let hp_noise = raw_noise - last_noise;
        last_noise = raw_noise;

        *sample = hp_noise * env;
    }
    out
}

fn normalize_peak(buf: &mut [f32]) {
    let peak = buf.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if peak > 1.0 {
        let gain = 1.0 / peak;
        for x in buf.iter_mut() {
            *x *= gain;
        }
    }
}
