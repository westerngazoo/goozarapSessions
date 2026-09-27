//! [`sound_span`] — where, inside a recording, the sound actually is.
//!
//! A take from the app is a fixed capture window, not a sound: silence (or
//! room hiss) before the user acted, the sound, then more silence. Anything
//! that treats the whole window as the sound inherits the lead-in — R-0040's
//! figure played a knock recorded 0.4 s into the window at beats 0.61, 1.49,
//! 2.41 and 3.35 instead of 0, 1, 2, 3, because every hit replayed the silence
//! first.

use std::ops::Range;

/// Below this peak a recording holds no sound worth keeping, in linear
/// amplitude: −50 dBFS.
///
/// Measured on the failure it exists for: a take of nothing but −60 dBFS room
/// hiss was peak-normalized to full scale — +60 dB of hiss, played as though it
/// were the user's instrument. A silent take (a muted mic, a denied macOS
/// permission) was accepted outright.
pub const SILENCE_FLOOR: f32 = 0.003_162_278;

/// Where the sound starts: the first sample within this ratio of the peak
/// (−20 dB). High enough that a −40 dBFS noise floor under a −6 dBFS knock is
/// not mistaken for the start of it.
const START_GATE: f32 = 0.1;

/// Where the sound ends: the last sample within this ratio of the peak
/// (−30 dB). Lower than the start gate, because a decay tail is part of what a
/// sound *is*; still above a typical noise floor, so trailing hiss is dropped.
const END_GATE: f32 = 0.031_622_78;

/// How much to keep before the start gate, in seconds. The gate trips on the
/// rise, a little after the sound physically begins; cutting exactly there
/// would clip the attack.
const PRE_ROLL_SECS: f64 = 0.005;

/// The range of `signal` that holds its sound, or `None` if it holds none.
///
/// `None` means nothing rose above [`SILENCE_FLOOR`] — silence, or hiss only.
/// That is a failed recording, not an instrument, and callers should say so
/// rather than amplify it.
///
/// Non-finite samples are ignored when looking for the peak and the gates; a
/// caller that needs them rejected should validate first.
///
/// ```
/// use gooz_dsp::sound_span;
///
/// let sr = 48_000;
/// let mut take = vec![0.0f32; sr as usize];          // one second of silence
/// for i in 24_000..24_480 { take[i] = 0.5; }         // then a 10 ms hit
/// let span = sound_span(&take, sr).expect("there is a sound");
/// assert!(span.start < 24_000 && span.start > 23_700); // just before the hit
/// assert!(span.end >= 24_480);
///
/// assert!(sound_span(&vec![0.0; 4_800], sr).is_none()); // silence is not a sound
/// ```
pub fn sound_span(signal: &[f32], sample_rate: u32) -> Option<Range<usize>> {
    let peak = signal
        .iter()
        .filter(|s| s.is_finite())
        .fold(0.0f32, |m, s| m.max(s.abs()));
    if peak < SILENCE_FLOOR {
        return None;
    }
    let loud = |gate: f32| move |s: &f32| s.is_finite() && s.abs() >= peak * gate;
    let first = signal.iter().position(loud(START_GATE))?;
    let last = signal.iter().rposition(loud(END_GATE))?;
    let pre_roll = (PRE_ROLL_SECS * f64::from(sample_rate)).round() as usize;
    Some(first.saturating_sub(pre_roll)..last + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    fn db(level: f32) -> f32 {
        10f32.powf(level / 20.0)
    }

    fn hiss(len: usize, level: f32) -> Vec<f32> {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        (0..len)
            .map(|_| {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                ((state >> 40) as f32 / 8_388_608.0 - 1.0) * level
            })
            .collect()
    }

    #[test]
    fn silence_and_hiss_are_not_a_sound() {
        assert_eq!(sound_span(&[], SR), None);
        assert_eq!(sound_span(&vec![0.0; 4_800], SR), None);
        assert_eq!(sound_span(&hiss(48_000, db(-60.0)), SR), None);
    }

    #[test]
    fn a_hit_under_a_noise_floor_is_found_where_it_is() {
        // −40 dBFS room noise, then a −6 dBFS hit at 0.7 s.
        let mut take = hiss(3 * SR as usize, db(-40.0));
        let at = (0.7 * f64::from(SR)) as usize;
        for (i, s) in take[at..at + 4_800].iter_mut().enumerate() {
            *s += db(-6.0) * (-(i as f32) / 800.0).exp();
        }
        let span = sound_span(&take, SR).expect("the hit is well above the floor");
        let pre_roll = (PRE_ROLL_SECS * f64::from(SR)) as usize;
        assert!(
            span.start + pre_roll >= at && span.start <= at,
            "started at {}, the hit is at {at}",
            span.start
        );
        assert!(
            span.end < 2 * SR as usize,
            "the trailing noise was kept: ends at {}",
            span.end
        );
    }

    #[test]
    fn the_span_never_leaves_the_signal() {
        // A hit at the very first and very last sample.
        let mut take = vec![0.0f32; 1_000];
        take[0] = 0.9;
        take[999] = 0.9;
        let span = sound_span(&take, SR).expect("two hits");
        assert_eq!(span, 0..1_000);
    }

    #[test]
    fn non_finite_samples_do_not_become_the_peak() {
        let mut take = vec![0.0f32; 1_000];
        take[10] = f32::NAN;
        take[20] = f32::INFINITY;
        take[500] = 0.5;
        let span = sound_span(&take, SR).expect("a finite hit");
        assert!(span.contains(&500) && !span.contains(&10));
    }
}
