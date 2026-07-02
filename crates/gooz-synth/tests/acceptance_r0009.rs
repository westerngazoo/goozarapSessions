//! Acceptance tests for R-0009 — beat builder (Euclidean drum templates +
//! synthesized kit), realized by SPEC-0009.
//!
//! Pure offline renderer — no device, no ears. Peak detection verifies pattern
//! placement (AC3); hit counting verifies k/n density (AC4).

use gooz_ratio::{Pattern, Tempo};
use gooz_synth::{BeatVoice, DrumKind, render_beat};

const SR: u32 = 48_000;
const BOUND_EPS: f32 = 1e-6;

fn tempo() -> Tempo {
    Tempo::new(120.0, 4.0).unwrap()
}

fn bar_samples(tempo: &Tempo) -> usize {
    (tempo.bar_seconds() * f64::from(SR)).round() as usize
}

fn default_voices() -> Vec<BeatVoice> {
    vec![
        BeatVoice {
            kind: DrumKind::Kick,
            pattern: Pattern::euclidean(4, 16).unwrap(),
            level: 1.0,
        },
        BeatVoice {
            kind: DrumKind::Snare,
            pattern: Pattern::euclidean(2, 16).unwrap().rotate(4),
            level: 0.9,
        },
        BeatVoice {
            kind: DrumKind::HiHat,
            pattern: Pattern::euclidean(7, 16).unwrap(),
            level: 0.7,
        },
    ]
}

fn kick_only(pattern: Pattern) -> Vec<BeatVoice> {
    vec![BeatVoice {
        kind: DrumKind::Kick,
        pattern,
        level: 1.0,
    }]
}

/// Local maxima above `threshold` in a window around each expected sample.
fn peaks_near(samples: &[f32], expected: &[usize], tolerance: usize) -> Vec<usize> {
    let mut found = Vec::new();
    for &target in expected {
        let start = target.saturating_sub(tolerance);
        let end = (target + tolerance + 1).min(samples.len());
        let (best_idx, best_val) = samples[start..end]
            .iter()
            .enumerate()
            .map(|(i, &v)| (start + i, v.abs()))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap_or((target, 0.0));
        if best_val > 0.05 {
            found.push(best_idx);
        }
    }
    found
}

fn expected_onset_samples(pattern: &Pattern, bar_samples: usize, bar: u32) -> Vec<usize> {
    let len = pattern.len();
    let bar_start = bar as usize * bar_samples;
    pattern
        .onsets()
        .into_iter()
        .map(|step| {
            let offset = ((step as f64 / len as f64) * bar_samples as f64).round() as usize;
            bar_start + offset.min(bar_samples.saturating_sub(1))
        })
        .collect()
}

fn count_local_peaks(samples: &[f32], bar_start: usize, bar_len: usize) -> usize {
    let slice = &samples[bar_start..bar_start + bar_len];
    let threshold = slice.iter().map(|s| s.abs()).fold(0.0f32, f32::max) * 0.35;
    let mut count = 0usize;
    for i in 1..slice.len().saturating_sub(1) {
        let a = slice[i - 1].abs();
        let b = slice[i].abs();
        let c = slice[i + 1].abs();
        if b >= a && b >= c && b > threshold {
            count += 1;
        }
    }
    count
}

// AC1 — three-voice render
#[test]
fn ac1_three_voice_render_is_non_empty_and_bounded() {
    let beat = render_beat(&default_voices(), &tempo(), 2, SR);
    assert!(!beat.is_empty());
    assert!(
        beat.iter()
            .all(|s| s.is_finite() && s.abs() <= 1.0 + BOUND_EPS)
    );
}

// AC2 — bar-aligned length
#[test]
fn ac2_stem_length_is_whole_bars() {
    let t = tempo();
    let bars = 4u32;
    let beat = render_beat(&default_voices(), &t, bars, SR);
    assert_eq!(beat.len(), bars as usize * bar_samples(&t));
}

// AC3 — pattern placement (kick lane only for a clean peak map)
#[test]
fn ac3_kick_onsets_align_with_energy_peaks() {
    let pattern = Pattern::euclidean(3, 8).unwrap();
    let voices = kick_only(pattern.clone());
    let t = tempo();
    let bs = bar_samples(&t);
    let beat = render_beat(&voices, &t, 1, SR);
    let expected = expected_onset_samples(&pattern, bs, 0);
    let tolerance = (bs / pattern.len()).max(4);
    let peaks = peaks_near(&beat, &expected, tolerance);
    assert_eq!(
        peaks.len(),
        pattern.onset_count(),
        "expected {:?}, found peaks {:?}",
        expected,
        peaks
    );
}

// AC4 — higher k → more hits
#[test]
fn ac4_more_onsets_increases_hit_count() {
    let t = tempo();
    let bs = bar_samples(&t);
    let sparse = render_beat(&kick_only(Pattern::euclidean(2, 16).unwrap()), &t, 1, SR);
    let busy = render_beat(&kick_only(Pattern::euclidean(6, 16).unwrap()), &t, 1, SR);
    let sparse_hits = count_local_peaks(&sparse, 0, bs);
    let busy_hits = count_local_peaks(&busy, 0, bs);
    assert!(
        busy_hits > sparse_hits,
        "busy={busy_hits} sparse={sparse_hits}"
    );
}

// AC5 — input guards (renderer side)
#[test]
fn ac5_zero_bars_or_rate_yields_empty_without_panic() {
    assert!(render_beat(&default_voices(), &tempo(), 0, SR).is_empty());
    assert!(render_beat(&default_voices(), &tempo(), 2, 0).is_empty());
    assert!(render_beat(&[], &tempo(), 2, SR).is_empty());
}

// AC6 — deterministic
#[test]
fn ac6_render_is_deterministic() {
    let voices = default_voices();
    let t = tempo();
    let a = render_beat(&voices, &t, 3, SR);
    let b = render_beat(&voices, &t, 3, SR);
    assert_eq!(a, b);
}

// AC7 — bounded, no nan/inf
#[test]
fn ac7_output_is_clean_and_bounded() {
    let beat = render_beat(&default_voices(), &tempo(), 2, SR);
    assert!(beat.iter().all(|s| s.is_finite()));
    assert!(beat.iter().all(|s| s.abs() <= 1.0 + BOUND_EPS));
}
