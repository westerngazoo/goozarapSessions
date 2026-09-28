//! The bass a style brings, under a sung take (R-0033 / SPEC-0033).
//!
//! A style's plan names its bass ([`BassVoice`]); this module decides what that
//! bass plays and renders it on the same clock, rate and length as the voice and
//! the drums.

use gooz_model::{BassVoice, SoundPlan};
use gooz_synth::{Bass808, BassNote, DrumKind, pattern_onsets, render_808};

use crate::describe::{beat_specs_from, plan_tempo};
use crate::pipeline::bar_samples;
use crate::view::{BassView, WAVE_BUCKETS, peak_envelope};

/// The bottom of the 808's register: the root is folded by octaves into
/// `[REGISTER_LOW_HZ, 2 · REGISTER_LOW_HZ)`.
const REGISTER_LOW_HZ: f64 = 40.0;

/// The style's bass for `bars` bars at `sample_rate`, on `root_hz`, or `None`
/// when the style has no bass, or nothing for it to follow (a plan with no
/// valid kick lane, which `plan_sound` never emits but a hand-edited plan can).
///
/// An 808 hits on every kick of the style's kick lane, at `root_hz` moved by
/// octaves into its register, each hit lasting until the next kick and the last
/// until the loop ends. It is driven by the plan's `drive`.
pub(crate) fn bass_from_plan(
    plan: &SoundPlan,
    root_hz: f64,
    bars: u32,
    sample_rate: u32,
) -> Option<BassView> {
    match plan.bass? {
        BassVoice::Sub808 => {
            let kick = beat_specs_from(plan)
                .into_iter()
                .find(|lane| lane.kind == DrumKind::Kick)?;
            let pattern = kick.pattern().ok()?;
            let tempo = plan_tempo(plan);
            let len = bars as usize * bar_samples(&tempo, sample_rate);
            let hz = in_808_register(root_hz)?;
            let onsets = pattern_onsets(&pattern, &tempo, bars, sample_rate);
            let cfg = Bass808 {
                drive: plan.drive,
                ..Bass808::default()
            };
            let samples = render_808(
                &hits_until_next(&onsets, len, hz, sample_rate),
                sample_rate,
                len,
                &cfg,
            );
            Some(BassView {
                voice: BassVoice::Sub808,
                sample_rate,
                bars,
                seconds: len as f64 / f64::from(sample_rate),
                root_hz: hz,
                wave: peak_envelope(&samples, WAVE_BUCKETS),
                samples,
            })
        }
    }
}

/// `hz` moved by whole octaves (2:1) into the 808's register, or `None` for a
/// pitch that is not a positive, finite number. Each step moves one octave
/// toward the band, so it always ends.
fn in_808_register(hz: f64) -> Option<f64> {
    if !(hz.is_finite() && hz > 0.0) {
        return None;
    }
    let mut hz = hz;
    while hz >= 2.0 * REGISTER_LOW_HZ {
        hz /= 2.0;
    }
    while hz < REGISTER_LOW_HZ {
        hz *= 2.0;
    }
    Some(hz)
}

/// One note per onset at `hz`, lasting until the next onset; the last lasts
/// until `len`. A repeated onset gives a note of no length, which the voice
/// drops.
fn hits_until_next(onsets: &[usize], len: usize, hz: f64, sample_rate: u32) -> Vec<BassNote> {
    let rate = f64::from(sample_rate);
    onsets
        .iter()
        .enumerate()
        .map(|(i, &at)| {
            let until = onsets.get(i + 1).copied().unwrap_or(len);
            BassNote {
                hz,
                onset_secs: at as f64 / rate,
                duration_secs: until.saturating_sub(at) as f64 / rate,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_root_folds_by_octaves_into_the_808_register() {
        assert_eq!(in_808_register(220.0), Some(55.0));
        assert_eq!(in_808_register(55.0), Some(55.0));
        assert_eq!(in_808_register(80.0), Some(40.0));
        assert_eq!(in_808_register(40.0), Some(40.0));
        assert_eq!(in_808_register(39.9), Some(79.8));
        assert_eq!(in_808_register(10.0), Some(40.0));
        for hz in [f64::MIN_POSITIVE, 1e-300, 1e300, f64::MAX] {
            let folded = in_808_register(hz).expect("positive and finite");
            assert!((40.0..80.0).contains(&folded), "{hz} → {folded}");
        }
    }

    #[test]
    fn a_pitch_that_is_not_one_has_no_register() {
        for hz in [0.0, -55.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(in_808_register(hz), None, "{hz}");
        }
    }

    #[test]
    fn each_hit_lasts_until_the_next_and_lands_on_its_onset() {
        let rate = 44_100;
        let onsets = [0, 11_308, 22_615, 22_615, 40_000];
        let hits = hits_until_next(&onsets, 50_000, 55.0, rate);
        assert_eq!(hits.len(), onsets.len());
        for (hit, &at) in hits.iter().zip(&onsets) {
            assert_eq!((hit.onset_secs * f64::from(rate)).round() as usize, at);
        }
        let ends: Vec<usize> = hits
            .iter()
            .map(|h| ((h.onset_secs + h.duration_secs) * f64::from(rate)).round() as usize)
            .collect();
        assert_eq!(ends, vec![11_308, 22_615, 22_615, 40_000, 50_000]);
        assert_eq!(hits[2].duration_secs, 0.0, "a repeated onset has no length");
    }
}
