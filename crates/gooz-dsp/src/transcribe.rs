//! Configuration, result types, input validation, and note-event assembly.

use crate::error::DspError;
use crate::onset::detect_onsets;
use crate::yin::pitch_track;

/// Analysis parameters. [`Config::default`] is tuned for voice/melody at typical
/// sample rates.
///
/// ```
/// use gooz_dsp::Config;
/// let cfg = Config::default();
/// assert_eq!(cfg.window, 2048);
/// assert_eq!(cfg.f_max, 1000.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Config {
    /// YIN analysis frame length, in samples.
    pub window: usize,
    /// Frames advance by this many samples.
    pub hop: usize,
    /// Lowest pitch reported as voiced, in Hz.
    pub f_min: f32,
    /// Highest pitch reported as voiced, in Hz.
    pub f_max: f32,
    /// YIN absolute threshold on the cumulative-mean-normalized difference.
    pub yin_threshold: f32,
    /// Onset STFT size, in samples.
    pub fft_size: usize,
    /// Onset threshold margin, in multiples of the global standard deviation of
    /// the spectral flux.
    pub onset_sensitivity: f32,
    /// Half-width (in frames) of the onset peak-pick window.
    pub onset_window_frames: usize,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            window: 2048,
            hop: 256,
            f_min: 80.0,
            f_max: 1000.0,
            yin_threshold: 0.15,
            fft_size: 1024,
            onset_sensitivity: 0.3,
            onset_window_frames: 8,
        }
    }
}

/// One frame of the pitch track. `f0_hz == None` means the frame is unvoiced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PitchFrame {
    /// The frame's centre time, in seconds.
    pub time_secs: f64,
    /// The detected fundamental in Hz, or `None` if unvoiced.
    pub f0_hz: Option<f32>,
    /// Periodicity confidence in `[0, 1]`.
    pub confidence: f32,
}

/// A pitch track: per-frame f0 over a signal.
#[derive(Debug, Clone, PartialEq)]
pub struct PitchTrack {
    /// The frames, in time order.
    pub frames: Vec<PitchFrame>,
}

/// A detected note start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Onset {
    /// Onset time, in seconds.
    pub time_secs: f64,
    /// Spectral-flux strength at the onset.
    pub strength: f32,
}

/// A transcribed note: when it starts, its pitch, and how long it lasts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteEvent {
    /// Start time, in seconds.
    pub onset_secs: f64,
    /// Pitch in Hz (median of the segment's voiced frames).
    pub pitch_hz: f32,
    /// Duration, in seconds (strictly positive).
    pub duration_secs: f64,
}

/// The full result of [`analyze`]: assembled note events plus the intermediate
/// pitch track and onsets.
#[derive(Debug, Clone, PartialEq)]
pub struct Transcription {
    /// The per-frame pitch track.
    pub pitch_track: PitchTrack,
    /// The detected onsets.
    pub onsets: Vec<Onset>,
    /// The assembled note events (the headline output).
    pub notes: Vec<NoteEvent>,
}

/// Shared up-front validation: rejects empty input, a zero sample rate, any
/// non-finite sample (so no downstream sum/sort is poisoned), and a window
/// longer than the signal.
pub(crate) fn validate(signal: &[f32], sample_rate: u32, cfg: &Config) -> Result<(), DspError> {
    if signal.is_empty() {
        return Err(DspError::EmptySignal);
    }
    if sample_rate == 0 {
        return Err(DspError::InvalidSampleRate);
    }
    if signal.iter().any(|s| !s.is_finite()) {
        return Err(DspError::NonFiniteSample);
    }
    if cfg.window > signal.len() {
        return Err(DspError::WindowTooLarge);
    }
    Ok(())
}

/// Transcribes a recorded monophonic signal into note events.
///
/// Runs YIN pitch tracking and spectral-flux onset detection, then segments the
/// pitch track at the onsets to assemble note events. Returns those plus the
/// intermediate pitch track and onsets. Errors (empty / zero-rate / non-finite /
/// window-too-large) are typed; library paths never panic.
///
/// ```
/// use gooz_dsp::{analyze, Config};
///
/// // 0.2 s of 440 Hz at 48 kHz.
/// let sr = 48_000;
/// let n = (0.2 * sr as f64) as usize;
/// let signal: Vec<f32> = (0..n)
///     .map(|i| 0.8 * (std::f64::consts::TAU * 440.0 * i as f64 / sr as f64).sin() as f32)
///     .collect();
/// let t = analyze(&signal, sr, &Config::default()).unwrap();
/// assert_eq!(t.notes.len(), 1);
/// ```
pub fn analyze(signal: &[f32], sample_rate: u32, cfg: &Config) -> Result<Transcription, DspError> {
    let pitch = pitch_track(signal, sample_rate, cfg)?;
    let onsets = detect_onsets(signal, sample_rate, cfg)?;
    let notes = assemble_notes(&pitch, &onsets, signal.len(), sample_rate, cfg);
    Ok(Transcription {
        pitch_track: pitch,
        onsets,
        notes,
    })
}

fn median(mut xs: Vec<f32>) -> f32 {
    xs.sort_by(|a, b| a.total_cmp(b));
    xs[xs.len() / 2]
}

/// Segments the pitch track at onset boundaries and emits one note per voiced
/// segment.
fn assemble_notes(
    pitch: &PitchTrack,
    onsets: &[Onset],
    signal_len: usize,
    sample_rate: u32,
    cfg: &Config,
) -> Vec<NoteEvent> {
    let hop_secs = cfg.hop as f64 / sample_rate as f64;

    let mut boundaries: Vec<f64> = onsets.iter().map(|o| o.time_secs).collect();
    boundaries.sort_by(|a, b| a.total_cmp(b));

    // If voiced audio precedes the first onset by more than a hop (the attack
    // escaped the onset detector, or there are no onsets at all), add a leading
    // boundary so the opening note is not dropped.
    if let Some(first_voiced) = pitch.frames.iter().find(|f| f.f0_hz.is_some()) {
        let lead = match boundaries.first() {
            Some(&first_onset) => first_voiced.time_secs + hop_secs < first_onset,
            None => true,
        };
        if lead {
            boundaries.insert(0, first_voiced.time_secs);
        }
    }

    let signal_end = signal_len as f64 / sample_rate as f64;
    let mut notes = Vec::new();
    for k in 0..boundaries.len() {
        let start = boundaries[k];
        let seg_end = boundaries.get(k + 1).copied().unwrap_or(signal_end);

        let voiced: Vec<f32> = pitch
            .frames
            .iter()
            .filter(|f| f.time_secs >= start && f.time_secs < seg_end)
            .filter_map(|f| f.f0_hz)
            .collect();
        if voiced.is_empty() {
            continue;
        }

        let duration_secs = if k + 1 < boundaries.len() {
            seg_end - start
        } else {
            let last_voiced = pitch
                .frames
                .iter()
                .filter(|f| f.time_secs >= start && f.time_secs < seg_end && f.f0_hz.is_some())
                .map(|f| f.time_secs)
                .next_back()
                .unwrap_or(start);
            (last_voiced - start).max(hop_secs)
        };

        notes.push(NoteEvent {
            onset_secs: start,
            pitch_hz: median(voiced),
            duration_secs,
        });
    }
    notes
}
