//! Mixdown of a song's arrangement and WAV export of the master and stems.

use std::path::{Path, PathBuf};

use crate::error::SessionError;
use crate::model::Song;

/// A rendered master: the summed arrangement as mono samples.
#[derive(Debug, Clone, PartialEq)]
pub struct Mixdown {
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// The mono master samples, bounded to `[-1, 1]`.
    pub samples: Vec<f32>,
}

/// Bar length in samples for a song's tempo settings.
fn bar_samples(song: &Song, sample_rate: u32) -> usize {
    let bar_seconds = 60.0 / song.settings.bpm * song.settings.beats_per_bar;
    ((bar_seconds * f64::from(sample_rate)).round() as usize).max(1)
}

/// The sample rate shared by every placed, non-muted stem, or an error if they
/// disagree. `None` means there is nothing to mix.
fn master_sample_rate(song: &Song) -> Result<Option<u32>, SessionError> {
    let mut rate: Option<u32> = None;
    for placement in &song.arrangement.placements {
        if placement.muted {
            continue;
        }
        let stem = &song.stems[placement.stem]; // validated: stem index in range
        match rate {
            None => rate = Some(stem.sample_rate),
            Some(r) if r != stem.sample_rate => {
                return Err(SessionError::Export(format!(
                    "stem '{}' sample rate {} != master {r}; resampling is not supported",
                    stem.name, stem.sample_rate
                )));
            }
            _ => {}
        }
    }
    Ok(rate)
}

impl Song {
    /// Renders the arrangement to a mono master: every non-muted placement is
    /// summed at its `start_bar` offset, scaled by its `level`, and looped across
    /// the arrangement's length. A clip-safety limiter scales the result down
    /// only if the summed peak would exceed `1.0`.
    ///
    /// Returns an empty master when there is nothing placed. Errors on an invalid
    /// arrangement or mismatched stem sample rates.
    pub fn mixdown(&self) -> Result<Mixdown, SessionError> {
        self.validate()?;
        let sample_rate = match master_sample_rate(self)? {
            Some(rate) => rate,
            None => {
                return Ok(Mixdown {
                    sample_rate: 0,
                    samples: Vec::new(),
                });
            }
        };
        let bar = bar_samples(self, sample_rate);
        let total = self.arrangement.total_bars(&self.stems) as usize * bar;
        let mut out = vec![0.0f32; total];

        for placement in &self.arrangement.placements {
            if placement.muted {
                continue;
            }
            let stem = &self.stems[placement.stem];
            if stem.samples.is_empty() {
                continue;
            }
            let start = placement.start_bar as usize * bar;
            for (offset, slot) in out.iter_mut().enumerate().skip(start) {
                let src = stem.samples[(offset - start) % stem.samples.len()];
                *slot += src * placement.level;
            }
        }

        limit_peak(&mut out);
        Ok(Mixdown {
            sample_rate,
            samples: out,
        })
    }

    /// Mixes the song and writes the master to a mono 16-bit PCM WAV at `path`.
    pub fn export_master(&self, path: impl AsRef<Path>) -> Result<(), SessionError> {
        let master = self.mixdown()?;
        write_wav(path.as_ref(), master.sample_rate.max(1), &master.samples)
    }

    /// Writes each stem to its own `NN-name.wav` in `dir` (created if missing),
    /// returning the written paths in stem order.
    pub fn export_stems(&self, dir: impl AsRef<Path>) -> Result<Vec<PathBuf>, SessionError> {
        let dir = dir.as_ref();
        std::fs::create_dir_all(dir).map_err(|e| SessionError::Io(e.to_string()))?;
        let mut written = Vec::with_capacity(self.stems.len());
        for (i, stem) in self.stems.iter().enumerate() {
            let path = dir.join(format!("{i:02}-{}.wav", sanitize(&stem.name)));
            write_wav(&path, stem.sample_rate.max(1), &stem.samples)?;
            written.push(path);
        }
        Ok(written)
    }
}

/// Scales `buf` so its peak magnitude is at most `1.0` (no-op if already safe).
fn limit_peak(buf: &mut [f32]) {
    let peak = buf.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    if peak > 1.0 {
        let gain = 1.0 / peak;
        for x in buf.iter_mut() {
            *x *= gain;
        }
    }
}

/// Replaces path-hostile characters in a stem name with `_`.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

/// Writes mono samples as a 16-bit PCM WAV.
fn write_wav(path: &Path, sample_rate: u32, samples: &[f32]) -> Result<(), SessionError> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(path, spec).map_err(|e| SessionError::Export(e.to_string()))?;
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let value = (clamped * f32::from(i16::MAX)).round() as i16;
        writer
            .write_sample(value)
            .map_err(|e| SessionError::Export(e.to_string()))?;
    }
    writer
        .finalize()
        .map_err(|e| SessionError::Export(e.to_string()))
}
