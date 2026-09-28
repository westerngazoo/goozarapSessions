//! The bass a style brings, under a sung take (R-0033 / SPEC-0033).
//!
//! A style's plan names its bass ([`BassVoice`]); this module decides what that
//! bass plays and renders it on the same clock, rate and length as the voice and
//! the drums. It also works out the gains the studio plays the tracks at, so
//! playback is what export writes.

use gooz_model::{BassVoice, SoundPlan};
use gooz_synth::{Bass808, BassNote, DrumKind, pattern_onsets, render_808};

use crate::describe::{beat_specs_from, plan_tempo};
use crate::pipeline::bar_samples;
use crate::view::{
    BASS_LEVEL, BEAT_LEVEL, BassView, PlaybackLevels, RIFF_LEVEL, WAVE_BUCKETS, peak_envelope,
};

/// The bottom of the 808's register: the root is folded by octaves into
/// `[REGISTER_LOW_HZ, 2 · REGISTER_LOW_HZ)`.
const REGISTER_LOW_HZ: f64 = 40.0;

/// The style's bass for `bars` bars at `sample_rate`, on `root_hz`, or `None`
/// when the style has no bass.
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
            // `plan_sound` always emits a valid kick lane (R-0026 AC5), and
            // `beat_from_plan` has already built this very pattern for the
            // drums, so neither can fail here.
            let kick = beat_specs_from(plan)
                .into_iter()
                .find(|lane| lane.kind == DrumKind::Kick)
                .expect("plan_sound always emits a kick lane");
            let pattern = kick
                .pattern()
                .expect("plan_sound only emits valid E(k, n) lanes");
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

/// The gains that make playback equal export's mixdown: each track at the level
/// `build_song` places it at, all scaled by one factor that is below 1 only when
/// their sum would pass full scale — export's peak limit, computed the same way.
pub(crate) fn playback_levels(
    voice: &[f32],
    track: &[f32],
    bass: Option<&[f32]>,
) -> PlaybackLevels {
    let bass = bass.unwrap_or(&[]);
    let at = |stem: &[f32], i: usize, level: f32| stem.get(i).map_or(0.0, |x| x * level);
    let longest = voice.len().max(track.len()).max(bass.len());
    // Summed in `mixdown`'s order (voice, drums, bass) so the peak is its peak.
    let peak = (0..longest)
        .map(|i| {
            (at(voice, i, RIFF_LEVEL) + at(track, i, BEAT_LEVEL) + at(bass, i, BASS_LEVEL)).abs()
        })
        .fold(0.0f32, f32::max);
    let scale = if peak > 1.0 { 1.0 / peak } else { 1.0 };
    PlaybackLevels {
        voice: RIFF_LEVEL * scale,
        track: BEAT_LEVEL * scale,
        bass: BASS_LEVEL * scale,
    }
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

    #[test]
    fn levels_are_export_levels_until_the_sum_would_clip() {
        let quiet = playback_levels(&[0.1, -0.1], &[0.2, 0.2], Some(&[0.1, 0.0]));
        assert_eq!(
            (quiet.voice, quiet.track, quiet.bass),
            (RIFF_LEVEL, BEAT_LEVEL, BASS_LEVEL)
        );
        let loud = playback_levels(&[0.9, 0.0], &[1.0, 0.0], Some(&[1.0, 0.0]));
        let peak = 0.9 * RIFF_LEVEL + 1.0 * BEAT_LEVEL + 1.0 * BASS_LEVEL;
        assert!((loud.voice - RIFF_LEVEL / peak).abs() < 1e-6);
        assert!((loud.track - BEAT_LEVEL / peak).abs() < 1e-6);
        assert!((loud.bass - BASS_LEVEL / peak).abs() < 1e-6);
    }
}
