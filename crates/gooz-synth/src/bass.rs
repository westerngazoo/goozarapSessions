//! [`render_808`] — the 808 bass: a sine sub-bass with a long decay, a glide
//! between overlapping notes, and a drive that cracks it. Realizes R-0033 /
//! SPEC-0033.

use std::f64::consts::TAU;

use crate::distortion::Distortion;

/// How long a phrase takes to reach full level, in seconds.
const ATTACK_SECS: f64 = 0.002;

/// How long a phrase takes to fall silent before it ends, in seconds.
const RELEASE_SECS: f64 = 0.005;

/// The `SoftClip` drive that `drive = 1` maps to.
const DRIVE_MAX: f32 = 8.0;

/// One note for the bass: only what the voice reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BassNote {
    /// Pitch in Hz.
    pub hz: f64,
    /// When the note starts, in seconds from the start of the part.
    pub onset_secs: f64,
    /// How long it lasts, in seconds.
    pub duration_secs: f64,
}

/// How an 808 sounds: how long overlapping notes take to slide, how long a
/// hit rings, and how hard it is driven.
///
/// ```
/// use gooz_synth::Bass808;
///
/// let cfg = Bass808::default();
/// assert_eq!((cfg.glide_secs, cfg.decay_secs, cfg.drive), (0.08, 1.2, 0.0));
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bass808 {
    /// Seconds a legato note takes to glide to its pitch. Not a positive,
    /// finite number: the pitch jumps at once.
    pub glide_secs: f64,
    /// Time constant of the exponential decay, in seconds. `+∞` never decays;
    /// NaN or not positive: the default.
    pub decay_secs: f64,
    /// Saturation amount in `0..=1`: `0` is a clean sine. Clamped; not finite:
    /// `0`.
    pub drive: f32,
}

impl Default for Bass808 {
    fn default() -> Bass808 {
        Bass808 {
            glide_secs: 0.08,
            decay_secs: 1.2,
            drive: 0.0,
        }
    }
}

/// Renders `notes` as an 808 part exactly `len` samples long.
///
/// The voice is monophonic, with one oscillator for the whole part. A note that
/// starts before the previous one has ended is **legato**: it glides from the
/// pitch the voice is at to its own, linearly in cents, over
/// [`glide_secs`](Bass808::glide_secs), and it does not re-attack. A note that
/// starts once the previous one has ended starts a new phrase from silence.
/// Each phrase attacks in 2 ms, decays exponentially and releases over its last
/// 5 ms, so its first and last samples are 0. Then the whole part goes through
/// a `tanh` drive.
///
/// Total and deterministic. A zero `sample_rate` gives an empty part; any other
/// rate gives exactly `len` samples, all in `[-1, 1]`, whatever order the notes
/// come in. A note is skipped when any of its numbers is not finite, its pitch
/// is not in `(0, rate / 2)`, its duration is not positive, its onset is
/// negative, or it starts at or past `len`. A note that runs past `len` is cut
/// there, released. Of two notes starting on the same sample, the higher plays,
/// then the longer.
///
/// ```
/// use gooz_synth::{Bass808, BassNote, render_808};
///
/// let hit = BassNote { hz: 55.0, onset_secs: 0.0, duration_secs: 0.5 };
/// let part = render_808(&[hit], 48_000, 48_000, &Bass808::default());
/// assert_eq!(part.len(), 48_000);
/// assert_eq!(part[0], 0.0); // a phrase starts from silence
/// assert!(part.iter().all(|s| s.abs() <= 1.0));
/// ```
pub fn render_808(notes: &[BassNote], sample_rate: u32, len: usize, cfg: &Bass808) -> Vec<f32> {
    if sample_rate == 0 {
        return Vec::new();
    }
    let rate = f64::from(sample_rate);
    let settings = Settings::from(cfg);
    let mut out = vec![0.0f32; len];
    let spans = spans(notes, rate, len);
    let mut voice = Oscillator::default();
    for phrase in phrases(&spans) {
        voice.play(&mut out, phrase, &settings, rate);
    }
    let drive = DRIVE_MAX * settings.drive;
    for x in &mut out {
        *x = Distortion::SoftClip.apply(*x, drive);
    }
    out
}

/// A note placed on the part: `[start, end)` in samples.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Span {
    start: usize,
    end: usize,
    hz: f64,
}

/// The playable notes as spans, in onset order, one per start sample.
fn spans(notes: &[BassNote], rate: f64, len: usize) -> Vec<Span> {
    let mut placed: Vec<Span> = notes
        .iter()
        .filter_map(|note| span_of(note, rate, len as f64))
        .collect();
    placed.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then(a.hz.total_cmp(&b.hz))
            .then(a.end.cmp(&b.end))
    });
    let mut kept: Vec<Span> = Vec::with_capacity(placed.len());
    for span in placed {
        match kept.last_mut() {
            Some(last) if last.start == span.start => *last = span,
            _ => kept.push(span),
        }
    }
    kept
}

/// Where `note` sits in a part of `len` samples, if it is playable there.
///
/// Onset and end are each rounded from their own instant, so notes that touch
/// in time also touch in samples; everything stays in `f64` until it is known
/// to fit.
fn span_of(note: &BassNote, rate: f64, len: f64) -> Option<Span> {
    let finite =
        note.hz.is_finite() && note.onset_secs.is_finite() && note.duration_secs.is_finite();
    let playable =
        note.hz > 0.0 && note.hz < rate / 2.0 && note.duration_secs > 0.0 && note.onset_secs >= 0.0;
    if !(finite && playable) {
        return None;
    }
    let start = (note.onset_secs * rate).round();
    if start >= len {
        return None;
    }
    let end = ((note.onset_secs + note.duration_secs) * rate)
        .round()
        .min(len);
    (end > start).then_some(Span {
        start: start as usize,
        end: end as usize,
        hz: note.hz,
    })
}

/// Groups spans into phrases: a fresh span and the legato spans chained to it.
/// A span is legato when it starts before the previous span's own end.
fn phrases(spans: &[Span]) -> Vec<&[Span]> {
    let mut phrases = Vec::new();
    let mut first = 0;
    for i in 1..spans.len() {
        if spans[i].start >= spans[i - 1].end {
            phrases.push(&spans[first..i]);
            first = i;
        }
    }
    if first < spans.len() {
        phrases.push(&spans[first..]);
    }
    phrases
}

/// [`Bass808`] with every unusable number replaced (SPEC-0033 §2.1).
struct Settings {
    /// `0` means an immediate jump.
    glide_secs: f64,
    decay_secs: f64,
    drive: f32,
}

impl From<&Bass808> for Settings {
    fn from(cfg: &Bass808) -> Settings {
        let glide_secs = if cfg.glide_secs.is_finite() && cfg.glide_secs > 0.0 {
            cfg.glide_secs
        } else {
            0.0
        };
        // NaN fails the comparison too; `+∞` passes and never decays.
        let decay_secs = if cfg.decay_secs > 0.0 {
            cfg.decay_secs
        } else {
            Bass808::default().decay_secs
        };
        let drive = if cfg.drive.is_finite() {
            cfg.drive.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Settings {
            glide_secs,
            decay_secs,
            drive,
        }
    }
}

/// The one oscillator: its phase, and the pitch it last sounded.
#[derive(Default)]
struct Oscillator {
    phase: f64,
    hz: f64,
}

impl Oscillator {
    /// Plays one phrase into `out`. The envelope is the phrase's, so a legato
    /// join changes nothing about the level; each legato span glides from the
    /// pitch the voice is at when it begins.
    fn play(&mut self, out: &mut [f32], phrase: &[Span], settings: &Settings, rate: f64) {
        let (Some(first), Some(last)) = (phrase.first(), phrase.last()) else {
            return;
        };
        let release = RELEASE_SECS * rate;
        for (i, span) in phrase.iter().enumerate() {
            let stop = phrase.get(i + 1).map_or(span.end, |next| next.start);
            let from = if i == 0 { span.hz } else { self.hz };
            for (n, sample) in out.iter_mut().enumerate().take(stop).skip(span.start) {
                let hz = glide_hz(
                    from,
                    span.hz,
                    (n - span.start) as f64 / rate,
                    settings.glide_secs,
                );
                let since = (n - first.start) as f64 / rate;
                let left = (last.end - 1 - n) as f64;
                let env = (since / ATTACK_SECS).min(1.0)
                    * (-since / settings.decay_secs).exp()
                    * (left / release).min(1.0);
                *sample = (self.phase.sin() * env) as f32;
                self.phase = (self.phase + TAU * hz / rate) % TAU;
                self.hz = hz;
            }
        }
    }
}

/// Pitch `t` seconds into a glide from `from` to `to`: linear in cents,
/// reaching `to` at `glide` and staying there. `glide` not positive: `to` at
/// once.
///
/// `from^(1−k) · to^k` rather than `from · (to / from)^k`, which overflows for
/// a tiny `from`.
fn glide_hz(from: f64, to: f64, t: f64, glide: f64) -> f64 {
    if glide.is_nan() || glide <= 0.0 {
        return to;
    }
    let k = (t / glide).clamp(0.0, 1.0);
    from.powf(1.0 - k) * to.powf(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(hz: f64, onset_secs: f64, duration_secs: f64) -> BassNote {
        BassNote {
            hz,
            onset_secs,
            duration_secs,
        }
    }

    #[test]
    fn a_glide_is_at_the_geometric_mean_halfway() {
        for (from, to) in [(55.0, 110.0), (80.0, 40.0), (200.0, 400.0), (41.2, 73.4)] {
            let mid = glide_hz(from, to, 0.04, 0.08);
            let expected = from.sqrt() * to.sqrt();
            assert!(
                ((mid - expected) / expected).abs() < 1e-12,
                "{from}→{to}: {mid} vs {expected}"
            );
        }
    }

    #[test]
    fn a_glide_reaches_its_target_and_stays() {
        assert_eq!(glide_hz(55.0, 110.0, 0.08, 0.08), 110.0);
        assert_eq!(glide_hz(55.0, 110.0, 3.0, 0.08), 110.0);
        assert_eq!(glide_hz(55.0, 110.0, 0.0, 0.08), 55.0);
    }

    #[test]
    fn a_glide_without_time_jumps() {
        assert_eq!(glide_hz(55.0, 110.0, 0.0, 0.0), 110.0);
        assert_eq!(glide_hz(55.0, 110.0, 0.0, -1.0), 110.0);
        assert_eq!(glide_hz(55.0, 110.0, 0.0, f64::NAN), 110.0);
    }

    #[test]
    fn a_glide_from_a_tiny_pitch_stays_finite() {
        for t in [0.0, 0.01, 0.04, 0.08] {
            let hz = glide_hz(1e-310, 50.0, t, 0.08);
            assert!(hz.is_finite() && hz >= 0.0, "t = {t}: {hz}");
        }
    }

    #[test]
    fn notes_that_touch_in_time_touch_in_samples() {
        // 117 BPM at 44.1 kHz: an eighth is 11 307.69… samples, so rounding a
        // start and a duration separately overlaps the next note by one sample.
        let rate = 44_100.0;
        let eighth = 60.0 / 117.0 / 2.0;
        let placed = spans(
            &[note(55.0, eighth, eighth), note(55.0, 2.0 * eighth, eighth)],
            rate,
            200_000,
        );
        assert_eq!(placed.len(), 2);
        assert_eq!(placed[0].end, placed[1].start);
        assert_eq!(phrases(&placed).len(), 2, "touching notes are two phrases");
    }

    #[test]
    fn of_notes_starting_together_the_highest_then_longest_plays_in_any_order() {
        let a = note(50.0, 0.0, 0.5);
        let b = note(60.0, 0.0, 0.2);
        let c = note(60.0, 0.0, 0.3);
        for order in [[a, b, c], [c, b, a], [b, a, c]] {
            let placed = spans(&order, 48_000.0, 48_000);
            assert_eq!(placed.len(), 1);
            assert_eq!((placed[0].hz, placed[0].end), (60.0, 14_400));
        }
    }

    #[test]
    fn a_legato_chain_is_one_phrase_and_a_contained_note_ends_it() {
        let rate = 1_000.0;
        let placed = spans(
            &[
                note(50.0, 0.0, 1.0),  // [0, 1000)
                note(60.0, 0.5, 0.1),  // [500, 600): legato, ends the phrase early
                note(70.0, 0.7, 0.1),  // [700, 800): after 600, so fresh
                note(80.0, 0.75, 0.1), // [750, 850): legato on the fresh one
            ],
            rate,
            10_000,
        );
        let grouped: Vec<Vec<f64>> = phrases(&placed)
            .iter()
            .map(|phrase| phrase.iter().map(|s| s.hz).collect())
            .collect();
        assert_eq!(grouped, vec![vec![50.0, 60.0], vec![70.0, 80.0]]);
    }

    #[test]
    fn unusable_settings_fall_back() {
        let odd = |glide_secs, decay_secs, drive| {
            Settings::from(&Bass808 {
                glide_secs,
                decay_secs,
                drive,
            })
        };
        let s = odd(f64::NAN, f64::NAN, f32::NAN);
        assert_eq!((s.glide_secs, s.decay_secs, s.drive), (0.0, 1.2, 0.0));
        let s = odd(-1.0, 0.0, 7.0);
        assert_eq!((s.glide_secs, s.decay_secs, s.drive), (0.0, 1.2, 1.0));
        let s = odd(f64::INFINITY, f64::INFINITY, -2.0);
        assert_eq!(
            (s.glide_secs, s.decay_secs, s.drive),
            (0.0, f64::INFINITY, 0.0)
        );
    }
}
