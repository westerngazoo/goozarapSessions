//! [`follow_take`] — what a take says about itself (R-0041 / SPEC-0041).
//!
//! Two numbers: how fast the take is, and the pitch it sits around. Both are
//! [`Option`]s, because a take does not always say. A knock has a pulse and no
//! pitch; one long note has a pitch and no pulse; a free-time hum has neither.
//! This layer reports what it heard — it does not know what the caller's
//! defaults are, and inventing one here would make it impossible for a UI to
//! say truthfully which of the two it actually followed.
//!
//! **Refusing to answer is the hard part.** An estimator with no gates always
//! produces a number: two notes nine seconds apart yield a confident 103 BPM,
//! and a free-time hum yields 117.8. Both are worse than silence, because the
//! caller's fallback is at least a known quantity. The gates below are what
//! make `None` reachable for the right reasons, and each one is here because a
//! measured signal got through without it.

use crate::error::DspError;
use crate::transcribe::{Config, PitchFrame, Transcription, analyze};

/// The slowest tempo this crate will report, in BPM.
pub const MIN_BPM: f64 = 60.0;
/// The fastest tempo this crate will report, in BPM.
pub const MAX_BPM: f64 = 180.0;

/// How many times a raw estimate may be doubled or halved.
///
/// Two, not more: a detector reporting double or half the felt pulse is
/// agreeing with the listener, but at four octaves of slack *any* interval
/// lands in range and the answer stops meaning anything. Measured, eight folds
/// made `None` unreachable for every take with two or more onsets.
const MAX_FOLDS: u32 = 2;

/// The fewest onsets a pulse may be read from.
///
/// Three, so there are at least two intervals to compare. One interval is a
/// gap, not a tempo — two notes nine seconds apart otherwise reported 103 BPM.
const MIN_ONSETS: usize = 3;

/// How uneven the intervals may be, as median-absolute-deviation over median.
///
/// A steady pulse sits near zero, and so does a long-short feel — `0.4 s` and
/// `0.8 s` are one grid of `0.4 s`, which is a real pulse and is reported as
/// 150 BPM. What this rejects is a take with no grid at all: a free-time hum
/// measured 0.54, and two notes nine seconds apart have no spread to measure
/// because they have only one interval (see [`MIN_ONSETS`]).
const MAX_IOI_SPREAD: f64 = 0.15;

/// The shortest voiced stretch a root may be read from, in seconds.
const MIN_VOICED_SECS: f64 = 0.25;

/// The fewest voiced frames a root may be read from.
const MIN_VOICED_FRAMES: usize = 8;

/// How solidly voiced the phrase must be **within its own span**.
///
/// Deliberately *not* measured against the whole take: a sung phrase with
/// silence on either side is the most ordinary shape a recording has, and
/// gating on the whole take rejected exactly that — a 1.5 s hum with three
/// seconds of lead-in reported no root at all, while the same hum with two
/// seconds reported 220 Hz. What matters is whether the voiced part is a
/// phrase or a scattering, and that is a question about the span it covers.
const MIN_VOICED_DENSITY: f64 = 0.5;

/// What a take says about itself.
///
/// `None` means the take did not say, not that the answer is zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Follow {
    /// The take's tempo in BPM, always within [`MIN_BPM`]..=[`MAX_BPM`].
    pub bpm: Option<f64>,
    /// The pitch the take sits around, in Hz.
    pub root_hz: Option<f64>,
}

/// Reads a take's tempo and pitch centre out of an analysis that already ran.
///
/// Pure and infallible: everything it needs is in the [`Transcription`]. Prefer
/// this when the caller has one — [`analyze`] costs hundreds of milliseconds on
/// a few seconds of audio, and running it twice to answer two questions about
/// one take is the kind of waste that is invisible until it is not.
///
/// The root is the take's **central pitch, not its tonic**. Establishing a key
/// centre properly means weighing which pitches are structurally important,
/// which this project does not do yet; for accompanying whoever is singing, the
/// pitch they are singing around is the honest answer.
pub fn follow(transcription: &Transcription) -> Follow {
    let onsets: Vec<f64> = transcription
        .onsets
        .iter()
        .map(|onset| onset.time_secs)
        .collect();
    Follow {
        bpm: estimate_bpm(&onsets).and_then(fold_into_range),
        root_hz: central_pitch(&transcription.pitch_track.frames),
    }
}

/// Listens to a take for its tempo and its pitch centre.
///
/// [`analyze`] then [`follow`]. Use the two separately when the transcription
/// is wanted for anything else.
///
/// # Errors
///
/// The crate's usual input guard: [`DspError::EmptySignal`],
/// [`DspError::InvalidSampleRate`], [`DspError::NonFiniteSample`], and
/// [`DspError::WindowTooLarge`] when the take is shorter than the window.
///
/// ```
/// use gooz_dsp::{Config, follow_take};
///
/// let sr = 48_000;
/// let tone: Vec<f32> = (0..sr as usize)
///     .map(|i| 0.8 * (std::f64::consts::TAU * 220.0 * i as f64 / f64::from(sr)).sin() as f32)
///     .collect();
///
/// let heard = follow_take(&tone, sr, &Config::default())?;
/// assert!(heard.root_hz.is_some());  // a steady tone has a pitch
/// assert!(heard.bpm.is_none());      // and no pulse to speak of
/// # Ok::<(), gooz_dsp::DspError>(())
/// ```
pub fn follow_take(signal: &[f32], sample_rate: u32, cfg: &Config) -> Result<Follow, DspError> {
    Ok(follow(&analyze(signal, sample_rate, cfg)?))
}

/// Estimates tempo from onset times, or `None` when they are not a pulse.
///
/// `60 / median(inter-onset interval)`, refused when there are fewer than
/// [`MIN_ONSETS`] onsets or the intervals are more uneven than
/// [`MAX_IOI_SPREAD`].
///
/// ```
/// use gooz_dsp::estimate_bpm;
///
/// // A steady half-second pulse is 120 BPM.
/// assert_eq!(estimate_bpm(&[0.0, 0.5, 1.0, 1.5]), Some(120.0));
/// // Two onsets are a gap, not a tempo.
/// assert_eq!(estimate_bpm(&[0.0, 9.0]), None);
/// // A long-short feel is one grid of 0.4 s, which is a real pulse.
/// let felt = estimate_bpm(&[0.0, 0.4, 1.2, 1.6, 2.4, 2.8]).unwrap();
/// assert!((felt - 150.0).abs() < 1e-9);
/// // Free time is not.
/// assert_eq!(estimate_bpm(&[0.0, 1.211, 1.68, 2.699, 3.019, 4.838]), None);
/// ```
pub fn estimate_bpm(onset_times: &[f64]) -> Option<f64> {
    if onset_times.len() < MIN_ONSETS {
        return None;
    }
    let iois: Vec<f64> = onset_times.windows(2).map(|w| w[1] - w[0]).collect();
    let median = median_of(iois.clone())?;
    if median <= 0.0 {
        return None;
    }
    let deviations: Vec<f64> = iois.iter().map(|ioi| (ioi - median).abs()).collect();
    let spread = median_of(deviations)? / median;
    if spread > MAX_IOI_SPREAD {
        return None;
    }
    let bpm = 60.0 / median;
    bpm.is_finite().then_some(bpm)
}

/// Folds a raw estimate into the musical range by octaves, or gives up.
///
/// A median inter-onset interval routinely lands on half or double the felt
/// pulse — a listener hearing 150 and a detector reporting 300 are agreeing.
fn fold_into_range(bpm: f64) -> Option<f64> {
    if !bpm.is_finite() || bpm <= 0.0 {
        return None;
    }
    let mut folded = bpm;
    for _ in 0..=MAX_FOLDS {
        if folded < MIN_BPM {
            folded *= 2.0;
        } else if folded > MAX_BPM {
            folded /= 2.0;
        } else {
            return Some(folded);
        }
    }
    None
}

/// The pitch a take sits around: the median of its voiced frames, or `None`
/// when too little of the take was voiced to mean anything.
fn central_pitch(frames: &[PitchFrame]) -> Option<f64> {
    let voiced: Vec<&PitchFrame> = frames
        .iter()
        .filter(|frame| frame.f0_hz.is_some_and(|hz| hz.is_finite() && hz > 0.0))
        .collect();
    if voiced.len() < MIN_VOICED_FRAMES {
        return None;
    }
    let first = voiced.first()?.time_secs;
    let last = voiced.last()?.time_secs;
    if !(last - first).is_finite() || last - first < MIN_VOICED_SECS {
        return None;
    }
    let inside_span = frames
        .iter()
        .filter(|frame| (first..=last).contains(&frame.time_secs))
        .count();
    if (voiced.len() as f64) < MIN_VOICED_DENSITY * inside_span as f64 {
        return None;
    }
    median_of(
        voiced
            .iter()
            .filter_map(|frame| frame.f0_hz)
            .map(f64::from)
            .collect(),
    )
}

/// The middle value, by order statistic.
///
/// Never the average of the two middle values: that invents a number the take
/// never contained — a long-short feel of `0.4 s` and `0.8 s` averaged to
/// `0.6 s`, a pulse matching neither the long notes nor the short ones.
///
/// Even counts take the **lower** middle, so the answer does not flip with the
/// number of onsets. On a 50/50 long-short pattern the upper middle picks the
/// long value for an even count and the short one for an odd count, which made
/// the same rhythm report 150 BPM or nothing depending on how many notes were
/// played.
fn median_of(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    Some(values[(values.len() - 1) / 2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(time_secs: f64, hz: Option<f32>) -> PitchFrame {
        PitchFrame {
            time_secs,
            f0_hz: hz,
            confidence: 0.9,
        }
    }

    #[test]
    fn a_steady_pulse_is_a_tempo() {
        assert_eq!(estimate_bpm(&[0.0, 0.5, 1.0, 1.5]), Some(120.0));
        assert_eq!(estimate_bpm(&[0.0, 0.25, 0.5, 0.75, 1.0]), Some(240.0));
    }

    #[test]
    fn what_is_not_a_pulse_reports_nothing() {
        // Each of these produced a confident, wrong number before the gates.
        assert_eq!(estimate_bpm(&[]), None);
        assert_eq!(estimate_bpm(&[0.3]), None);
        assert_eq!(estimate_bpm(&[0.0, 9.29]), None, "one gap is not a tempo");
        assert_eq!(
            estimate_bpm(&[0.0, 1.211, 1.68, 2.699, 3.019, 4.838]),
            None,
            "free time is not a tempo"
        );
        assert_eq!(estimate_bpm(&[0.5, 0.5, 0.5]), None, "no interval at all");
    }

    #[test]
    fn folding_never_reports_an_unmusical_tempo() {
        let mut raw = vec![
            0.0,
            -1.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::MIN_POSITIVE,
            f64::MAX,
            1e-300,
            1e300,
        ];
        let mut bpm = 0.01;
        while bpm < 100_000.0 {
            raw.push(bpm);
            bpm *= 1.07;
        }
        for value in raw {
            if let Some(folded) = fold_into_range(value) {
                assert!(
                    (MIN_BPM..=MAX_BPM).contains(&folded),
                    "{value} folded to {folded}, outside the musical range"
                );
            }
        }
    }

    #[test]
    fn folding_moves_by_whole_octaves_or_not_at_all() {
        for (raw, expected) in [(300.0, 150.0), (40.0, 80.0), (120.0, 120.0), (20.0, 80.0)] {
            assert_eq!(fold_into_range(raw), Some(expected), "{raw} BPM");
        }
        // Two folds is the limit: past that the answer means nothing.
        assert_eq!(fold_into_range(1e300), None);
        assert_eq!(fold_into_range(1e-300), None);
        assert_eq!(fold_into_range(2000.0), None);
    }

    #[test]
    fn a_phrase_padded_with_silence_still_has_a_root() {
        // The whole-take fraction gate rejected exactly this — the most
        // ordinary shape a real recording has.
        let mut frames: Vec<PitchFrame> = (0..200).map(|i| frame(i as f64 * 0.01, None)).collect();
        frames.extend((0..120).map(|i| frame(2.0 + i as f64 * 0.01, Some(220.0))));
        frames.extend((0..200).map(|i| frame(3.2 + i as f64 * 0.01, None)));
        assert_eq!(central_pitch(&frames), Some(220.0));
    }

    #[test]
    fn a_scattering_of_voiced_frames_is_not_a_phrase() {
        // Voiced frames spread thinly across their own span: no phrase, no root.
        let frames: Vec<PitchFrame> = (0..300)
            .map(|i| frame(i as f64 * 0.01, (i % 10 == 0).then_some(220.0)))
            .collect();
        assert_eq!(central_pitch(&frames), None);
    }

    #[test]
    fn a_root_needs_enough_voiced_frames_over_enough_time() {
        let few: Vec<PitchFrame> = (0..3)
            .map(|i| frame(i as f64 * 0.01, Some(220.0)))
            .collect();
        assert_eq!(central_pitch(&few), None, "three frames is not a phrase");

        let brief: Vec<PitchFrame> = (0..20)
            .map(|i| frame(i as f64 * 0.001, Some(220.0)))
            .collect();
        assert_eq!(central_pitch(&brief), None, "20 ms is not a phrase");
    }

    #[test]
    fn a_non_finite_frame_cannot_become_the_answer() {
        let frames: Vec<PitchFrame> = (0..40)
            .map(|i| {
                frame(
                    i as f64 * 0.01,
                    Some(if i % 4 == 0 { f32::NAN } else { 220.0 }),
                )
            })
            .collect();
        assert_eq!(central_pitch(&frames), Some(220.0));
    }

    #[test]
    fn the_median_is_an_order_statistic_not_an_average() {
        // The average of 0.4 and 0.8 is a value the take never contained.
        assert_eq!(median_of(vec![0.4, 0.8]), Some(0.4));
        assert_eq!(median_of(vec![3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median_of(Vec::new()), None);
    }

    #[test]
    fn a_long_short_feel_reports_its_grid_however_many_notes_were_played() {
        // 0.4 s and 0.8 s are one grid of 0.4 s. Taking the *upper* middle made
        // the same rhythm answer 150 BPM or nothing depending on the note count.
        let short = estimate_bpm(&[0.0, 0.4, 1.2, 1.6, 2.4, 2.8]).expect("a grid");
        let long =
            estimate_bpm(&[0.0, 0.4, 1.2, 1.6, 2.4, 2.8, 3.6, 4.0, 4.8]).expect("the same grid");
        assert!((short - 150.0).abs() < 1e-9, "{short}");
        assert!(
            (short - long).abs() < 1e-9,
            "the answer moved with the number of onsets: {short} vs {long}"
        );
    }
}
