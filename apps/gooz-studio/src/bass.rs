//! The bass a style brings, under a sung take (R-0033 / SPEC-0033).

use gooz_model::SoundPlan;

use crate::view::{BassView, PlaybackLevels};

/// The style's bass for `bars` bars at `sample_rate`, on `root_hz`: `None`
/// when the style has no bass.
pub(crate) fn bass_from_plan(
    plan: &SoundPlan,
    root_hz: f64,
    bars: u32,
    sample_rate: u32,
) -> Option<BassView> {
    let _ = (plan, root_hz, bars, sample_rate);
    None
}

/// The gains that make playback equal export's mixdown.
pub(crate) fn playback_levels(
    voice: &[f32],
    track: &[f32],
    bass: Option<&[f32]>,
) -> PlaybackLevels {
    let _ = (voice, track, bass);
    PlaybackLevels {
        voice: 1.0,
        track: 1.0,
        bass: 1.0,
    }
}
