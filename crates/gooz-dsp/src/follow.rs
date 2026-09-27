//! [`follow_take`] — what a take says about itself (R-0041 / SPEC-0041).
//!
//! Two numbers: how fast the take is, and the pitch it sits around. Both are
//! [`Option`]s, because a take does not always say. A knock has a pulse and no
//! pitch; one long note has a pitch and no pulse; a free-time hum has neither.
//! This layer reports what it heard — it does not know what the caller's
//! defaults are, and inventing one here would make it impossible for a UI to
//! say truthfully which of the two it actually followed.
//!
//! # Why the tempo is an autocorrelation, not a median of intervals
//!
//! The first design measured intervals between spectral-flux onsets and took
//! their median, then added gates for every way that went wrong. Three rounds
//! of review later it still reported a held hum at 122 BPM — spectral flux
//! fires on the steady wobble of a sustained note, and a wobble is perfectly
//! regular, so no interval gate can refuse it — and it could not follow swing,
//! whose two interval lengths have no single "median" pulse.
//!
//! [`tempo_of`] is the method beat trackers use instead. It builds an envelope
//! of **energy attacks** — rises in loudness, which a held note does not have
//! however much it wobbles in pitch — and asks at which period that envelope
//! best repeats itself. A swung bar repeats at the beat, rests reinforce the
//! period rather than break it, and a take with no repeating attacks simply has
//! no strong period. Refusing to answer falls out of the method instead of
//! being bolted onto it.

use crate::error::DspError;
use crate::transcribe::{Config, PitchFrame, Transcription, analyze};

/// The slowest tempo this crate will report, in BPM.
pub const MIN_BPM: f64 = 60.0;
/// The fastest tempo this crate will report, in BPM.
pub const MAX_BPM: f64 = 180.0;

/// The attack envelope's frame step, in seconds (100 frames a second).
///
/// Fixed in *time*, not samples, so a take reads the same at 16, 44.1 or
/// 48 kHz. The previous design's resolution depended on the analysis hop, which
/// moved the answer by several BPM between sample rates.
const FRAME_STEP_SECS: f64 = 0.010;

/// The window each frame's loudness is measured over, in seconds.
///
/// Several periods of any sung pitch, so a steady tone's loudness does not
/// ripple with where the window happens to fall on its waveform.
const FRAME_WINDOW_SECS: f64 = 0.040;

/// Frames quieter than this are silence, in dBFS. Loudness is floored here, so
/// an attack out of silence counts as rising from the floor rather than from
/// the `-∞` of digital zero.
const SILENCE_DB: f64 = -60.0;

/// The smallest frame-to-frame rise that counts toward an attack, in dB.
///
/// A steady note's loudness moves by hundredths of a dB between frames; a
/// struck, plucked or sung onset rises by several dB per frame. The floor sits
/// between them, so a held note contributes no attacks at all.
const RISE_FLOOR_DB: f64 = 1.0;

/// How far each attack is spread in time before looking for a period, in
/// seconds either side.
///
/// Nobody plays to a click. An attack is a spike one or two frames wide, and a
/// hand-played pulse wobbles by ±20 ms — two frames — so unspread spikes on
/// successive beats miss each other and a perfectly human 92 BPM take scored
/// below [`MIN_PULSE_STRENGTH`]. Spreading each attack over ±30 ms lets
/// neighbouring beats overlap while still keeping a 180 BPM beat (333 ms)
/// clearly separate from its neighbours.
const ATTACK_SPREAD_SECS: f64 = 0.030;

/// The slowest pulse the envelope is searched for, in BPM, before folding.
///
/// Half of [`MIN_BPM`]: a 40 BPM pulse repeats at no period between 60 and 180
/// BPM — its attacks are 1.5 s apart and nothing lands in between — so it has
/// to be found at its own period and then doubled into range.
const SLOWEST_SEARCHED_BPM: f64 = MIN_BPM / 2.0;

/// How strongly the attack envelope must repeat at its best period, as a share
/// of its energy at zero lag, for that period to count as a pulse.
///
/// Three evenly spaced hits score about 2/3 and are a pulse. Two hits half a
/// second apart and a third nine seconds later score about 1/3 — an earlier
/// median-of-intervals design reported that as 120 BPM, because with only two
/// intervals its spread gate could never fire.
const MIN_PULSE_STRENGTH: f64 = 0.4;

/// The tempo a listener leans toward when a pulse could be heard at two
/// octaves, in BPM, and how many octaves either side that lean fades over.
///
/// Inside [`MIN_BPM`]..=[`MAX_BPM`] some tempos have an octave that also fits
/// (90 and 180, say). The envelope repeats at both; this breaks the tie toward
/// the one a person is more likely to be feeling, without overruling a pulse
/// that is clearly stronger at the other.
const PRIOR_CENTRE_BPM: f64 = 120.0;
const PRIOR_WIDTH_OCTAVES: f64 = 1.0;

/// The shortest stretch of continuous voicing that counts as a sung note, in
/// seconds. Shorter runs are breath, consonants, or a stray frame.
const MIN_RUN_SECS: f64 = 0.08;

/// How much sung voicing a root may be read from, in seconds, across all runs.
const MIN_VOICED_SECS: f64 = 0.25;

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

/// Reads a take's tempo and pitch centre, given an analysis that already ran.
///
/// The pitch comes from the [`Transcription`]'s pitch track; the tempo from
/// the signal's own loudness ([`tempo_of`]), which the transcription does not
/// carry. Prefer this over [`follow_take`] when the caller has the analysis
/// already — [`analyze`] costs hundreds of milliseconds on a few seconds of
/// audio.
///
/// The root is the take's **central pitch, not its tonic**. Establishing a key
/// centre means weighing which pitches are structurally important, which this
/// project does not do yet; for accompanying whoever is singing, the pitch they
/// are singing around is the honest answer.
pub fn follow(signal: &[f32], sample_rate: u32, transcription: &Transcription) -> Follow {
    Follow {
        bpm: tempo_of(signal, sample_rate),
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
    let transcription = analyze(signal, sample_rate, cfg)?;
    Ok(follow(signal, sample_rate, &transcription))
}

/// The pulse of a signal, in BPM, or `None` when it has none.
///
/// Autocorrelation of the energy-attack envelope over the periods that are
/// [`MIN_BPM`]..=[`MAX_BPM`] apart; the strongest repeat wins, leaning toward
/// [`PRIOR_CENTRE_BPM`] when two octaves repeat about equally, and the answer
/// is refused when even the strongest repeat is weaker than
/// [`MIN_PULSE_STRENGTH`]. Non-finite samples are ignored.
///
/// ```
/// use gooz_dsp::tempo_of;
///
/// let sr = 16_000;
/// // A click every half second: 120 BPM.
/// let mut clicks = vec![0.0f32; 4 * sr as usize];
/// for beat in 0..8 {
///     for i in 0..80 { clicks[beat * sr as usize / 2 + i] = 0.8; }
/// }
/// let bpm = tempo_of(&clicks, sr).expect("a steady pulse");
/// assert!((bpm - 120.0).abs() < 2.0);
///
/// // One held note: loud, but nothing that repeats.
/// let held: Vec<f32> = (0..3 * sr as usize)
///     .map(|i| (0.8 * (std::f64::consts::TAU * 220.0 * i as f64 / 16_000.0).sin()) as f32)
///     .collect();
/// assert_eq!(tempo_of(&held, sr), None);
/// ```
pub fn tempo_of(signal: &[f32], sample_rate: u32) -> Option<f64> {
    if sample_rate == 0 {
        return None;
    }
    let attacks = spread(&attack_envelope(signal, sample_rate));
    strongest_pulse(&attacks, 1.0 / FRAME_STEP_SECS)
}

/// Each attack smeared over ±[`ATTACK_SPREAD_SECS`] with a triangular kernel,
/// so a beat played a little early still lines up with one played a little
/// late.
fn spread(attacks: &[f64]) -> Vec<f64> {
    let reach = (ATTACK_SPREAD_SECS / FRAME_STEP_SECS).round() as usize;
    (0..attacks.len())
        .map(|i| {
            let from = i.saturating_sub(reach);
            let to = (i + reach).min(attacks.len().saturating_sub(1));
            (from..=to)
                .map(|j| attacks[j] * (1.0 - i.abs_diff(j) as f64 / (reach + 1) as f64))
                .sum()
        })
        .collect()
}

/// How much louder each frame is than the last, past the ripple floor: the
/// energy attacks of the signal, one value per [`FRAME_STEP_SECS`].
fn attack_envelope(signal: &[f32], sample_rate: u32) -> Vec<f64> {
    let rate = f64::from(sample_rate);
    let step = ((FRAME_STEP_SECS * rate).round() as usize).max(1);
    let window = ((FRAME_WINDOW_SECS * rate).round() as usize).max(1);
    if signal.len() < window {
        return Vec::new();
    }
    let loudness: Vec<f64> = (0..=(signal.len() - window) / step)
        .map(|frame| {
            let slice = &signal[frame * step..frame * step + window];
            let energy = slice
                .iter()
                .filter(|s| s.is_finite())
                .map(|&s| f64::from(s) * f64::from(s))
                .sum::<f64>()
                / window as f64;
            (10.0 * energy.max(f64::MIN_POSITIVE).log10()).max(SILENCE_DB)
        })
        .collect();
    let mut attacks = vec![0.0; loudness.len()];
    for (i, pair) in loudness.windows(2).enumerate() {
        attacks[i + 1] = (pair[1] - pair[0] - RISE_FLOOR_DB).max(0.0);
    }
    attacks
}

/// The period at which `attacks` best repeats, as BPM, or `None` when no
/// period repeats strongly enough to be a pulse.
fn strongest_pulse(attacks: &[f64], frames_per_sec: f64) -> Option<f64> {
    let energy: f64 = attacks.iter().map(|a| a * a).sum();
    if !energy.is_finite() || energy <= 0.0 {
        return None;
    }
    let shortest = (frames_per_sec * 60.0 / MAX_BPM).ceil() as usize;
    let longest = ((frames_per_sec * 60.0 / SLOWEST_SEARCHED_BPM).floor() as usize)
        .min(attacks.len().saturating_sub(1));
    if shortest < 1 || shortest > longest {
        return None;
    }
    let repeat = |lag: usize| -> f64 {
        attacks
            .iter()
            .zip(&attacks[lag..])
            .map(|(a, b)| a * b)
            .sum::<f64>()
            / energy
    };
    let bpm_at = |lag: f64| 60.0 * frames_per_sec / lag;
    let lean = |lag: usize| {
        let octaves = (bpm_at(lag as f64) / PRIOR_CENTRE_BPM).log2() / PRIOR_WIDTH_OCTAVES;
        (-0.5 * octaves * octaves).exp()
    };
    let best = (shortest..=longest)
        .max_by(|&a, &b| (repeat(a) * lean(a)).total_cmp(&(repeat(b) * lean(b))))?;
    if repeat(best) < MIN_PULSE_STRENGTH {
        return None;
    }
    // Refine between frames: the peak of the parabola through the best lag and
    // its neighbours, so the answer is not quantized to whole 10 ms steps.
    let refined = if best > shortest && best < longest {
        let (before, here, after) = (repeat(best - 1), repeat(best), repeat(best + 1));
        let curvature = before - 2.0 * here + after;
        if curvature < 0.0 {
            best as f64 + 0.5 * (before - after) / curvature
        } else {
            best as f64
        }
    } else {
        best as f64
    };
    // A pulse found below the range is felt at double time: one fold, never
    // more — the search does not go low enough for a second one to be needed.
    let found = bpm_at(refined);
    let bpm = if found < MIN_BPM { found * 2.0 } else { found };
    bpm.is_finite().then(|| bpm.clamp(MIN_BPM, MAX_BPM))
}

/// The pitch a take sits around: the median pitch of its sung notes, or `None`
/// when too little of it was sung to mean anything.
///
/// Voicing is read in **runs** — stretches of continuous voiced frames — and
/// runs shorter than [`MIN_RUN_SECS`] are set aside as breath, consonants or a
/// stray frame. What matters is whether there are sung notes, not how much of
/// the take they fill: an earlier gate measured voicing against the span it
/// covered and refused detached notes, and two phrases with a breath between
/// them, both of which are ordinary singing.
fn central_pitch(frames: &[PitchFrame]) -> Option<f64> {
    let step = match frames {
        [first, second, ..] => second.time_secs - first.time_secs,
        _ => return None,
    };
    if !step.is_finite() || step <= 0.0 {
        return None;
    }
    let voiced = |frame: &PitchFrame| frame.f0_hz.is_some_and(|hz| hz.is_finite() && hz > 0.0);
    let mut sung: Vec<f64> = Vec::new();
    let mut run: Vec<f64> = Vec::new();
    for frame in frames.iter().chain(std::iter::once(&PitchFrame {
        time_secs: f64::INFINITY,
        f0_hz: None,
        confidence: 0.0,
    })) {
        if voiced(frame) {
            run.extend(frame.f0_hz.map(f64::from));
        } else {
            if run.len() as f64 * step >= MIN_RUN_SECS {
                sung.append(&mut run);
            }
            run.clear();
        }
    }
    if (sung.len() as f64) * step < MIN_VOICED_SECS {
        return None;
    }
    median_of(sung)
}

/// The trailing window a sample's loudness is measured over when locating a
/// note's start, in seconds: long enough to average room noise, short enough
/// that a sharp onset reads as sharp.
const LOUDNESS_WINDOW_SECS: f64 = 0.005;

/// How far below a sung note's level its start is placed, at most, in dB.
///
/// A sung syllable is often led by an unvoiced consonant — an "s" measured
/// 18 dB under the vowel. The beat belongs on the vowel, where the note is, so
/// the gate sits well above a consonant and well below a vowel.
const NOTE_GATE_DB: f64 = 10.5;

/// How long a sound must actually last to be a sung note, in seconds —
/// measured on loudness, which the pitch window does not smear.
///
/// A 30 ms pitched squeak before the song reads as a ~160 ms voiced run at
/// 16 kHz, where the pitch window is 128 ms, and was taken for the first note:
/// the singer entered 350 ms late. Requiring a longer voiced run instead skipped
/// a 140 ms spoken syllable. Sound duration separates them: 30 ms against 110 ms
/// and more.
const MIN_NOTE_SOUND_SECS: f64 = 0.060;

/// How far after the first sung frame the note's level is read, in seconds —
/// long enough to reach the top of a soft, swelling onset.
const NOTE_LEVEL_SECS: f64 = 0.100;

/// Where the take's first **sung note** physically begins, in samples.
///
/// Two questions, answered by two different signals:
///
/// - **Which note**: the first run of continuous voicing at least
///   [`MIN_RUN_SECS`] long in the pitch track whose sound lasts at least
///   [`MIN_NOTE_SOUND_SECS`]. Room noise, a breath, a click and an unvoiced
///   consonant are not voiced; a brief pitched squeak is voiced but does not
///   last. None of them can be chosen.
///   (The transcription's first *note* could: room noise fires an onset at
///   t = 0, and a soft note's own onset is stamped after its first voiced
///   frame, so the first "note" began at 0.0 and a singer entered 700 ms late.)
/// - **Where it starts**: pitch frames are coarse and late — YIN needs its
///   window substantially periodic — so the start is read from loudness: the
///   stretch around the first sung frame where a short trailing RMS is at or
///   above a gate. The gate is [`NOTE_GATE_DB`] under the note's level, or
///   halfway (in dB) between the note and the room before it, whichever is
///   higher — so a consonant stays under it and so does a noisy room under a
///   quiet singer. Only the *contiguous* stretch counts: a noise blip
///   separated from the note by quieter samples does not move the start.
///
/// The search runs back at most half a pitch window plus two hops from the
/// first sung frame — the most that frame can be late.
///
/// `None` when nothing in the take is sung.
///
/// ```
/// use gooz_dsp::{Config, analyze, first_sung_note};
///
/// let sr = 48_000;
/// let mut take = vec![0.0f32; sr as usize / 2];          // half a second of room
/// take.extend((0..sr as usize).map(|i| {                  // then a sung note
///     (0.6 * (std::f64::consts::TAU * 220.0 * i as f64 / f64::from(sr)).sin()) as f32
/// }));
/// let heard = analyze(&take, sr, &Config::default())?;
/// let start = first_sung_note(&take, sr, &heard, &Config::default()).expect("a note");
/// assert!(start.abs_diff(sr as usize / 2) < 48);          // within a millisecond
/// # Ok::<(), gooz_dsp::DspError>(())
/// ```
pub fn first_sung_note(
    signal: &[f32],
    sample_rate: u32,
    transcription: &Transcription,
    cfg: &Config,
) -> Option<usize> {
    let rate = f64::from(sample_rate);
    let loudness = TrailingRms::new(signal, ((LOUDNESS_WINDOW_SECS * rate) as usize).max(1));
    sung_run_starts(&transcription.pitch_track.frames, MIN_RUN_SECS)
        .into_iter()
        .find_map(|sung_from| note_start_near(sung_from, signal.len(), rate, &loudness, cfg))
}

/// Where the note heard from `sung_from` seconds physically begins, or `None`
/// when the sound there is too brief to be a sung note.
fn note_start_near(
    sung_from: f64,
    len: usize,
    rate: f64,
    loudness: &TrailingRms,
    cfg: &Config,
) -> Option<usize> {
    let v = ((sung_from * rate).round().max(0.0) as usize).min(len.checked_sub(1)?);
    let note_end = (v + (NOTE_LEVEL_SECS * rate) as usize).min(len);
    let note_db = (v..note_end)
        .map(|i| loudness.db(i))
        .fold(f64::NEG_INFINITY, f64::max);
    let reach = cfg.window / 2 + 2 * cfg.hop.max(1);
    let lower = v.saturating_sub(reach);
    let room_db = median_of(
        (0..lower)
            .step_by(cfg.hop.max(1))
            .map(|i| loudness.db(i))
            .collect(),
    )
    .unwrap_or(f64::NEG_INFINITY);
    let gate = (note_db - NOTE_GATE_DB).max((room_db + note_db) / 2.0);

    let start = if loudness.db(v) >= gate {
        let mut start = v;
        while start > lower && loudness.db(start - 1) >= gate {
            start -= 1;
        }
        start
    } else {
        (v..note_end).find(|&i| loudness.db(i) >= gate)?
    };
    // Still sounding [`MIN_NOTE_SOUND_SECS`] later — asked at that point, not
    // as "above the gate the whole way": a 5 ms loudness window spans about
    // one period of a sung pitch and ripples with its phase, so at the gate
    // crossing of a soft onset it dips straight back under, and an unbroken
    // stretch from there was never found (an 80 ms swell was not a note).
    let later = start + ((MIN_NOTE_SOUND_SECS * rate) as usize).max(1);
    (later < len && loudness.db(later) >= gate).then_some(start)
}

/// Where each run of continuous voicing at least `min_run_secs` long begins,
/// in seconds, in order.
fn sung_run_starts(frames: &[PitchFrame], min_run_secs: f64) -> Vec<f64> {
    let step = match frames {
        [first, second, ..] => second.time_secs - first.time_secs,
        _ => return Vec::new(),
    };
    if !step.is_finite() || step <= 0.0 {
        return Vec::new();
    }
    let voiced = |frame: &PitchFrame| frame.f0_hz.is_some_and(|hz| hz.is_finite() && hz > 0.0);
    let needed = (min_run_secs / step).ceil().max(1.0) as usize;
    let mut starts = Vec::new();
    let mut run_start = None;
    for (i, frame) in frames.iter().enumerate() {
        match (voiced(frame), run_start) {
            (true, None) => run_start = Some(i),
            (false, _) => run_start = None,
            _ => {}
        }
        if let Some(start) = run_start
            && i + 1 - start == needed
            && frames[start].time_secs.is_finite()
        {
            starts.push(frames[start].time_secs);
        }
    }
    starts
}

/// Loudness in dB over a trailing window, in constant time per sample.
struct TrailingRms {
    energy: Vec<f64>,
    window: usize,
}

impl TrailingRms {
    fn new(signal: &[f32], window: usize) -> TrailingRms {
        let mut energy = Vec::with_capacity(signal.len() + 1);
        energy.push(0.0);
        let mut total = 0.0f64;
        for &sample in signal {
            let s = if sample.is_finite() {
                f64::from(sample)
            } else {
                0.0
            };
            total += s * s;
            energy.push(total);
        }
        TrailingRms { energy, window }
    }

    /// The loudness of the `window` samples ending at `i`, in dB.
    fn db(&self, i: usize) -> f64 {
        let end = (i + 1).min(self.energy.len() - 1);
        let start = end.saturating_sub(self.window);
        let mean = (self.energy[end] - self.energy[start]) / (end - start).max(1) as f64;
        10.0 * mean.max(1e-20).log10()
    }
}

/// The middle value, by order statistic — never the average of the two middle
/// values, which invents a number the take never contained. Even counts take
/// the lower middle.
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

    const SR: u32 = 16_000;

    fn frame(time_secs: f64, hz: Option<f32>) -> PitchFrame {
        PitchFrame {
            time_secs,
            f0_hz: hz,
            confidence: 0.9,
        }
    }

    /// Short decaying bursts at the given times, in seconds.
    fn hits_at(times: &[f64], total_secs: f64) -> Vec<f32> {
        let mut out = vec![0.0f32; (total_secs * f64::from(SR)) as usize];
        for &t in times {
            let start = (t * f64::from(SR)) as usize;
            for i in 0..1_600 {
                if start + i < out.len() {
                    let tone = (std::f64::consts::TAU * 330.0 * i as f64 / f64::from(SR)).sin();
                    out[start + i] += (0.8 * (-(i as f64) / 300.0).exp() * tone) as f32;
                }
            }
        }
        out
    }

    #[test]
    fn a_steady_pulse_is_found_between_frames_not_quantized_to_them() {
        // 131 BPM is 45.8 frames per beat; a whole-frame answer would be 130.4
        // or 133.3. The parabola between frames should land close to 131.
        let times: Vec<f64> = (0..12).map(|b| 0.1 + b as f64 * 60.0 / 131.0).collect();
        let bpm = tempo_of(&hits_at(&times, 6.0), SR).expect("a pulse");
        // Tight on purpose: whole frames give 130.43, which a ±1.5 tolerance
        // accepted — so removing the refinement survived an earlier version.
        assert!((bpm - 131.0).abs() < 0.3, "measured {bpm:.2}");
    }

    #[test]
    fn a_long_short_swing_follows_the_beat_not_its_halves() {
        // 2:1 swung eighths at 100 BPM: hits at 0 and 2/3 of every beat. The
        // median-of-intervals design reported 90.7 or 175.8 depending on how
        // many notes there were.
        let beat = 60.0 / 100.0;
        let times: Vec<f64> = (0..10)
            .flat_map(|b| [b as f64 * beat, (b as f64 + 2.0 / 3.0) * beat])
            .map(|t| t + 0.1)
            .collect();
        let bpm = tempo_of(&hits_at(&times, 7.0), SR).expect("swing has a pulse");
        assert!((bpm - 100.0).abs() < 3.0, "swing measured {bpm:.2}");
    }

    #[test]
    fn a_pulse_slower_than_the_range_is_doubled_not_clamped() {
        // 40 BPM repeats at no period inside 60..=180; it is found at its own
        // and folded to 80. Clamped instead, it would report 60 — a tempo that
        // is not an octave of anything that was played.
        let times: Vec<f64> = (0..8).map(|b| 0.1 + b as f64 * 1.5).collect();
        let bpm = tempo_of(&hits_at(&times, 13.0), SR).expect("a slow pulse");
        assert!((bpm - 80.0).abs() < 1.0, "40 BPM reported as {bpm:.2}");
    }

    #[test]
    fn a_pulse_after_digital_silence_is_not_drowned_by_its_first_note() {
        // Sung notes that ring into each other, after a second of exact zeros.
        // Each note rises only out of the last one's tail; the first rises out
        // of nothing. Unfloored, "nothing" is −3000 dB and that one rise holds
        // almost all of the envelope's energy, so no period can repeat against
        // it and the pulse is refused.
        let beat = 0.5;
        let mut take = vec![0.0f32; SR as usize];
        for n in 0..10 {
            for i in 0..(beat * f64::from(SR)) as usize {
                let t = i as f64 / f64::from(SR);
                let decay = (-8.0 * t / beat).exp(); // about −35 dB by the next note
                let tone = (std::f64::consts::TAU * 262.0 * (n as f64 * beat + t)).sin();
                take.push((0.7 * decay * tone) as f32);
            }
        }
        let bpm = tempo_of(&take, SR).expect("ten sung notes on a pulse");
        assert!((bpm - 120.0).abs() < 2.0, "measured {bpm:.2}");
    }

    #[test]
    fn two_close_hits_and_a_distant_one_are_not_a_pulse() {
        assert_eq!(tempo_of(&hits_at(&[0.1, 0.6, 9.1], 10.0), SR), None);
    }

    #[test]
    fn silence_and_noise_have_no_pulse() {
        assert_eq!(tempo_of(&vec![0.0; 3 * SR as usize], SR), None);
        assert_eq!(tempo_of(&[], SR), None);
        assert_eq!(tempo_of(&[0.5; 10], SR), None, "shorter than one window");
        assert_eq!(tempo_of(&hits_at(&[0.1, 0.6, 1.1], 2.0), 0), None);
    }

    #[test]
    fn a_reported_tempo_is_always_in_the_musical_range() {
        for bpm in [40.0, 55.0, 61.0, 95.0, 119.0, 150.0, 179.0, 200.0, 260.0] {
            let times: Vec<f64> = (0..16).map(|b| 0.1 + b as f64 * 60.0 / bpm).collect();
            let total = times.last().copied().unwrap_or(0.0) + 1.0;
            if let Some(found) = tempo_of(&hits_at(&times, total), SR) {
                assert!(
                    (MIN_BPM..=MAX_BPM).contains(&found),
                    "{bpm} BPM reported as {found:.1}"
                );
            }
        }
    }

    #[test]
    fn detached_notes_and_two_phrases_still_have_a_root() {
        // Eight 0.2 s notes, each followed by 0.3 s of rest: 40% voiced.
        let mut detached = Vec::new();
        for note in 0..8 {
            for i in 0..50 {
                let t = note as f64 * 0.5 + i as f64 * 0.01;
                detached.push(frame(t, (i < 20).then_some(260.0)));
            }
        }
        assert_eq!(central_pitch(&detached), Some(260.0));

        // 0.8 s sung, a 2 s breath, 0.7 s sung.
        let mut phrases: Vec<PitchFrame> = (0..80)
            .map(|i| frame(i as f64 * 0.01, Some(220.0)))
            .collect();
        phrases.extend((80..280).map(|i| frame(i as f64 * 0.01, None)));
        phrases.extend((280..350).map(|i| frame(i as f64 * 0.01, Some(220.0))));
        assert_eq!(central_pitch(&phrases), Some(220.0));
    }

    #[test]
    fn a_scattering_of_single_voiced_frames_is_not_singing() {
        let scattered: Vec<PitchFrame> = (0..400)
            .map(|i| frame(i as f64 * 0.01, (i % 7 == 0).then_some(220.0)))
            .collect();
        assert_eq!(central_pitch(&scattered), None);
    }

    #[test]
    fn a_non_finite_frame_cannot_become_the_root() {
        // 0.4 s sung at 220 Hz, then 0.5 s of frames reporting +inf. Counted as
        // voice, the infinite frames would be the majority and the lower-middle
        // median would *be* infinity. (NaN would not show this: `NaN > 0.0` is
        // already false, and a median shrugs off a minority of outliers.)
        let mut frames: Vec<PitchFrame> = (0..40)
            .map(|i| frame(i as f64 * 0.01, Some(220.0)))
            .collect();
        frames.extend((40..90).map(|i| frame(i as f64 * 0.01, Some(f32::INFINITY))));
        assert_eq!(central_pitch(&frames), Some(220.0));
    }

    /// A take that is `room_db` of noise, then a note at `note_amp` from
    /// `onset` seconds on, with a pitch track that hears the note from `heard`
    /// seconds — late, as YIN is.
    fn noisy_note(
        rate: u32,
        room_db: f64,
        note_amp: f64,
        onset: f64,
        heard: f64,
    ) -> (Vec<f32>, Transcription) {
        let rate_f = f64::from(rate);
        let len = (2.0 * rate_f) as usize;
        let room = 10f64.powf(room_db / 20.0) * 3f64.sqrt();
        let mut state = 0x5EED_u64;
        let signal: Vec<f32> = (0..len)
            .map(|i| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                let noise = ((state >> 40) as f64 / 8_388_608.0 - 1.0) * room;
                let t = i as f64 / rate_f;
                let tone = if t >= onset {
                    note_amp * (std::f64::consts::TAU * 220.0 * t).sin()
                } else {
                    0.0
                };
                (noise + tone) as f32
            })
            .collect();
        let cfg = Config::default();
        let step = cfg.hop as f64 / rate_f;
        let frames = (0..(2.0 / step) as usize)
            .map(|k| {
                let t = k as f64 * step;
                frame(t, (t >= heard).then_some(220.0))
            })
            .collect();
        let transcription = Transcription {
            pitch_track: crate::transcribe::PitchTrack { frames },
            onsets: Vec::new(),
            notes: Vec::new(),
        };
        (signal, transcription)
    }

    #[test]
    fn a_noisy_room_close_to_the_note_does_not_pull_its_start_early() {
        // A −40 dBFS room under a note whose level is about −33 dBFS: the room
        // is within 10.5 dB. Gated only 10.5 dB under the note, the gate would
        // sit *below* the room, and the contiguous search would walk back
        // through noise to its bound — 96 ms before the first heard frame at
        // 16 kHz. Gated halfway between room and note, it stops at the note.
        let rate = 16_000;
        let (signal, heard) = noisy_note(rate, -40.0, 0.0316, 0.70, 0.72);
        let start = first_sung_note(&signal, rate, &heard, &Config::default()).expect("a note");
        let error_ms = (start as f64 / f64::from(rate) - 0.70) * 1000.0;
        assert!(
            error_ms.abs() < 3.0,
            "the note's start is {error_ms:+.1} ms off"
        );
    }

    #[test]
    fn voiced_runs_are_found_in_order_and_short_ones_are_skipped() {
        let rate = 16_000;
        let step = Config::default().hop as f64 / f64::from(rate);
        let frames: Vec<PitchFrame> = (0..(2.0 / step) as usize)
            .map(|k| {
                let t = k as f64 * step;
                let voiced = (0.20..0.24).contains(&t) || (0.50..0.70).contains(&t) || t >= 1.0;
                frame(t, voiced.then_some(220.0))
            })
            .collect();
        let starts = sung_run_starts(&frames, MIN_RUN_SECS);
        assert_eq!(starts.len(), 2, "the 40 ms run is not a note: {starts:?}");
        for (found, want) in starts.iter().zip([0.50, 1.00]) {
            assert!(
                (found - want).abs() <= step,
                "a run began at {found}, not {want}"
            );
        }
    }

    #[test]
    fn a_brief_sound_is_passed_over_for_the_note_after_it() {
        // A 30 ms squeak whose voicing the pitch window smears into a long
        // run, then a real note: the squeak is voiced long enough, but its
        // sound does not last, so the note after it is the first sung note.
        let rate = 16_000;
        let rate_f = f64::from(rate);
        let signal: Vec<f32> = (0..2 * rate as usize)
            .map(|i| {
                let t = i as f64 / rate_f;
                let squeak = (0.30..0.33).contains(&t);
                let note = t >= 0.70;
                if squeak || note {
                    (0.5 * (std::f64::consts::TAU * 220.0 * t).sin()) as f32
                } else {
                    0.0
                }
            })
            .collect();
        let step = Config::default().hop as f64 / rate_f;
        let frames = (0..(2.0 / step) as usize)
            .map(|k| {
                let t = k as f64 * step;
                frame(t, ((0.28..0.46).contains(&t) || t >= 0.71).then_some(220.0))
            })
            .collect();
        let heard = Transcription {
            pitch_track: crate::transcribe::PitchTrack { frames },
            onsets: Vec::new(),
            notes: Vec::new(),
        };
        let start = first_sung_note(&signal, rate, &heard, &Config::default()).expect("a note");
        let at_ms = start as f64 / rate_f * 1000.0;
        assert!(
            (at_ms - 700.0).abs() < 3.0,
            "the first note was put at {at_ms:.1} ms"
        );
    }

    #[test]
    fn the_median_is_an_order_statistic_not_an_average() {
        assert_eq!(median_of(vec![0.4, 0.8]), Some(0.4));
        assert_eq!(median_of(vec![3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median_of(Vec::new()), None);
    }
}
