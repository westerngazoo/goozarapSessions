//! R-0033 acceptance tests — the 808 voice ([`render_808`], AC1–AC5) and the
//! kick onsets it is laid on ([`pattern_onsets`]), realized by SPEC-0033
//! §2.1–§2.2.
//!
//! Every measurement follows SPEC-0033 §6:
//!
//! - harmonics over a **steady span** (decay = +∞, skipping the first 2 ms and
//!   the last 5 ms) with a **Hann window** — a rectangular one leaks the
//!   fundamental into 3f at −44 to −65 dB;
//! - pitch from **zero-crossing periods**, never from the renderer's own maths;
//! - clicks against **3×** the steepest step of a sustained sine at the highest
//!   pitch involved, with a hard-cut control built here so the check cannot pass
//!   vacuously.
//!
//! Where a test compares against a reference it computes that reference here,
//! from the formulas SPEC-0033 §2.1 pins (envelope, phase convention), not from
//! the code under test.

use std::f64::consts::TAU;
use std::ops::Range;

use gooz_ratio::Tempo;
use gooz_synth::{
    Bass808, BassNote, BeatVoice, DrumKind, Pattern, pattern_onsets, render_808, render_beat,
};

/// The rates AC3 names: a headset, a CD, and a studio interface.
const RATES: [u32; 3] = [16_000, 44_100, 48_000];

fn note(hz: f64, onset_secs: f64, duration_secs: f64) -> BassNote {
    BassNote {
        hz,
        onset_secs,
        duration_secs,
    }
}

/// A clean voice that never decays: drive 0, decay = +∞ (the AC1/AC2/AC4
/// measurement set-up).
fn held(glide_secs: f64) -> Bass808 {
    Bass808 {
        glide_secs,
        decay_secs: f64::INFINITY,
        drive: 0.0,
    }
}

/// A clean voice with a decay.
fn clean(glide_secs: f64, decay_secs: f64) -> Bass808 {
    Bass808 {
        glide_secs,
        decay_secs,
        drive: 0.0,
    }
}

fn at(rate: u32, secs: f64) -> usize {
    (secs * f64::from(rate)).round() as usize
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

fn is_silent(x: &[f32]) -> bool {
    x.iter().all(|s| *s == 0.0)
}

/// Asserts the part has sound in it: the guard that keeps the "nothing bad
/// happened" checks from passing on a renderer that renders nothing.
fn assert_sounds(x: &[f32], what: &str) {
    assert!(
        peak(x) > 0.3,
        "{what}: the part is silent (peak {}) — the note did not play",
        peak(x)
    );
}

/// The steady span of a note sounding over samples `[start, end)`: its first
/// 2 ms (attack) and last 5 ms (release) skipped, plus a sample of margin.
fn steady(rate: u32, start: usize, end: usize) -> Range<usize> {
    let r = f64::from(rate);
    (start + (0.002 * r).ceil() as usize + 1)..(end - (0.005 * r).ceil() as usize - 1)
}

/// The amplitude of the `hz` component of `x`, Hann-windowed and normalized so
/// a unit sine at `hz` reads 1.
fn amplitude_at(x: &[f32], rate: u32, hz: f64) -> f64 {
    let n = x.len();
    let r = f64::from(rate);
    let (mut re, mut im, mut sum) = (0.0f64, 0.0f64, 0.0f64);
    for (i, s) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * (TAU * i as f64 / (n - 1) as f64).cos();
        let phase = TAU * hz * i as f64 / r;
        re += w * f64::from(*s) * phase.cos();
        im += w * f64::from(*s) * phase.sin();
        sum += w;
    }
    2.0 * re.hypot(im) / sum
}

fn db(ratio: f64) -> f64 {
    20.0 * ratio.log10()
}

fn cents(hz: f64, reference: f64) -> f64 {
    1200.0 * (hz / reference).log2()
}

/// One entry per period between consecutive upward zero crossings (linearly
/// interpolated): the period's midpoint in seconds and its frequency in Hz.
fn periods(x: &[f32], rate: u32) -> Vec<(f64, f64)> {
    let r = f64::from(rate);
    let crossings: Vec<f64> = x
        .windows(2)
        .enumerate()
        .filter_map(|(i, w)| {
            let (a, b) = (f64::from(w[0]), f64::from(w[1]));
            (a < 0.0 && b >= 0.0).then(|| i as f64 + a / (a - b))
        })
        .collect();
    crossings
        .windows(2)
        .map(|c| ((c[0] + c[1]) / 2.0 / r, r / (c[1] - c[0])))
        .collect()
}

/// Checks the pitch of every period whose midpoint lies in `window` (seconds)
/// against `law`, to within 5 cents — and that there were periods to check.
fn assert_pitch_follows(
    x: &[f32],
    rate: u32,
    window: Range<f64>,
    law: impl Fn(f64) -> f64,
    what: &str,
) {
    let measured: Vec<(f64, f64)> = periods(x, rate)
        .into_iter()
        .filter(|(t, _)| window.contains(t))
        .collect();
    let lowest = law(window.start).min(law(window.end));
    let enough = ((window.end - window.start) * lowest * 0.8) as usize;
    assert!(
        measured.len() >= enough.max(2),
        "{what}: {} periods in {window:?}, expected at least {} — is anything sounding?",
        measured.len(),
        enough.max(2)
    );
    for (t, hz) in measured {
        let want = law(t);
        let off = cents(hz, want);
        assert!(
            off.abs() <= 5.0,
            "{what}: at {t:.4} s the pitch is {hz:.3} Hz where the law says {want:.3} Hz ({off:+.1} cents)"
        );
    }
}

/// The amplitude of a sine around sample `n`, from three samples:
/// `A² = (x[n]² − x[n−1]·x[n+1]) / sin²ω`, exact for a steady sine of angular
/// step `ω = 2π·hz/rate`, and blind to the phase.
fn amplitude_near(x: &[f32], n: usize, hz: f64, rate: u32) -> f64 {
    let w = TAU * hz / f64::from(rate);
    let (a, b, c) = (f64::from(x[n - 1]), f64::from(x[n]), f64::from(x[n + 1]));
    (b * b - a * c).max(0.0).sqrt() / w.sin()
}

/// The steepest sample-to-sample step and the sample it starts at.
fn steepest_step(x: &[f32]) -> (f64, usize) {
    x.windows(2)
        .enumerate()
        .map(|(i, w)| ((f64::from(w[1]) - f64::from(w[0])).abs(), i))
        .fold(
            (0.0, 0),
            |best, step| if step.0 > best.0 { step } else { best },
        )
}

/// SPEC-0033 §6 AC3: 3× the steepest step of a sustained sine at `f_max`.
fn click_bound(f_max: f64, rate: u32) -> f64 {
    3.0 * TAU * f_max / f64::from(rate)
}

/// SPEC-0033 §2.1's phrase envelope for sample `i` of a phrase `[start, end)`:
/// `min(1, τ/0.002) · e^(−τ/decay) · min(1, e/(0.005·rate))`.
fn envelope(i: usize, start: usize, end: usize, rate: u32, decay: f64) -> f64 {
    let r = f64::from(rate);
    let tau = (i - start) as f64 / r;
    let remaining = (end - 1 - i) as f64;
    (tau / 0.002).min(1.0) * (-tau / decay).exp() * (remaining / (0.005 * r)).min(1.0)
}

// ---------------------------------------------------------------------------
// AC1 — a sine at the note's pitch; one oscillator through the whole part.
// ---------------------------------------------------------------------------

#[test]
fn ac1_a_clean_note_is_a_sine_at_its_pitch() {
    for rate in [16_000, 48_000] {
        for hz in [41.2, 55.0, 79.9, 110.0, 440.0] {
            let what = format!("{hz} Hz at {rate} Hz");
            let part = render_808(&[note(hz, 0.0, 1.0)], rate, at(rate, 1.1), &held(0.08));
            let x = &part[steady(rate, 0, at(rate, 1.0))];
            let fundamental = amplitude_at(x, rate, hz);
            assert!(
                (0.9..=1.001).contains(&fundamental),
                "{what}: the note's own frequency has amplitude {fundamental:.4}; a held clean \
                 note is a unit sine there"
            );
            for harmonic in [2.0, 3.0, 5.0] {
                let level = db(amplitude_at(x, rate, harmonic * hz) / fundamental);
                assert!(
                    level <= -60.0,
                    "{what}: harmonic {harmonic} sits at {level:.1} dB under the fundamental; \
                     a clean sine has none"
                );
            }
        }
    }
}

#[test]
fn ac1_one_oscillator_runs_through_the_part_and_holds_in_silence() {
    // Three fresh phrases: A and B touch (so B re-attacks, it does not glide),
    // then 0.1 s of silence, then C. The phase starts at 0, is used before it
    // advances, advances only inside a phrase and holds through the silence
    // (SPEC-0033 §2.1 "Oscillator"). A ends on half a cycle and B on a quarter,
    // so an oscillator restarted per phrase, or left running through the
    // silence, is off by a half or a quarter cycle — never by whole ones.
    for rate in [16_000, 48_000] {
        let r = f64::from(rate);
        let notes = [
            note(55.0, 0.0, 0.3),
            note(82.5, 0.3, 0.3),
            note(55.0, 0.7, 0.3),
        ];
        let part = render_808(&notes, rate, at(rate, 1.1), &held(0.08));
        let (a_end, b_end, c_start, c_end) =
            (at(rate, 0.3), at(rate, 0.6), at(rate, 0.7), at(rate, 1.0));
        let phase_at_b = TAU * 55.0 * a_end as f64 / r;
        let phase_at_c = phase_at_b + TAU * 82.5 * (b_end - a_end) as f64 / r;
        // One oscillator: the phase advances inside each phrase and holds
        // through the silence between B and C.
        let phase = |i: usize| {
            if i < a_end {
                TAU * 55.0 * i as f64 / r
            } else if i < b_end {
                phase_at_b + TAU * 82.5 * (i - a_end) as f64 / r
            } else {
                phase_at_c + TAU * 55.0 * i.saturating_sub(c_start) as f64 / r
            }
        };
        for (what, span) in [
            ("A (the first phrase: phase from 0)", 0..a_end),
            ("B (a fresh phrase right after A)", a_end..b_end),
            ("C (after the silence: the phase held)", c_start..c_end),
        ] {
            let mut checked = 0;
            for i in steady(rate, span.start, span.end) {
                let want = phase(i).sin();
                let got = f64::from(part[i]);
                assert!(
                    (got - want).abs() <= 2e-3,
                    "{what} at {rate} Hz, sample {i}: {got:.5}, one continuous oscillator gives \
                     {want:.5}"
                );
                checked += 1;
            }
            assert!(checked > 1000, "harness: {what} has no steady span");
        }
        assert!(
            is_silent(&part[b_end..c_start]),
            "the silence between phrases is not silent"
        );
    }
}

// ---------------------------------------------------------------------------
// AC2 — glide on overlapping notes only.
// ---------------------------------------------------------------------------

const GLIDE: f64 = 0.5;

/// SPEC-0033 §2.1 `glide_hz`, restated: linear in cents, reaching `to` at
/// `glide` and staying there.
fn glide_law(from: f64, to: f64, t: f64, glide: f64) -> f64 {
    from * (to / from).powf((t / glide).clamp(0.0, 1.0))
}

#[test]
fn ac2_an_overlapping_note_glides_linearly_in_cents() {
    // 200 → 400 Hz with a 0.5 s glide and no decay (SPEC-0033 §6 AC2).
    let rate = 48_000;
    let notes = [note(200.0, 0.0, 1.0), note(400.0, 0.5, 1.5)];
    let part = render_808(&notes, rate, at(rate, 2.1), &held(GLIDE));
    let what = "200 → 400 Hz";
    assert_pitch_follows(&part, rate, 0.01..0.495, |_| 200.0, what);
    assert_pitch_follows(
        &part,
        rate,
        0.505..0.995,
        |t| glide_law(200.0, 400.0, t - 0.5, GLIDE),
        what,
    );
    assert_pitch_follows(&part, rate, 1.005..1.99, |_| 400.0, what);

    // Halfway through it sits at the geometric mean, √(200·400) ≈ 282.84 Hz:
    // the period nearest 0.75 s, less the glide's slope over how far off
    // centre that period is (1200 cents per glide time).
    let (t, hz) = periods(&part, rate)
        .into_iter()
        .min_by(|a, b| (a.0 - 0.75).abs().total_cmp(&(b.0 - 0.75).abs()))
        .expect("periods");
    let mean = (200.0f64 * 400.0).sqrt();
    let off = cents(hz, mean) - 1200.0 * (t - 0.75) / GLIDE;
    assert!(
        (t - 0.75).abs() < 0.003 && off.abs() <= 5.0,
        "halfway through the glide ({t:.4} s) the pitch is {hz:.2} Hz, {off:+.1} cents from the \
         geometric mean {mean:.2} Hz"
    );
}

#[test]
fn ac2_a_slide_does_not_re_attack() {
    // The phrase's envelope carries straight through the join: with no decay
    // the level is 1 on both sides of it, sample by sample — no second thump.
    // Also with an instant jump (glide 0).
    let rate = 48_000;
    for glide in [GLIDE, 0.0] {
        let notes = [note(200.0, 0.0, 1.0), note(400.0, 0.5, 1.5)];
        let part = render_808(&notes, rate, at(rate, 2.1), &held(glide));
        let join = at(rate, 0.5);
        let pitch = |n: usize| {
            if n < join {
                200.0
            } else if glide > 0.0 {
                glide_law(200.0, 400.0, (n - join) as f64 / f64::from(rate), glide)
            } else {
                400.0
            }
        };
        for n in (join - at(rate, 0.02))..(join + at(rate, 0.02)) {
            if n == join {
                continue; // an instant jump changes ω between the three samples
            }
            let level = amplitude_near(&part, n, pitch(n), rate);
            assert!(
                (level - 1.0).abs() <= 0.02,
                "glide {glide} s: the level is {level:.3} at {:+.2} ms from the join — the \
                 slide re-attacked",
                (n as f64 - join as f64) / f64::from(rate) * 1000.0
            );
        }
    }
}

#[test]
fn ac2_notes_that_touch_or_leave_a_gap_re_attack_at_their_own_pitch() {
    let rate = 48_000;
    for (what, first) in [
        ("touching", note(200.0, 0.0, 0.5)),
        ("after a gap", note(200.0, 0.0, 0.4)),
    ] {
        let part = render_808(
            &[first, note(400.0, 0.5, 0.5)],
            rate,
            at(rate, 1.1),
            &held(GLIDE),
        );
        let onset = at(rate, 0.5);
        assert_sounds(&part, what);
        assert_eq!(
            part[onset], 0.0,
            "{what}: the second note does not start from silence — it slid instead of \
             re-attacking"
        );
        assert_eq!(
            part[onset - 1],
            0.0,
            "{what}: the first note did not release"
        );
        // A glide would still be near 200 Hz 50 ms in (≈ 214 Hz).
        assert_pitch_follows(&part, rate, 0.503..0.6, |_| 400.0, what);
    }
}

#[test]
fn ac2_notes_that_touch_in_time_touch_in_samples() {
    // Eighth notes at 117 BPM at 44.1 kHz, each lasting exactly until the next.
    // An eighth is 11 307.69 samples, so `start + round(duration)` runs a note
    // one sample into the next at some joins — turning a re-attack into a slide
    // (SPEC-0033 finding 3). Onset and end are rounded from their own instants.
    let rate = 44_100;
    let r = f64::from(rate);
    let eighth = 30.0 / 117.0;
    let notes: Vec<BassNote> = (0..8)
        .map(|k| {
            note(
                if k % 2 == 0 { 55.0 } else { 82.5 },
                k as f64 * eighth,
                eighth,
            )
        })
        .collect();
    let naive_overlaps = (0..7).any(|k| {
        let start = (k as f64 * eighth * r).round();
        start + (eighth * r).round() > ((k + 1) as f64 * eighth * r).round()
    });
    assert!(
        naive_overlaps,
        "harness: the fixture must hit the rounding trap"
    );

    let part = render_808(&notes, rate, at(rate, 8.0 * eighth), &held(0.08));
    assert_sounds(&part, "eighth notes");
    for (k, n) in notes.iter().enumerate().skip(1) {
        let onset = (n.onset_secs * r).round() as usize;
        assert!(
            part[onset] == 0.0 && part[onset - 1] == 0.0,
            "eighth {k} (sample {onset}) slid out of the one before it instead of re-attacking: \
             {} then {}",
            part[onset - 1],
            part[onset]
        );
    }
}

#[test]
fn ac2_a_note_cut_mid_glide_glides_on_from_where_the_voice_is() {
    // A 200 Hz; B 400 Hz from 0.30 s (glides); C 100 Hz from 0.55 s cuts B
    // halfway through its glide, where the voice is at √(200·400) ≈ 282.8 Hz.
    // C glides from there — not from B's target, 400 Hz, which would jump.
    let rate = 48_000;
    let notes = [
        note(200.0, 0.0, 3.0),
        note(400.0, 0.30, 2.7),
        note(100.0, 0.55, 2.45),
    ];
    let part = render_808(&notes, rate, at(rate, 3.1), &held(GLIDE));
    let current = glide_law(200.0, 400.0, 0.25, GLIDE);
    let what = "C, cut into B mid-glide";
    assert_pitch_follows(
        &part,
        rate,
        0.31..0.545,
        |t| glide_law(200.0, 400.0, t - 0.30, GLIDE),
        "B, before C cuts it",
    );
    assert_pitch_follows(
        &part,
        rate,
        0.56..1.04,
        |t| glide_law(current, 100.0, t - 0.55, GLIDE),
        what,
    );
    assert_pitch_follows(&part, rate, 1.06..2.9, |_| 100.0, what);
}

// ---------------------------------------------------------------------------
// AC3 — a long, click-free envelope that belongs to the phrase.
// ---------------------------------------------------------------------------

#[test]
fn ac3_a_phrase_starts_from_silence_attacks_decays_and_releases_as_specified() {
    // One fresh note, 110 Hz, from 0.1 s to 0.6 s, decay 0.3 s. Its samples are
    // the first phrase's sine (phase from 0) times SPEC-0033 §2.1's envelope.
    const DECAY: f64 = 0.3;
    for rate in [16_000, 48_000] {
        let r = f64::from(rate);
        let part = render_808(
            &[note(110.0, 0.1, 0.5)],
            rate,
            at(rate, 0.8),
            &clean(0.08, DECAY),
        );
        let (start, end) = (at(rate, 0.1), at(rate, 0.6));
        assert!(
            is_silent(&part[..start]),
            "{rate} Hz: sound before the note"
        );
        assert!(is_silent(&part[end..]), "{rate} Hz: sound after the note");
        assert_sounds(&part, "a fresh note");
        assert_eq!(part[start], 0.0, "{rate} Hz: the phrase's first sample");
        assert_eq!(part[end - 1], 0.0, "{rate} Hz: the phrase's last sample");
        for (i, sample) in part.iter().enumerate().take(end).skip(start) {
            let want =
                (TAU * 110.0 * (i - start) as f64 / r).sin() * envelope(i, start, end, rate, DECAY);
            let got = f64::from(*sample);
            let region = if (i - start) as f64 / r < 0.002 {
                "the 2 ms attack"
            } else if ((end - 1 - i) as f64) < 0.005 * r {
                "the 5 ms release"
            } else {
                "the decay"
            };
            assert!(
                (got - want).abs() <= 2e-3,
                "{rate} Hz, {region}, {:.2} ms into the note: {got:.5} where the phrase envelope \
                 gives {want:.5}",
                (i - start) as f64 / r * 1000.0
            );
        }
        // The decay measured without the phase: e^(−τ/decay) from 2 ms on.
        for tau in [0.0025, 0.01, 0.1, 0.2, 0.4] {
            let level = amplitude_near(&part, start + at(rate, tau), 110.0, rate);
            let want = (-tau / DECAY).exp();
            assert!(
                (level / want - 1.0).abs() <= 0.01,
                "{rate} Hz: {tau} s in, the level is {level:.4}; full level times the decay is \
                 {want:.4}"
            );
        }
    }
}

/// A part to check for clicks, and the highest pitch in it.
struct ClickScene {
    what: String,
    notes: Vec<BassNote>,
    cfg: Bass808,
    secs: f64,
    f_max: f64,
}

fn click_scenes() -> Vec<ClickScene> {
    let mut scenes = Vec::new();
    for hz in [30.0, 40.0, 55.0, 79.9] {
        // An attack from sample 0, a release, a touching re-attack, a note
        // after a gap, and one cut by the end of the part.
        for (label, cfg) in [
            ("no decay", held(0.08)),
            ("the default decay", clean(0.08, 1.2)),
        ] {
            scenes.push(ClickScene {
                what: format!("fresh notes at {hz} Hz, {label}"),
                notes: vec![
                    note(hz, 0.0, 0.2),
                    note(hz, 0.2, 0.05),
                    note(hz, 0.3, 0.1),
                    note(hz, 0.45, 1.0),
                ],
                cfg,
                secs: 0.6,
                f_max: hz,
            });
        }
    }
    for (label, glide) in [("gliding", 0.08), ("jumping", 0.0)] {
        scenes.push(ClickScene {
            what: format!("a legato chain across 30–80 Hz, {label}"),
            notes: vec![
                note(30.0, 0.02, 0.3),
                note(79.9, 0.2, 0.3),
                note(45.0, 0.3, 0.3),
                note(60.0, 0.45, 0.4),
            ],
            cfg: held(glide),
            secs: 0.9,
            f_max: 79.9,
        });
    }
    scenes.push(ClickScene {
        what: "legato notes shorter than 5 ms inside a phrase".into(),
        notes: vec![
            note(55.0, 0.02, 0.3),
            note(79.9, 0.1, 0.003),
            note(30.0, 0.102, 0.004),
            note(70.0, 0.105, 0.002),
            note(40.0, 0.106, 0.25),
            // A phrase that ends on a short legato note: its release runs
            // across the join.
            note(55.0, 0.4, 0.2),
            note(65.0, 0.5, 0.003),
        ],
        cfg: held(0.08),
        secs: 0.7,
        f_max: 79.9,
    });
    scenes
}

#[test]
fn ac3_no_clicks_over_attacks_releases_and_joins_from_30_to_80_hz() {
    // The control first: the same enveloped sine cut hard at a peak must fail
    // the check, or the check proves nothing (SPEC-0033 §6 AC3). A cut on a
    // zero crossing is a 0× step and would pass.
    for rate in RATES {
        for hz in [30.0f64, 79.9] {
            let r = f64::from(rate);
            let cut = ((((0.2 * hz).floor() + 0.25) / hz) * r).round() as usize;
            let control: Vec<f32> = (0..cut + 100)
                .map(|i| {
                    let tau = i as f64 / r;
                    if i < cut {
                        ((tau / 0.002).min(1.0) * (TAU * hz * tau).sin()) as f32
                    } else {
                        0.0
                    }
                })
                .collect();
            assert!(
                control[cut - 1] > 0.99,
                "harness: the control is cut at a peak"
            );
            let (step, _) = steepest_step(&control);
            assert!(
                step > click_bound(hz, rate),
                "harness: a hard cut at {hz} Hz / {rate} Hz passes the click check ({step:.4})"
            );
        }
    }

    for rate in RATES {
        for scene in click_scenes() {
            let part = render_808(&scene.notes, rate, at(rate, scene.secs), &scene.cfg);
            assert_sounds(&part, &scene.what);
            let (step, i) = steepest_step(&part);
            let bound = click_bound(scene.f_max, rate);
            assert!(
                step <= bound,
                "{} at {rate} Hz: a step of {step:.4} at {:.2} ms ({:.2}× the steepest step of a \
                 {} Hz sine; the bound is 3×)",
                scene.what,
                i as f64 / f64::from(rate) * 1000.0,
                step / (bound / 3.0),
                scene.f_max
            );
        }
    }
}

#[test]
fn ac3_a_phrase_ends_with_its_last_note_and_never_goes_back() {
    // A legato note cuts the one before it; the phrase ends where the last one
    // ends, released to silence there, even though A would have lasted longer.
    let rate = 16_000;
    for (what, cutter, ends) in [
        ("a short legato note", note(70.0, 0.2, 0.003), 0.203),
        ("a longer legato note", note(70.0, 0.2, 0.1), 0.3),
    ] {
        let part = render_808(
            &[note(55.0, 0.05, 0.55), cutter],
            rate,
            at(rate, 0.7),
            &held(0.08),
        );
        let end = at(rate, ends);
        assert_sounds(&part[..end], what);
        assert_eq!(part[end - 1], 0.0, "{what}: the phrase's last sample");
        assert!(
            is_silent(&part[end..]),
            "{what}: the voice went back to the note it left"
        );
    }
}

// ---------------------------------------------------------------------------
// AC4 — drive cracks it.
// ---------------------------------------------------------------------------

/// The 3rd harmonic against the fundamental, in dB, over the steady span of a
/// held note at `drive`; and the fundamental's amplitude.
fn third_harmonic(rate: u32, hz: f64, drive: f32) -> (f64, f64) {
    let cfg = Bass808 {
        drive,
        ..held(0.08)
    };
    let part = render_808(&[note(hz, 0.0, 1.0)], rate, at(rate, 1.1), &cfg);
    let x = &part[steady(rate, 0, at(rate, 1.0))];
    let fundamental = amplitude_at(x, rate, hz);
    (
        fundamental,
        db(amplitude_at(x, rate, 3.0 * hz) / fundamental),
    )
}

#[test]
fn ac4_drive_raises_the_third_harmonic_and_never_lowers_it() {
    for (rate, hz) in [(48_000, 55.0), (16_000, 41.2), (16_000, 79.9)] {
        let mut previous = f64::NEG_INFINITY;
        for k in 0..=10 {
            let drive = k as f32 / 10.0;
            let what = format!("{hz} Hz at {rate} Hz, drive {drive}");
            let (fundamental, h3) = third_harmonic(rate, hz, drive);
            assert!(
                fundamental > 0.5,
                "{what}: the fundamental is {fundamental:.3} — the note did not play"
            );
            assert!(
                h3 >= previous - 1e-3,
                "{what}: the 3rd harmonic fell to {h3:.2} dB from {previous:.2} dB"
            );
            if k == 0 {
                assert!(
                    h3 <= -60.0,
                    "{what}: {h3:.1} dB of 3rd harmonic — drive 0 is a clean sine"
                );
            }
            if k == 10 {
                assert!(
                    h3 >= -20.0,
                    "{what}: {h3:.1} dB of 3rd harmonic — full drive must clearly crack"
                );
            }
            previous = h3;
        }
    }
}

#[test]
fn ac4_every_sample_stays_in_range_at_any_drive() {
    // A dense, undecaying chain of overlapping notes, at every drive a caller
    // could send — including ones that are not numbers.
    let rate = 16_000;
    let notes: Vec<BassNote> = (0..12)
        .map(|k| note(40.0 + 3.3 * k as f64, 0.07 * k as f64, 0.3))
        .collect();
    for drive in [
        0.0,
        0.4,
        1.0,
        -1.0,
        2.0,
        1e9,
        f32::MAX,
        f32::MIN_POSITIVE,
        -f32::MAX,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ] {
        let cfg = Bass808 {
            drive,
            ..held(0.08)
        };
        let part = render_808(&notes, rate, at(rate, 1.2), &cfg);
        assert_sounds(&part, &format!("drive {drive}"));
        for (i, s) in part.iter().enumerate() {
            assert!(
                s.is_finite() && s.abs() <= 1.0,
                "drive {drive}: sample {i} is {s}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// AC5 — total and deterministic.
// ---------------------------------------------------------------------------

/// Notes with overlaps, a touch, a gap and a tie — so that order has every
/// chance to matter.
fn busy_notes() -> Vec<BassNote> {
    vec![
        note(55.0, 0.0, 0.3),
        note(73.4, 0.2, 0.3),
        note(41.2, 0.5, 0.2),
        note(61.7, 0.7, 0.1),
        note(49.0, 0.9, 0.4),
        note(65.4, 0.9, 0.2),
        note(46.2, 1.0, 0.5),
    ]
}

#[test]
fn ac5_the_same_notes_in_any_order_render_the_same_samples() {
    let rate = 16_000;
    let len = at(rate, 1.6);
    let cfg = Bass808::default();
    let notes = busy_notes();
    let reference = render_808(&notes, rate, len, &cfg);
    assert_sounds(&reference, "the busy phrase");
    assert_eq!(
        render_808(&notes, rate, len, &cfg),
        reference,
        "the same input rendered twice differs"
    );

    let mut orders: Vec<Vec<BassNote>> = Vec::new();
    let mut reversed = notes.clone();
    reversed.reverse();
    orders.push(reversed);
    for k in 1..notes.len() {
        let mut rotated = notes.clone();
        rotated.rotate_left(k);
        orders.push(rotated);
    }
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..16 {
        let mut shuffled = notes.clone();
        for i in (1..shuffled.len()).rev() {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            shuffled.swap(i, (state >> 33) as usize % (i + 1));
        }
        orders.push(shuffled);
    }
    for order in orders {
        assert!(
            render_808(&order, rate, len, &cfg) == reference,
            "the notes in this order render differently: {:?}",
            order
                .iter()
                .map(|n| (n.hz, n.onset_secs))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn ac5_the_part_is_exactly_the_requested_length() {
    let rate = 16_000;
    for len in [0, 1, 7, 1_000, 8_000, 40_000] {
        let part = render_808(&[note(55.0, 0.0, 10.0)], rate, len, &Bass808::default());
        assert_eq!(part.len(), len, "asked for {len} samples");
        if len >= 1_000 {
            assert_sounds(&part, &format!("a {len}-sample part"));
            assert_eq!(part[len - 1], 0.0, "the note is released at the part's end");
        }
        let silent = render_808(&[], rate, len, &Bass808::default());
        assert_eq!(silent.len(), len, "no notes, {len} samples");
        assert!(is_silent(&silent), "no notes is silence");
    }
    for len in [0, 1, 48_000] {
        assert!(
            render_808(&busy_notes(), 0, len, &Bass808::default()).is_empty(),
            "a zero sample rate gives an empty part, whatever the length asked"
        );
    }
}

#[test]
fn ac5_an_onset_of_zero_plays() {
    let rate = 16_000;
    let part = render_808(
        &[note(55.0, 0.0, 0.5)],
        rate,
        at(rate, 0.6),
        &Bass808::default(),
    );
    assert_sounds(&part, "a note at onset 0");
    assert_eq!(part[0], 0.0, "it starts from silence");
}

#[test]
fn ac5_each_invalid_note_is_skipped_alone_and_beside_a_valid_one() {
    // Each bad note is placed where, were it not skipped, it would change the
    // valid note's phrase (a legato cut, a glide, a tie).
    let rate = 16_000;
    let r = f64::from(rate);
    let len = at(rate, 0.6);
    let nyquist = r / 2.0;
    let valid = note(55.0, 0.1, 0.3);
    let cases = [
        ("a NaN frequency", note(f64::NAN, 0.2, 0.2)),
        ("an infinite frequency", note(f64::INFINITY, 0.2, 0.2)),
        (
            "a negative infinite frequency",
            note(f64::NEG_INFINITY, 0.2, 0.2),
        ),
        ("a zero frequency", note(0.0, 0.2, 0.2)),
        ("a negative zero frequency", note(-0.0, 0.2, 0.2)),
        ("a negative frequency", note(-73.4, 0.2, 0.2)),
        ("a frequency at Nyquist", note(nyquist, 0.2, 0.2)),
        ("a frequency above Nyquist", note(1.5 * nyquist, 0.2, 0.2)),
        ("a NaN onset", note(73.4, f64::NAN, 0.2)),
        ("an infinite onset", note(73.4, f64::INFINITY, 0.2)),
        (
            "a negative infinite onset",
            note(73.4, f64::NEG_INFINITY, 0.2),
        ),
        ("a negative onset", note(73.4, -0.05, 0.3)),
        (
            "a negative onset that would round to sample 0",
            note(73.4, -1e-9, 0.3),
        ),
        ("a NaN duration", note(73.4, 0.2, f64::NAN)),
        ("an infinite duration", note(73.4, 0.2, f64::INFINITY)),
        (
            "a negative infinite duration",
            note(73.4, 0.2, f64::NEG_INFINITY),
        ),
        ("a zero duration", note(73.4, 0.2, 0.0)),
        ("a negative duration", note(73.4, 0.2, -0.1)),
        ("a tie that is not a note", note(f64::NAN, 0.1, 0.3)),
        ("an onset at the end", note(73.4, len as f64 / r, 0.2)),
        ("an onset past the end", note(73.4, 1.0, 0.2)),
    ];
    let cfg = Bass808::default();
    let alone = render_808(&[valid], rate, len, &cfg);
    assert_sounds(&alone, "the valid note alone");
    for (what, bad) in cases {
        let part = render_808(&[bad], rate, len, &cfg);
        assert_eq!(part.len(), len, "{what}: length");
        assert!(is_silent(&part), "{what}: it played");
        assert!(
            render_808(&[valid, bad], rate, len, &cfg) == alone,
            "{what}: it changed the valid note beside it"
        );
    }
}

#[test]
fn ac5_a_note_past_the_end_is_cut_there_with_its_release() {
    // #71's lesson: a note running past the end must still release, or the
    // loop clicks every time it wraps.
    let rate = 16_000;
    let len = at(rate, 0.5);
    assert_eq!(at(rate, 0.2 + 0.3), len, "harness: 0.2 + 0.3 ends the part");
    let cfg = held(0.08);
    let past = render_808(&[note(55.0, 0.2, 1.0)], rate, len, &cfg);
    assert_eq!(past.len(), len);
    assert_sounds(&past, "a note past the end");
    assert_eq!(
        past[len - 1],
        0.0,
        "the last sample of a note cut by the part's end is not silence"
    );
    let ends_there = render_808(&[note(55.0, 0.2, 0.3)], rate, len, &cfg);
    assert!(
        past == ends_there,
        "a note cut by the end is not the same as one that ends there"
    );
    let (step, i) = steepest_step(&past[len - at(rate, 0.006)..]);
    assert!(
        step <= click_bound(55.0, rate),
        "the release before the end steps {step:.4} at {i}"
    );
    // Durations too long to count in samples are cut the same way.
    for duration in [1e6, 1e300, f64::MAX] {
        assert!(
            render_808(&[note(55.0, 0.2, duration)], rate, len, &cfg) == ends_there,
            "a {duration} s note is not cut at the end"
        );
    }
}

#[test]
fn ac5_a_note_that_rounds_to_nothing_is_dropped() {
    // Were it kept, it would be a legato note that ends the phrase on the
    // sample it starts.
    let rate = 16_000;
    let len = at(rate, 0.7);
    let cfg = Bass808::default();
    let long = note(55.0, 0.1, 0.5);
    let alone = render_808(&[long], rate, len, &cfg);
    assert_sounds(&alone, "the long note alone");
    for (what, blip) in [
        ("a nanosecond note", note(79.9, 0.3, 1e-9)),
        (
            "a fifth of a sample",
            note(79.9, 0.3, 0.2 / f64::from(rate)),
        ),
    ] {
        assert!(
            render_808(&[long, blip], rate, len, &cfg) == alone,
            "{what} cut the phrase it fell inside"
        );
    }
}

#[test]
fn ac5_ties_on_one_sample_are_broken_by_rule_not_list_order() {
    let rate = 16_000;
    let r = f64::from(rate);
    let len = at(rate, 0.6);
    let cfg = Bass808::default();
    let render = |notes: &[BassNote]| render_808(notes, rate, len, &cfg);

    let low = note(55.0, 0.1, 0.4);
    let high = note(73.4, 0.1, 0.2);
    assert_sounds(&render(&[high]), "the higher note alone");
    assert!(
        render(&[high]) != render(&[low]),
        "harness: the two notes differ"
    );
    assert!(
        render(&[low, high]) == render(&[high, low]),
        "list order chose between two notes on one sample"
    );
    assert!(
        render(&[low, high]) == render(&[high]),
        "SPEC-0033 §2.1: the higher note wins a tie"
    );

    let short = note(55.0, 0.1, 0.2);
    let long = note(55.0, 0.1, 0.4);
    assert!(
        render(&[long, short]) == render(&[short, long]),
        "list order chose between two lengths"
    );
    assert!(
        render(&[short, long]) == render(&[long]),
        "SPEC-0033 §2.1: at one pitch, the longer note wins a tie"
    );

    // A tie is one sample, not one number of seconds.
    let late = note(73.4, 0.1 + 0.3 / r, 0.2);
    assert_eq!(
        at(rate, late.onset_secs),
        at(rate, 0.1),
        "harness: one sample"
    );
    assert!(
        render(&[low, late]) == render(&[late, low]),
        "list order chose between notes 0.3 samples apart"
    );
    assert!(
        render(&[low, late]) == render(&[late]),
        "notes that round to one sample tie"
    );
}

#[test]
fn ac5_unusable_settings_fall_back_to_defined_values() {
    // SPEC-0033 §2.1 "Sanitized settings". A legato join (so glide matters)
    // and a fresh note after it.
    let rate = 16_000;
    let len = at(rate, 1.2);
    let notes = [
        note(55.0, 0.0, 0.6),
        note(79.9, 0.3, 0.6),
        note(41.2, 0.95, 0.2),
    ];
    let render = |glide_secs: f64, decay_secs: f64, drive: f32| {
        render_808(
            &notes,
            rate,
            len,
            &Bass808 {
                glide_secs,
                decay_secs,
                drive,
            },
        )
    };

    // Glide: non-finite or ≤ 0 is an immediate jump.
    let jump = render(0.0, 1.2, 0.0);
    assert_sounds(&jump, "glide 0");
    assert!(
        render(0.08, 1.2, 0.0) != jump,
        "a real glide sounds like a jump"
    );
    for glide in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, -0.0] {
        assert!(
            render(glide, 1.2, 0.0) == jump,
            "glide {glide} is not an immediate jump"
        );
    }
    assert_pitch_follows(&jump, rate, 0.305..0.59, |_| 79.9, "a jump");

    // Decay: NaN or ≤ 0 is the default, 1.2 s; +∞ is no decay.
    let default = render(0.08, 1.2, 0.0);
    for decay in [f64::NAN, 0.0, -0.0, -1.0, f64::NEG_INFINITY] {
        assert!(
            render(0.08, decay, 0.0) == default,
            "decay {decay} is not the 1.2 s default"
        );
    }
    let undecayed = render(0.08, f64::INFINITY, 0.0);
    for tau in [0.01, 0.25] {
        let level = amplitude_near(&undecayed, at(rate, tau), 55.0, rate);
        assert!(
            (level - 1.0).abs() <= 0.01,
            "decay +∞: the level is {level:.4} at {tau} s — it decayed"
        );
    }

    // Drive: non-finite is 0; anything else is clamped to 0..=1.
    let full = render(0.08, 1.2, 1.0);
    assert!(full != default, "drive 1 sounds like drive 0");
    for drive in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.5, -f32::MAX] {
        assert!(
            render(0.08, 1.2, drive) == default,
            "drive {drive} is not drive 0"
        );
    }
    for drive in [1.5, 1e9, f32::MAX] {
        assert!(
            render(0.08, 1.2, drive) == full,
            "drive {drive} is not clamped to 1"
        );
    }
}

#[test]
fn ac5_extreme_but_valid_numbers_never_panic_and_stay_audio() {
    let rate = 16_000;
    let len = at(rate, 0.5);
    let bounded = |part: &[f32], what: &str| {
        for (i, s) in part.iter().enumerate() {
            assert!(s.is_finite() && s.abs() <= 1.0, "{what}: sample {i} is {s}");
        }
    };

    // Settings at the edges of the numbers, driven hard.
    for x in [
        f64::MIN_POSITIVE,
        5e-324,
        1e-300,
        1e300,
        f64::MAX,
        f64::INFINITY,
    ] {
        let cfg = Bass808 {
            glide_secs: x,
            decay_secs: x,
            drive: 1.0,
        };
        let part = render_808(&busy_notes(), rate, at(rate, 1.6), &cfg);
        assert_eq!(part.len(), at(rate, 1.6));
        bounded(&part, &format!("glide and decay {x}"));
    }
    let long_ring = render_808(
        &[note(55.0, 0.0, 0.5)],
        rate,
        len,
        &Bass808 {
            decay_secs: f64::MAX,
            ..held(0.08)
        },
    );
    assert_sounds(&long_ring, "decay f64::MAX");

    // Notes at the edges of the numbers.
    let edges = [
        note(f64::MIN_POSITIVE, 0.0, 0.3),
        note(1e-300, 0.1, 0.3),
        note(0.5 * f64::from(rate) - 1e-9, 0.0, 0.3),
        note(55.0, 1e-300, 0.3),
        note(55.0, f64::MAX, 0.3),
        note(55.0, 1e300, 0.3),
        note(55.0, 0.0, f64::MIN_POSITIVE),
    ];
    for n in edges {
        let part = render_808(&[n], rate, len, &Bass808::default());
        assert_eq!(part.len(), len);
        bounded(&part, &format!("{n:?}"));
    }
    assert!(
        is_silent(&render_808(
            &[note(55.0, f64::MAX, 0.3)],
            rate,
            len,
            &Bass808::default()
        )),
        "a note that starts past any end played"
    );

    // Rates at the edges: Nyquist under a note, and more samples per second
    // than the note has periods.
    for rate in [1, 2, 3, 100, u32::MAX] {
        let part = render_808(
            &[note(0.4, 0.0, 10.0), note(55.0, 0.0, 1.0)],
            rate,
            64,
            &Bass808::default(),
        );
        assert_eq!(part.len(), 64, "rate {rate}");
        bounded(&part, &format!("rate {rate}"));
    }
}

// ---------------------------------------------------------------------------
// pattern_onsets — where render_beat triggers a pattern's hits (SPEC-0033 §2.2).
// ---------------------------------------------------------------------------

fn kick_only(pattern: Pattern) -> Vec<BeatVoice> {
    vec![BeatVoice {
        kind: DrumKind::Kick,
        pattern,
        level: 1.0,
    }]
}

/// render_beat's arithmetic, restated: bar by bar, step by step, the step's
/// share of the bar rounded, then clamped into the bar.
fn render_beat_offsets(pattern: &Pattern, tempo: &Tempo, bars: u32, rate: u32) -> Vec<usize> {
    let bar = ((tempo.bar_seconds() * f64::from(rate)).round() as usize).max(1);
    let steps = pattern.len();
    (0..bars as usize)
        .flat_map(|b| {
            pattern.onsets().into_iter().map(move |step| {
                let in_bar = ((step as f64 / steps as f64) * bar as f64).round() as usize;
                b * bar + in_bar.min(bar - 1)
            })
        })
        .collect()
}

fn assert_non_decreasing(offsets: &[usize], what: &str) {
    assert!(
        offsets.windows(2).all(|w| w[0] <= w[1]),
        "{what}: offsets go backwards: {offsets:?}"
    );
}

#[test]
fn pattern_onsets_are_where_render_beat_starts_each_kick() {
    // Patterns whose hits never overlap (a kick rings 0.2 s), so each hit in
    // the beat is exactly the one-hit kick, starting from silence.
    let cases = [
        (120.0, 16_000, Pattern::euclidean(4, 16).unwrap(), 2),
        (
            117.0,
            44_100,
            Pattern::euclidean(3, 8).unwrap().rotate(1),
            3,
        ),
        (140.0, 48_000, Pattern::euclidean(3, 16).unwrap(), 2),
    ];
    for (bpm, rate, pattern, bars) in cases {
        let what = format!(
            "E({}, {}) at {bpm} BPM, {rate} Hz",
            pattern.onset_count(),
            pattern.len()
        );
        let tempo = Tempo::new(bpm, 4.0).unwrap();
        let offsets = pattern_onsets(&pattern, &tempo, bars, rate);
        assert_eq!(
            offsets.len(),
            pattern.onset_count() * bars as usize,
            "{what}: one offset per hit"
        );
        assert_non_decreasing(&offsets, &what);

        let beat = render_beat(&kick_only(pattern.clone()), &tempo, bars, rate);
        let one = render_beat(
            &kick_only(Pattern::euclidean(1, 16).unwrap()),
            &tempo,
            1,
            rate,
        );
        let hit = one.iter().rposition(|s| *s != 0.0).expect("a kick") + 1;
        let mut covered = vec![false; beat.len()];
        for &o in &offsets {
            if o > 0 {
                assert_eq!(
                    beat[o - 1],
                    0.0,
                    "{what}: sound right before the hit at {o}"
                );
            }
            let end = (o + hit).min(beat.len());
            for (j, slot) in covered.iter_mut().enumerate().take(end).skip(o) {
                assert!(
                    (beat[j] - one[j - o]).abs() <= 1e-6,
                    "{what}: the hit at {o} is not a kick starting there (sample {j})"
                );
                *slot = true;
            }
        }
        assert!(
            beat.iter().zip(&covered).all(|(s, c)| *c || *s == 0.0),
            "{what}: the beat has a kick at an offset pattern_onsets did not give"
        );
    }
}

#[test]
fn pattern_onsets_follow_render_beats_bar_and_step_arithmetic() {
    let cases = [
        (
            117.0,
            44_100,
            Pattern::euclidean(5, 12).unwrap().rotate(-2),
            4,
        ),
        (92.0, 16_000, Pattern::euclidean(7, 16).unwrap(), 3),
        (160.0, 48_000, Pattern::euclidean(16, 16).unwrap(), 1),
        (
            131.0,
            22_050,
            Pattern::euclidean(3, 16).unwrap().rotate(13),
            5,
        ),
        (60.0, 8, Pattern::euclidean(16, 16).unwrap(), 2),
    ];
    for (bpm, rate, pattern, bars) in cases {
        let what = format!(
            "E({}, {}) at {bpm} BPM, {rate} Hz",
            pattern.onset_count(),
            pattern.len()
        );
        let tempo = Tempo::new(bpm, 4.0).unwrap();
        let offsets = pattern_onsets(&pattern, &tempo, bars, rate);
        assert_eq!(
            offsets,
            render_beat_offsets(&pattern, &tempo, bars, rate),
            "{what}"
        );
        assert_non_decreasing(&offsets, &what);
    }
}

#[test]
fn pattern_onsets_can_coincide_on_a_bar_shorter_than_its_steps() {
    // 120 BPM at 4 Hz: a bar is 8 samples holding 16 steps; at 1 Hz, 2 samples.
    // Every hit still has its offset, clamped into its bar.
    let tempo = Tempo::new(120.0, 4.0).unwrap();
    for (rate, bar) in [(4, 8usize), (1, 2)] {
        let pattern = Pattern::euclidean(16, 16).unwrap();
        let offsets = pattern_onsets(&pattern, &tempo, 3, rate);
        assert_eq!(offsets.len(), 48, "{rate} Hz: one offset per hit");
        assert_eq!(offsets, render_beat_offsets(&pattern, &tempo, 3, rate));
        assert_non_decreasing(&offsets, &format!("{rate} Hz"));
        assert!(
            offsets.windows(2).any(|w| w[0] == w[1]),
            "{rate} Hz: expected coinciding offsets"
        );
        for (k, o) in offsets.iter().enumerate() {
            assert_eq!(o / bar, k / 16, "{rate} Hz: hit {k} left its bar");
        }
    }
}

#[test]
fn pattern_onsets_are_empty_for_zero_bars_zero_rate_or_no_hits() {
    let tempo = Tempo::new(120.0, 4.0).unwrap();
    let pattern = Pattern::euclidean(4, 16).unwrap();
    assert_eq!(
        pattern_onsets(&pattern, &tempo, 2, 16_000),
        vec![0, 8_000, 16_000, 24_000, 32_000, 40_000, 48_000, 56_000],
        "control: two bars of four on the floor"
    );
    assert!(
        pattern_onsets(&pattern, &tempo, 0, 16_000).is_empty(),
        "zero bars"
    );
    assert!(
        pattern_onsets(&pattern, &tempo, 2, 0).is_empty(),
        "zero rate"
    );
    assert!(
        pattern_onsets(&Pattern::euclidean(0, 16).unwrap(), &tempo, 2, 16_000).is_empty(),
        "a pattern with no hits"
    );
}

// ---------------------------------------------------------------------------
// QA sign-off (loop step 7): what verifying the implementation found untested —
// legato rolls shorter than the glide, the driven 808 trap actually plays, and
// the formulas on parts nobody wrote by hand.
// ---------------------------------------------------------------------------

mod qa_signoff {
    use super::*;
    use gooz_synth::Distortion;

    /// A note every 7 ms, each lasting 20 ms, across 30–80 Hz: every note is
    /// legato, and every 80 ms glide is cut long before it arrives, so every
    /// join starts from a pitch between two notes.
    fn dense_roll() -> (Vec<BassNote>, f64, f64) {
        let pitches = [30.0, 79.9, 45.0, 60.0, 35.0, 75.0, 50.0];
        let notes: Vec<BassNote> = (0..60)
            .map(|k| note(pitches[k % pitches.len()], 0.01 + 0.007 * k as f64, 0.02))
            .collect();
        (notes, 0.01, 0.01 + 0.007 * 59.0 + 0.02)
    }

    /// The steepest step of the voice's own held note at `hz` and `drive`, over
    /// its steady span: what a part at that drive is held to.
    fn sustained_step(hz: f64, rate: u32, drive: f32) -> f64 {
        let cfg = Bass808 {
            drive,
            ..held(0.08)
        };
        let part = render_808(&[note(hz, 0.0, 1.0)], rate, at(rate, 1.0), &cfg);
        steepest_step(&part[steady(rate, 0, at(rate, 1.0))]).0
    }

    #[test]
    fn ac2_ac3_a_legato_roll_shorter_than_the_glide_is_one_click_free_phrase() {
        let (notes, first, last_end) = dense_roll();
        for rate in RATES {
            let part = render_808(&notes, rate, at(rate, 0.6), &held(0.08));
            let (start, end) = (at(rate, first), at(rate, last_end));
            assert_sounds(&part, "the roll");
            assert!(
                is_silent(&part[..=start]) && is_silent(&part[end - 1..]),
                "{rate} Hz: the roll is not one phrase from silence to silence"
            );
            assert!(
                !part[start + 1..end - 1]
                    .windows(2)
                    .any(|w| w[0] == 0.0 && w[1] == 0.0),
                "{rate} Hz: the roll fell silent inside — a legato note re-attacked"
            );
            let (step, i) = steepest_step(&part);
            let bound = click_bound(79.9, rate);
            assert!(
                step <= bound,
                "{rate} Hz: a step of {step:.4} at {:.2} ms; the bound is {bound:.4}",
                i as f64 / f64::from(rate) * 1000.0
            );
        }
    }

    #[test]
    fn ac3_a_driven_808_steps_no_more_than_its_own_held_note() {
        // Trap plays the 808 driven (0.40 by default, owner decision). The
        // drive steepens every zero crossing of a sustained note too, so the
        // reference is the voice's own held note at the same drive.
        let (roll, _, _) = dense_roll();
        for rate in RATES {
            for drive in [0.4f32, 1.0] {
                let mut scenes = click_scenes();
                scenes.push(ClickScene {
                    what: "the legato roll".into(),
                    notes: roll.clone(),
                    cfg: held(0.08),
                    secs: 0.6,
                    f_max: 79.9,
                });
                for scene in scenes {
                    let bound = 3.0 * sustained_step(scene.f_max, rate, drive);
                    assert!(
                        bound < 0.9,
                        "harness: a hard cut at a driven peak (a step near 1) must fail"
                    );
                    let cfg = Bass808 { drive, ..scene.cfg };
                    let part = render_808(&scene.notes, rate, at(rate, scene.secs), &cfg);
                    assert_sounds(&part, &scene.what);
                    let (step, i) = steepest_step(&part);
                    assert!(
                        step <= bound,
                        "{} at {rate} Hz, drive {drive}: a step of {step:.4} at {:.2} ms; 3× \
                         the held note's is {bound:.4}",
                        scene.what,
                        i as f64 / f64::from(rate) * 1000.0
                    );
                }
            }
        }
    }

    /// SPEC-0033 §2.1 restated from its text: sanitized settings; spans in
    /// `(start, hz, end)` order, one per start sample; phrases of legato
    /// chains; the phrase envelope; a glide from the current pitch; one phase,
    /// used before it advances; then `SoftClip` at `8 · drive`.
    fn spec_render(notes: &[BassNote], rate: u32, len: usize, cfg: &Bass808) -> Vec<f32> {
        let r = f64::from(rate);
        let glide = if cfg.glide_secs.is_finite() && cfg.glide_secs > 0.0 {
            cfg.glide_secs
        } else {
            0.0
        };
        let decay = if cfg.decay_secs.is_nan() || cfg.decay_secs <= 0.0 {
            1.2
        } else {
            cfg.decay_secs
        };
        let drive = if cfg.drive.is_finite() {
            cfg.drive.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let mut spans: Vec<(usize, usize, f64)> = notes
            .iter()
            .filter(|n| n.hz.is_finite() && n.onset_secs.is_finite() && n.duration_secs.is_finite())
            .filter(|n| {
                n.hz > 0.0 && n.hz < r / 2.0 && n.duration_secs > 0.0 && n.onset_secs >= 0.0
            })
            .map(|n| {
                let start = (n.onset_secs * r).round();
                let end = ((n.onset_secs + n.duration_secs) * r)
                    .round()
                    .min(len as f64);
                (start, end, n.hz)
            })
            .filter(|&(start, end, _)| start < len as f64 && end > start)
            .map(|(start, end, hz)| (start as usize, end as usize, hz))
            .collect();
        spans.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.total_cmp(&b.2)).then(a.1.cmp(&b.1)));
        let mut phrases: Vec<Vec<(usize, usize, f64)>> = Vec::new();
        for span in spans {
            match phrases.last_mut() {
                Some(phrase) if phrase.last().is_some_and(|p| p.0 == span.0) => {
                    phrase.pop();
                    phrase.push(span);
                }
                Some(phrase) if phrase.last().is_some_and(|p| span.0 < p.1) => phrase.push(span),
                _ => phrases.push(vec![span]),
            }
        }
        let mut out = vec![0.0f32; len];
        let (mut phase, mut current) = (0.0f64, 0.0f64);
        for phrase in &phrases {
            let (p_start, p_end) = (phrase[0].0, phrase[phrase.len() - 1].1);
            for (k, &(start, end, to)) in phrase.iter().enumerate() {
                let stop = phrase.get(k + 1).map_or(end, |next| next.0);
                let from = if k == 0 { to } else { current };
                for (i, slot) in out.iter_mut().enumerate().take(stop).skip(start) {
                    let hz = if glide > 0.0 {
                        let k = ((i - start) as f64 / r / glide).min(1.0);
                        from.powf(1.0 - k) * to.powf(k)
                    } else {
                        to
                    };
                    let tau = (i - p_start) as f64 / r;
                    let left = (p_end - 1 - i) as f64;
                    let env = (tau / 0.002).min(1.0)
                        * (-tau / decay).exp()
                        * (left / (0.005 * r)).min(1.0);
                    *slot = (phase.sin() * env) as f32;
                    phase += TAU * hz / r;
                    current = hz;
                }
            }
        }
        out.iter()
            .map(|x| Distortion::SoftClip.apply(*x, 8.0 * drive))
            .collect()
    }

    /// Notes nobody would write by hand: ties on a grid, NaN and Nyquist
    /// pitches, notes shorter than the release or the glide, and notes that run
    /// past the end.
    fn random_part(rng: &mut u64, secs: f64, rate: u32) -> Vec<BassNote> {
        let mut uniform = || {
            *rng = rng
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (*rng >> 11) as f64 / (1u64 << 53) as f64
        };
        let count = 1 + (uniform() * 40.0) as usize;
        (0..count)
            .map(|_| {
                let pick = uniform();
                let hz = if pick < 0.05 {
                    f64::NAN
                } else if pick < 0.08 {
                    f64::from(rate) / 2.0
                } else {
                    30.0 + 50.0 * uniform()
                };
                let onset = if uniform() < 0.15 {
                    (uniform() * 8.0).floor() * secs / 8.0
                } else {
                    uniform() * secs
                };
                let duration = match (uniform() * 4.0) as u32 {
                    0 => uniform() * 0.004,
                    1 => uniform() * 0.05,
                    2 => uniform() * 0.4,
                    _ => uniform() * 2.0 * secs,
                };
                note(hz, onset, duration)
            })
            .collect()
    }

    #[test]
    fn ac2_ac3_ac5_random_parts_render_as_spec_0033_says_in_any_order() {
        let mut rng = 0x0033_u64;
        for trial in 0..240 {
            let rate = [8_000, 16_000, 44_100, 48_000][trial % 4];
            let cfg = Bass808 {
                glide_secs: [0.0, 0.02, 0.08, 0.5][(trial / 4) % 4],
                decay_secs: [0.3, 1.2, f64::INFINITY][trial % 3],
                drive: [0.0, 0.4, 1.0][(trial / 3) % 3],
            };
            let secs = 0.4 + (trial % 7) as f64 * 0.1;
            let len = at(rate, secs);
            let notes = random_part(&mut rng, secs, rate);
            let part = render_808(&notes, rate, len, &cfg);
            let spec = spec_render(&notes, rate, len, &cfg);
            assert_eq!(part.len(), len, "trial {trial}: length");
            for (i, (got, want)) in part.iter().zip(&spec).enumerate() {
                assert!(
                    got.is_finite() && got.abs() <= 1.0 && (got - want).abs() <= 1e-4,
                    "trial {trial}, sample {i}: {got} where SPEC-0033 gives {want} ({cfg:?}, \
                     {notes:?})"
                );
            }
            let mut reversed = notes.clone();
            reversed.reverse();
            reversed.rotate_left(trial % notes.len());
            assert!(
                render_808(&reversed, rate, len, &cfg) == part,
                "trial {trial}: the same notes in another order render differently"
            );
        }
    }
}
