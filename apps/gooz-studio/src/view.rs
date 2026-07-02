//! Serializable riff views for the Easy Mode studio shell (R-0013 v0).
//!
//! The desktop shell's Tauri commands return these DTOs across the IPC boundary;
//! the web frontend renders the notes, draws the waveform envelope, and plays
//! the samples. Keeping the logic here (not in the Tauri crate) means it is
//! device-free and unit-tested by the workspace gate.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use gooz_dsp::{DspError, PitchGrid, Tempo};
use gooz_session::{Section, SessionError, Settings, Song, Stem, StemKind, StemPlacement};

use crate::{
    BeatConfig, BeatStem, BeatVoiceSpec, DrumKind, PipelineConfig, RiffOutcome, build_beat,
    hum_to_riff,
};

/// How many points the waveform is downsampled to for drawing.
const WAVE_BUCKETS: usize = 600;
/// Easy Mode's grid root (Hz).
const GRID_ROOT_HZ: f64 = 220.0;
/// The smooth↔tense slider walks the harmonic-series odd-limit between these
/// bounds: smooth = just the fifth (simple ratios), tense = up to the 15th
/// harmonic (denser, more complex ratios).
const TENSE_MIN_ODD: u64 = 3;
const TENSE_MAX_ODD: u64 = 15;
/// The slider position used by [`demo_riff`].
const DEFAULT_TENSE: u8 = 30;
/// Easy Mode's default tempo (BPM) and beats per bar.
const TEMPO_BPM: f64 = 92.0;
const BEATS_PER_BAR: f64 = 4.0;

/// One grid-locked note, as shown on a note card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Runs Easy Mode's pipeline (220 Hz harmonic grid, 92 BPM) on a recorded take
/// and returns a UI view. The `tense` control (`0..=100`, the smooth↔tense
/// slider) sets the harmonic-series odd-limit: smoother grids favour simple
/// ratios, tenser grids admit more complex ones. Propagates analysis errors
/// (empty / zero-rate / non-finite input) as a typed [`DspError`].
pub fn riff_from_take(samples: &[f32], sample_rate: u32, tense: u8) -> Result<RiffView, DspError> {
    let outcome = hum_to_riff(
        samples,
        sample_rate,
        &easy_mode_grid(tense),
        &easy_mode_tempo(),
        &PipelineConfig::default(),
    )?;
    Ok(RiffView::from_outcome(&outcome))
}

/// The built-in demo: a synthesized four-tone hum through the standard pipeline
/// at the default smooth↔tense setting. Deterministic and device-free — powers
/// the shell's "hear a demo" button and makes it previewable without a mic.
pub fn demo_riff() -> RiffView {
    let sample_rate = 48_000u32;
    let hum = demo_hum(sample_rate);
    riff_from_take(&hum, sample_rate, DEFAULT_TENSE)
        .expect("the demo hum is a valid, finite, non-empty signal")
}

/// Maps the smooth↔tense slider onto the harmonic-series odd-limit: `0` → the
/// simplest grid ([`TENSE_MIN_ODD`]), `100` → the densest ([`TENSE_MAX_ODD`]),
/// stepping through the odd harmonics in between.
fn odd_limit_for(tense: u8) -> u64 {
    let b = f64::from(tense.min(100)) / 100.0;
    let rungs = (TENSE_MAX_ODD - TENSE_MIN_ODD) / 2;
    TENSE_MIN_ODD + 2 * (b * rungs as f64).round() as u64
}

/// How many bars the beat builder loops in the shell.
const BEAT_BARS: u32 = 2;
/// The shared step resolution (`n` in `E(k, n)`) for every drum voice.
const BEAT_STEPS: u32 = 16;

/// One drum lane, as shown under the beat builder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceView {
    /// Human-readable voice name (`kick`, `snare`, `hat`).
    pub name: String,
    /// Euclidean onset count `k` (what the sparse↔busy slider moves).
    pub onsets: u32,
    /// Euclidean step count `n`.
    pub steps: u32,
}

/// A beat prepared for the UI: the lanes, a waveform envelope, and raw samples.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeatView {
    /// Sample rate of `samples`, in Hz.
    pub sample_rate: u32,
    /// Length of the loop in whole bars.
    pub bars: u32,
    /// Duration of the loop in seconds.
    pub seconds: f64,
    /// The drum lanes and their `E(k, n)` densities.
    pub voices: Vec<VoiceView>,
    /// A peak-envelope downsample of the beat, for the waveform canvas.
    pub wave: Vec<f32>,
    /// The raw mono beat samples, for Web Audio playback.
    pub samples: Vec<f32>,
}

/// Builds an Easy-Mode beat whose density follows a single `busy` control
/// (`0..=100`, the sparse↔busy slider): each drum voice's `E(k, 16)` onset
/// count scales between a sparse floor and a busy ceiling. Deterministic and
/// device-free.
pub fn beat_view(busy: u8) -> BeatView {
    let voices = beat_specs(busy);
    let cfg = BeatConfig {
        voices: voices.clone(),
        bars: BEAT_BARS,
    };
    // build_beat only errors on invalid E(k, n); beat_specs keeps k <= 16 = n.
    let stem = build_beat(&easy_mode_tempo(), 48_000, &cfg)
        .expect("beat_specs always produces valid E(k, 16) patterns");
    BeatView::from_stem(&stem, &voices)
}

/// The shell's default beat (sparse↔busy at the slider's mid setting).
pub fn demo_beat() -> BeatView {
    beat_view(55)
}

impl BeatView {
    fn from_stem(stem: &BeatStem, voices: &[BeatVoiceSpec]) -> BeatView {
        let seconds = if stem.sample_rate == 0 {
            0.0
        } else {
            stem.samples.len() as f64 / f64::from(stem.sample_rate)
        };
        BeatView {
            sample_rate: stem.sample_rate,
            bars: stem.bars,
            seconds,
            voices: voices.iter().map(VoiceView::from_spec).collect(),
            wave: peak_envelope(&stem.samples, WAVE_BUCKETS),
            samples: stem.samples.clone(),
        }
    }
}

impl VoiceView {
    fn from_spec(spec: &BeatVoiceSpec) -> VoiceView {
        VoiceView {
            name: drum_name(spec.kind).to_string(),
            onsets: spec.onsets,
            steps: spec.steps,
        }
    }
}

fn drum_name(kind: DrumKind) -> &'static str {
    match kind {
        DrumKind::Kick => "kick",
        DrumKind::Snare => "snare",
        DrumKind::HiHat => "hat",
    }
}

/// Maps the `busy` slider onto each voice's onset count over 16 steps: the kick
/// and hat open up with density while the snare stays near the backbeat.
fn beat_specs(busy: u8) -> Vec<BeatVoiceSpec> {
    let scale = |min: u32, max: u32| -> u32 {
        let b = f64::from(busy.min(100)) / 100.0;
        (f64::from(min) + (f64::from(max - min) * b)).round() as u32
    };
    vec![
        BeatVoiceSpec {
            kind: DrumKind::Kick,
            onsets: scale(2, 8),
            steps: BEAT_STEPS,
            rotate: 0,
            level: 1.0,
        },
        BeatVoiceSpec {
            kind: DrumKind::Snare,
            onsets: scale(2, 4),
            steps: BEAT_STEPS,
            rotate: 4,
            level: 0.9,
        },
        BeatVoiceSpec {
            kind: DrumKind::HiHat,
            onsets: scale(4, 16),
            steps: BEAT_STEPS,
            rotate: 0,
            level: 0.7,
        },
    ]
}

fn easy_mode_grid(tense: u8) -> PitchGrid {
    PitchGrid::harmonic(GRID_ROOT_HZ, odd_limit_for(tense))
        .expect("a harmonic grid with odd_limit >= 3 is valid")
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

/// Assembles a saveable [`Song`] from what the shell is holding: the current
/// slider settings plus an optional riff and/or beat. Each present part becomes
/// a [`Stem`] placed at bar 0; a single section spans the longer of the two.
pub fn build_song(
    name: &str,
    tense: u8,
    busy: u8,
    riff: Option<&RiffView>,
    beat: Option<&BeatView>,
) -> Song {
    let settings = Settings {
        bpm: TEMPO_BPM,
        beats_per_bar: BEATS_PER_BAR,
        root_hz: GRID_ROOT_HZ,
        odd_limit: odd_limit_for(tense),
    };
    let _ = busy; // density is captured in the beat stem's samples already
    let mut song = Song::new(name, settings);
    let mut span_bars = 0u32;

    if let Some(r) = riff.filter(|r| !r.samples.is_empty()) {
        let idx = song.stems.len();
        song = song
            .with_stem(Stem {
                name: "guitar".into(),
                kind: StemKind::Riff,
                sample_rate: r.sample_rate,
                bars: r.bars,
                samples: r.samples.clone(),
            })
            .with_placement(StemPlacement {
                stem: idx,
                start_bar: 0,
                muted: false,
                level: 1.0,
            });
        span_bars = span_bars.max(r.bars);
    }

    if let Some(b) = beat.filter(|b| !b.samples.is_empty()) {
        let idx = song.stems.len();
        song = song
            .with_stem(Stem {
                name: "drums".into(),
                kind: StemKind::Beat,
                sample_rate: b.sample_rate,
                bars: b.bars,
                samples: b.samples.clone(),
            })
            .with_placement(StemPlacement {
                stem: idx,
                start_bar: 0,
                muted: false,
                level: 0.9,
            });
        span_bars = span_bars.max(b.bars);
    }

    if span_bars > 0 {
        song = song.with_section(Section {
            name: "loop".into(),
            start_bar: 0,
            length_bars: span_bars,
        });
    }
    song
}

/// Ensures `dir` exists and returns `dir/<sanitized name>.<ext>`.
fn session_path(dir: &Path, name: &str, ext: &str) -> Result<PathBuf, SessionError> {
    std::fs::create_dir_all(dir).map_err(|e| SessionError::Io(e.to_string()))?;
    let stem: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    let stem = if stem.is_empty() {
        "session".into()
    } else {
        stem
    };
    Ok(dir.join(format!("{stem}.{ext}")))
}

/// Saves the current riff/beat as a `.json` session under `dir`, returning the
/// written path.
pub fn save_session(
    dir: &Path,
    name: &str,
    tense: u8,
    busy: u8,
    riff: Option<&RiffView>,
    beat: Option<&BeatView>,
) -> Result<PathBuf, SessionError> {
    let song = build_song(name, tense, busy, riff, beat);
    let path = session_path(dir, name, "json")?;
    song.save(&path)?;
    Ok(path)
}

/// Mixes the current riff/beat and writes a master `.wav` under `dir`, returning
/// the written path.
pub fn export_master(
    dir: &Path,
    name: &str,
    tense: u8,
    busy: u8,
    riff: Option<&RiffView>,
    beat: Option<&BeatView>,
) -> Result<PathBuf, SessionError> {
    let song = build_song(name, tense, busy, riff, beat);
    let path = session_path(dir, name, "wav")?;
    song.export_master(&path)?;
    Ok(path)
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
            riff_from_take(&[], 48_000, DEFAULT_TENSE),
            Err(DspError::EmptySignal)
        ));
    }

    #[test]
    fn tense_slider_walks_the_odd_limit() {
        assert_eq!(odd_limit_for(0), TENSE_MIN_ODD);
        assert_eq!(odd_limit_for(100), TENSE_MAX_ODD);
        // Monotonic non-decreasing across the range.
        let mut prev = 0;
        for t in (0..=100).step_by(5) {
            let ol = odd_limit_for(t);
            assert!(ol >= prev, "odd_limit dropped at tense={t}");
            assert!(ol % 2 == 1, "odd_limit stays odd at tense={t}");
            prev = ol;
        }
    }

    #[test]
    fn tenser_grid_has_at_least_as_many_degrees() {
        let sr = 48_000u32;
        let hum = demo_hum(sr);
        let smooth = riff_from_take(&hum, sr, 0).unwrap();
        let tense = riff_from_take(&hum, sr, 100).unwrap();
        // A tenser grid never snaps the same hum to fewer distinct ratios.
        let distinct = |v: &RiffView| {
            let mut r: Vec<(u64, u64)> = v.notes.iter().map(|n| (n.num, n.den)).collect();
            r.sort();
            r.dedup();
            r.len()
        };
        assert!(distinct(&tense) >= distinct(&smooth));
    }

    #[test]
    fn demo_beat_has_three_voices_and_audio() {
        let beat = demo_beat();
        assert_eq!(beat.voices.len(), 3);
        assert_eq!(
            beat.voices
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>(),
            ["kick", "snare", "hat"]
        );
        assert!(beat.bars >= 1 && beat.seconds > 0.0);
        assert!(!beat.samples.is_empty());
    }

    #[test]
    fn beat_is_bounded_and_downsampled() {
        let beat = demo_beat();
        assert!(beat.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert!(!beat.wave.is_empty() && beat.wave.len() <= WAVE_BUCKETS);
        assert!(
            beat.wave
                .iter()
                .all(|w| w.is_finite() && (0.0..=1.0).contains(w))
        );
    }

    #[test]
    fn busy_slider_increases_total_onsets() {
        let sparse: u32 = beat_view(0).voices.iter().map(|v| v.onsets).sum();
        let busy: u32 = beat_view(100).voices.iter().map(|v| v.onsets).sum();
        assert!(busy > sparse, "busy={busy} should exceed sparse={sparse}");
    }

    #[test]
    fn beat_view_is_deterministic() {
        assert_eq!(beat_view(70), beat_view(70));
    }

    #[test]
    fn beat_serializes_to_camel_case_json() {
        let json = serde_json::to_string(&demo_beat()).expect("BeatView serializes");
        assert!(json.contains("\"sampleRate\""));
        assert!(json.contains("\"voices\"") && json.contains("\"onsets\""));
    }

    #[test]
    fn build_song_captures_riff_and_beat_as_stems() {
        let riff = demo_riff();
        let beat = demo_beat();
        let song = build_song("my song", 30, 55, Some(&riff), Some(&beat));
        assert_eq!(song.stems.len(), 2);
        assert_eq!(song.stems[0].kind, gooz_session::StemKind::Riff);
        assert_eq!(song.stems[1].kind, gooz_session::StemKind::Beat);
        assert_eq!(song.arrangement.placements.len(), 2);
        assert!(song.validate().is_ok());
        // The tense slider is reflected in the saved grid setting.
        assert_eq!(song.settings.odd_limit, odd_limit_for(30));
    }

    #[test]
    fn build_song_skips_empty_parts() {
        let beat = demo_beat();
        let song = build_song("beat only", 0, 55, None, Some(&beat));
        assert_eq!(song.stems.len(), 1);
        assert_eq!(song.stems[0].kind, gooz_session::StemKind::Beat);
    }

    #[test]
    fn save_and_export_write_files() {
        let riff = demo_riff();
        let beat = demo_beat();
        let mut dir = std::env::temp_dir();
        dir.push(format!("gooz_studio_io_{}", std::process::id()));

        let json = save_session(&dir, "take 1", 30, 55, Some(&riff), Some(&beat)).unwrap();
        assert!(json.exists() && json.extension().unwrap() == "json");
        let loaded = gooz_session::Song::load(&json).unwrap();
        assert_eq!(loaded.stems.len(), 2);

        let wav = export_master(&dir, "take 1", 30, 55, Some(&riff), Some(&beat)).unwrap();
        assert!(wav.exists() && wav.extension().unwrap() == "wav");
        let bytes = std::fs::metadata(&wav).unwrap().len();
        assert!(bytes > 44, "a real WAV is larger than its 44-byte header");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
