//! Acceptance tests for R-0004 — ratio-locked transport (metronome),
//! realized by SPEC-0004.
//!
//! TDD red: this file is written **before** the implementation exists. It is
//! the executable contract that the new items must satisfy:
//!   * `gooz_ratio::Tempo::bpm()` / `Tempo::beats_per_bar()` accessors;
//!   * `gooz_audio::ClickKind` (Accent / Beat / Subdivision);
//!   * `gooz_audio::Transport` (the sample-accurate beat/subdivision clock);
//!   * `gooz_audio::Metronome` (the RT-safe render source);
//!   * `gooz_audio::Engine::start_metronome` / `is_metronome_running`.
//!
//! Until those land the file will not compile — that is the intended red state
//! for loop step 3.
//!
//! **No audio device in CI.** Every test drives the deterministic, synchronous
//! `VirtualBackend` only; `CpalBackend` (the real device) is never instantiated.
//! Real-device behaviour (the click) is verified by ear via the demo (AC7) —
//! `cargo run -p gooz-audio --example metronome` — not in CI.
//!
//! One section per acceptance criterion (AC1–AC7); every test fn is prefixed
//! with the criterion id it verifies.
//!
//! ## AC → test mapping
//!
//! | AC  | Verified by                                                          |
//! |-----|----------------------------------------------------------------------|
//! | AC1 | `ac1_boundary_frames_match_worked_example`,                         |
//! |     | `ac1_boundary_frame_is_absolute_round_of_index`,                    |
//! |     | `ac1_frames_per_sub_is_clean_for_a_divisor_subdivision`,            |
//! |     | `ac1_boundaries_strictly_increasing_for_a_non_divisor_subdivision`  |
//! | AC2 | `ac2_click_kinds_over_one_bar_subdivision_two`,                     |
//! |     | `ac2_next_bar_downbeat_is_accent`,                                   |
//! |     | `ac2_click_kinds_beats_only_subdivision_one`                         |
//! | AC3 | `ac3_tempo_exposes_bpm_and_beats_per_bar`,                          |
//! |     | `ac3_transport_built_from_tempo_reflects_it`                         |
//! | AC4 | `ac4_click_voices_have_descending_peak_amplitude`                    |
//! | AC5 | `ac5_render_is_block_invariant`,                                     |
//! |     | `ac5_downbeat_click_begins_at_frame_zero_with_silence_between`,      |
//! |     | `ac5_stereo_render_writes_every_channel_equally`,                    |
//! |     | `ac5_degenerate_config_does_not_panic_or_stall`                      |
//! | AC6 | `ac6_start_metronome_runs_and_excludes_playback`,                   |
//! |     | `ac6_metronome_clicks_at_expected_boundaries_via_virtual_backend`,  |
//! |     | `ac6_start_playback_stops_a_running_metronome`,                      |
//! |     | `ac6_stop_clears_both_metronome_and_playback`                        |
//! | AC7 | verified by ear via `examples/metronome.rs` on a real machine;      |
//! |     | **not** a CI test — see the AC7 note (no device is opened here).     |
//! |     | The four toolchain gates are verified at QA sign-off (step 7).       |
//!
//! ## API assumptions beyond SPEC-0004 §2/§3
//!
//! SPEC-0004 §2/§3 fix every constructor, accessor, method, and enum name these
//! tests call; nothing was inferred. The notes below pin the exact
//! spelling/semantics the tests rely on so any mismatch is caught at the
//! implementation step, not blamed on a test typo:
//!
//! * `Tempo::bpm(&self) -> f64` and `Tempo::beats_per_bar(&self) -> f64`,
//!   returning the values passed to `Tempo::new` verbatim — spec §2
//!   ("`Tempo` accessors").
//! * `ClickKind` is a fieldless enum (`Accent`, `Beat`, `Subdivision`) deriving
//!   `Debug, Clone, Copy, PartialEq, Eq` — spec §2 verbatim.
//! * `Transport::new(sample_rate: u32, tempo: &Tempo, subdivision: u32)
//!   -> Transport` with `boundary_frame(&self, index: u64) -> u64`,
//!   `click_kind(&self, index: u64) -> ClickKind`, `subdivision(&self) -> u32`,
//!   `beats_per_bar(&self) -> u32`, and `frames_per_sub(&self) -> f64` — spec §2
//!   ("`ClickKind` + `Transport`"). `boundary_frame(index) ==
//!   round(index * frames_per_sub)` computed absolutely;
//!   `frames_per_sub == sample_rate * tempo.seconds_per_beat() / subdivision`.
//! * `Metronome::new(sample_rate: u32, tempo: &Tempo, subdivision: u32,
//!   channels: u16) -> Metronome` with `render(&mut self, output: &mut [f32])`
//!   — spec §2/§3 ("`Metronome`"). `render` writes each frame's click value to
//!   every channel of that frame (`output.chunks_mut(channels)`), is
//!   block-invariant across calls, and fires every boundary at or before the
//!   current frame (`<= pos`). The three ticks are ~30 ms enveloped sines with
//!   peaks accent 0.9 / beat 0.6 / subdivision 0.3 (spec §2, AC4).
//! * `Engine::start_metronome(&mut self, metronome: Metronome)
//!   -> Result<(), AudioError>` and `Engine::is_metronome_running(&self) -> bool`
//!   — spec §2 ("`Engine::start_metronome`"). Metronome and take playback are
//!   mutually exclusive: starting one drops the other; `stop()` clears both;
//!   `is_playing()` stays take-only. `VirtualBackend`, `Take`, `start_playback`,
//!   `stop`, and `pull_output` are as established by R-0003.

use gooz_audio::{ClickKind, Engine, Metronome, Take, Transport, VirtualBackend};
use gooz_ratio::Tempo;

/// The v0 device sample rate the worked examples pin against.
const SAMPLE_RATE: u32 = 48_000;
/// v0's mono target — a frame is a single sample (R-0003 §4).
const MONO: u16 = 1;
/// A callback block size used to drive the `VirtualBackend`.
const BLOCK: usize = 64;

/// The 4/4, 120 BPM tempo used across the suite. 120 BPM → 0.5 s/beat; at
/// 48000 Hz with subdivision 2 that is exactly 12000 frames per subdivision.
fn tempo_120_4() -> Tempo {
    Tempo::new(120.0, 4.0).expect("120 BPM / 4 beats-per-bar is a valid tempo")
}

/// Peak absolute amplitude over a window `[start, start + len)` of `buf`.
fn peak_abs(buf: &[f32], start: usize, len: usize) -> f32 {
    buf[start..(start + len).min(buf.len())]
        .iter()
        .fold(0.0f32, |acc, &s| acc.max(s.abs()))
}

// ---------------------------------------------------------------------------
// AC1 — Sample-accurate boundaries
// ---------------------------------------------------------------------------

#[test]
fn ac1_boundary_frames_match_worked_example() {
    // 48000 Hz, 120 BPM (0.5 s/beat), subdivision 2 → 12000 frames per sub.
    let tempo = tempo_120_4();
    let transport = Transport::new(SAMPLE_RATE, &tempo, 2);

    assert_eq!(transport.boundary_frame(0), 0, "boundary(0) is frame 0");
    assert_eq!(transport.boundary_frame(1), 12_000);
    assert_eq!(transport.boundary_frame(2), 24_000);
    assert_eq!(transport.boundary_frame(3), 36_000);

    // Strictly increasing over the first ~16 indices (sane tempo).
    let mut prev = transport.boundary_frame(0);
    for index in 1..16u64 {
        let next = transport.boundary_frame(index);
        assert!(
            next > prev,
            "boundary({index}) = {next} must exceed boundary({}) = {prev}",
            index - 1
        );
        prev = next;
    }
}

#[test]
fn ac1_boundary_frame_is_absolute_round_of_index() {
    // boundary_frame(k) == round(k * frames_per_sub), computed from k absolutely
    // (no cumulative error). Checked against the transport's own frames_per_sub
    // so the contract holds for any clean tempo, not just the worked example.
    let tempo = tempo_120_4();
    let transport = Transport::new(SAMPLE_RATE, &tempo, 2);

    let fps = transport.frames_per_sub();
    for index in 0..32u64 {
        let expected = (index as f64 * fps).round() as u64;
        assert_eq!(
            transport.boundary_frame(index),
            expected,
            "boundary({index}) must equal round({index} * frames_per_sub)"
        );
    }
}

#[test]
fn ac1_frames_per_sub_is_clean_for_a_divisor_subdivision() {
    // frames_per_sub == sample_rate * seconds_per_beat / subdivision
    //               == 48000 * 0.5 / 2 == 12000.0 exactly.
    let tempo = tempo_120_4();
    let transport = Transport::new(SAMPLE_RATE, &tempo, 2);
    assert_eq!(
        transport.frames_per_sub(),
        12_000.0,
        "48000 * 0.5 / 2 is exactly 12000 frames per subdivision"
    );
}

#[test]
fn ac1_boundaries_strictly_increasing_for_a_non_divisor_subdivision() {
    // Subdivision 3 does not divide 48000 * 0.5 = 24000 evenly
    // (24000 / 3 == 8000.0 actually divides cleanly here), so use a tempo whose
    // frames-per-beat is not a multiple of 3 to exercise rounding. 100 BPM →
    // 0.6 s/beat → 28800 frames/beat; /3 == 9600.0 (clean). Use a fractional
    // case: 110 BPM → 60/110 s/beat → 48000 * 0.545454... / 3 ≈ 8727.27, which
    // forces rounding at every boundary. The contract is monotonic
    // strictly-increasing despite rounding.
    let tempo = Tempo::new(110.0, 4.0).expect("110 BPM is a valid tempo");
    let transport = Transport::new(SAMPLE_RATE, &tempo, 3);

    // frames_per_sub is non-integral here, so boundaries land on rounded frames.
    assert!(
        transport.frames_per_sub().fract() != 0.0,
        "this case is chosen so frames_per_sub is non-integral (forces rounding)"
    );
    assert_eq!(transport.boundary_frame(0), 0, "boundary(0) is always 0");

    let mut prev = transport.boundary_frame(0);
    for index in 1..24u64 {
        let next = transport.boundary_frame(index);
        assert!(
            next > prev,
            "rounded boundary({index}) = {next} must still strictly exceed {prev}"
        );
        // Each rounded boundary stays within half a frame of the ideal position.
        let ideal = index as f64 * transport.frames_per_sub();
        assert!(
            (next as f64 - ideal).abs() <= 0.5,
            "boundary({index}) = {next} must be the rounded ideal {ideal}"
        );
        prev = next;
    }
}

// ---------------------------------------------------------------------------
// AC2 — Beat/bar classification & accent
// ---------------------------------------------------------------------------

#[test]
fn ac2_click_kinds_over_one_bar_subdivision_two() {
    // 4/4, subdivision 2 → 8 boundaries per bar. The worked example:
    // Accent, Sub, Beat, Sub, Beat, Sub, Beat, Sub.
    let tempo = tempo_120_4();
    let transport = Transport::new(SAMPLE_RATE, &tempo, 2);

    let kinds: Vec<ClickKind> = (0..8u64).map(|i| transport.click_kind(i)).collect();
    let expected = [
        ClickKind::Accent,
        ClickKind::Subdivision,
        ClickKind::Beat,
        ClickKind::Subdivision,
        ClickKind::Beat,
        ClickKind::Subdivision,
        ClickKind::Beat,
        ClickKind::Subdivision,
    ];
    assert_eq!(
        kinds, expected,
        "one 4/4 bar with subdivision 2 classifies as Accent/Sub/Beat/Sub/Beat/Sub/Beat/Sub"
    );
}

#[test]
fn ac2_next_bar_downbeat_is_accent() {
    // Index 8 is the next bar's downbeat (beat 4, 4 % 4 == 0) → Accent.
    let tempo = tempo_120_4();
    let transport = Transport::new(SAMPLE_RATE, &tempo, 2);
    assert_eq!(
        transport.click_kind(8),
        ClickKind::Accent,
        "the first boundary of the next bar is an accented downbeat"
    );
    assert_eq!(
        transport.click_kind(16),
        ClickKind::Accent,
        "and so is the downbeat of the bar after that"
    );
}

#[test]
fn ac2_click_kinds_beats_only_subdivision_one() {
    // Subdivision 1 → every boundary is a beat; the first of each 4/4 bar is the
    // accented downbeat: Accent, Beat, Beat, Beat.
    let tempo = tempo_120_4();
    let transport = Transport::new(SAMPLE_RATE, &tempo, 1);

    let kinds: Vec<ClickKind> = (0..4u64).map(|i| transport.click_kind(i)).collect();
    let expected = [
        ClickKind::Accent,
        ClickKind::Beat,
        ClickKind::Beat,
        ClickKind::Beat,
    ];
    assert_eq!(
        kinds, expected,
        "subdivision 1 over a 4/4 bar is Accent then three plain Beats"
    );
    assert_eq!(
        transport.click_kind(4),
        ClickKind::Accent,
        "boundary 4 is the next bar's downbeat"
    );
}

// ---------------------------------------------------------------------------
// AC3 — Built from `Tempo`
// ---------------------------------------------------------------------------

#[test]
fn ac3_tempo_exposes_bpm_and_beats_per_bar() {
    let tempo = tempo_120_4();
    assert_eq!(
        tempo.bpm(),
        120.0,
        "Tempo exposes the BPM it was built with"
    );
    assert_eq!(
        tempo.beats_per_bar(),
        4.0,
        "Tempo exposes the beats-per-bar it was built with"
    );
}

#[test]
fn ac3_transport_built_from_tempo_reflects_it() {
    // The transport is constructed from a Tempo plus sample rate + subdivision,
    // so the rhythm core drives the engine.
    let tempo = tempo_120_4();
    let transport = Transport::new(SAMPLE_RATE, &tempo, 2);
    assert_eq!(
        transport.beats_per_bar(),
        4,
        "the transport carries the tempo's beats-per-bar (rounded to u32)"
    );
    assert_eq!(
        transport.subdivision(),
        2,
        "the transport carries the requested subdivision"
    );
}

// ---------------------------------------------------------------------------
// AC4 — Distinct click voices
// ---------------------------------------------------------------------------

#[test]
fn ac4_click_voices_have_descending_peak_amplitude() {
    // Render one full 4/4 bar (8 subdivisions) of mono click and inspect the
    // onset window of an accent, a beat, and a subdivision boundary. Peaks must
    // descend accent > beat > subdivision (~0.9 / 0.6 / 0.3). The envelope and
    // sine phase make exact samples awkward, so assert ordering + rough
    // magnitude with tolerance, not exact values.
    let tempo = tempo_120_4();
    let transport = Transport::new(SAMPLE_RATE, &tempo, 2);
    let mut metronome = Metronome::new(SAMPLE_RATE, &tempo, 2, MONO);

    // One bar plus a margin so the last subdivision's tick fits fully.
    let bar_frames = transport.boundary_frame(8) as usize;
    let mut out = vec![0.0f32; bar_frames + 4_096];
    metronome.render(&mut out);

    // Click onset windows: ~30 ms tick; the peak lives near the onset, so a
    // 64-sample window from each boundary captures it.
    let window = 64usize;
    let accent_peak = peak_abs(&out, transport.boundary_frame(0) as usize, window); // index 0
    let beat_peak = peak_abs(&out, transport.boundary_frame(2) as usize, window); // index 2 = beat 1
    let sub_peak = peak_abs(&out, transport.boundary_frame(1) as usize, window); // index 1 = subdivision

    assert!(
        accent_peak > beat_peak,
        "accent peak ({accent_peak}) must exceed beat peak ({beat_peak})"
    );
    assert!(
        beat_peak > sub_peak,
        "beat peak ({beat_peak}) must exceed subdivision peak ({sub_peak})"
    );

    // Rough magnitude tiers with generous tolerance (the linear-decay envelope
    // means the captured peak is at or just under the tick's design peak).
    let tol = 0.2f32;
    assert!(
        (accent_peak - 0.9).abs() <= tol,
        "accent peak ({accent_peak}) is near the 0.9 design amplitude"
    );
    assert!(
        (beat_peak - 0.6).abs() <= tol,
        "beat peak ({beat_peak}) is near the 0.6 design amplitude"
    );
    assert!(
        (sub_peak - 0.3).abs() <= tol,
        "subdivision peak ({sub_peak}) is near the 0.3 design amplitude"
    );
}

// ---------------------------------------------------------------------------
// AC5 — Real-time-safe, block-invariant render
// ---------------------------------------------------------------------------

#[test]
fn ac5_render_is_block_invariant() {
    // Rendering a span as one large block must equal rendering it as many small,
    // frame-aligned blocks — sample-for-sample. Mono so a frame is one sample.
    let tempo = tempo_120_4();

    let total = 30_000usize; // > one beat (12000) + change, covers several clicks
    let mut one_block = vec![0.0f32; total];
    Metronome::new(SAMPLE_RATE, &tempo, 2, MONO).render(&mut one_block);

    let mut chunked = vec![0.0f32; total];
    let mut metronome = Metronome::new(SAMPLE_RATE, &tempo, 2, MONO);
    let chunk = 100usize; // frame-aligned in mono (one frame == one sample)
    let mut start = 0usize;
    while start < total {
        let end = (start + chunk).min(total);
        metronome.render(&mut chunked[start..end]);
        start = end;
    }

    assert_eq!(
        one_block, chunked,
        "one big render must equal many small frame-aligned renders, sample-for-sample"
    );
}

#[test]
fn ac5_downbeat_click_begins_at_frame_zero_with_silence_between() {
    // The accent click is placed at frame 0 (boundary(0)). The very first sample
    // may be ~0 because the sine starts at phase 0, so assert that the onset
    // window holds the bar's largest peak and that there is genuine silence in
    // the gap between the first subdivision tick's end and the next beat.
    let tempo = tempo_120_4();
    let transport = Transport::new(SAMPLE_RATE, &tempo, 2);
    let mut metronome = Metronome::new(SAMPLE_RATE, &tempo, 2, MONO);

    let bar_frames = transport.boundary_frame(8) as usize;
    let mut out = vec![0.0f32; bar_frames + 4_096];
    metronome.render(&mut out);

    // The accent at frame 0 is the loudest click in the bar.
    let accent_peak = peak_abs(&out, 0, 64);
    let beat_peak = peak_abs(&out, transport.boundary_frame(2) as usize, 64);
    assert!(
        accent_peak >= beat_peak,
        "the downbeat accent at frame 0 is at least as loud as any beat click"
    );
    assert!(
        accent_peak > 0.5,
        "the accent click is clearly present at the start (peak {accent_peak})"
    );

    // A tick is ~30 ms (~1440 frames @ 48k); the gap before the next boundary at
    // 12000 frames is therefore long. Sample a window safely inside that gap and
    // require it to be exactly silent (the tick has ended, the next not begun).
    let gap_probe = 6_000usize; // well after the accent tick, well before boundary(1)
    assert_eq!(
        peak_abs(&out, gap_probe, 256),
        0.0,
        "there is true silence between clicks (a finite tick, then zeros)"
    );
}

#[test]
fn ac5_stereo_render_writes_every_channel_equally() {
    // Channel-aware render: each frame's click value is written to every channel.
    // With channels = 2 the interleaved buffer has L == R for every frame, and
    // the click onset frame matches the mono render.
    let tempo = tempo_120_4();
    let stereo_channels = 2u16;

    let frames = 20_000usize; // covers boundary 0 and boundary 1 (12000)
    let mut stereo = vec![0.0f32; frames * stereo_channels as usize];
    Metronome::new(SAMPLE_RATE, &tempo, 2, stereo_channels).render(&mut stereo);

    // L == R for every frame.
    for (frame_index, frame) in stereo.chunks_exact(stereo_channels as usize).enumerate() {
        assert_eq!(
            frame[0], frame[1],
            "frame {frame_index}: both channels carry the same click value"
        );
    }

    // The onset must match a mono render of the same span: extract the
    // left-channel signal and compare to the mono buffer sample-for-sample.
    let mut mono = vec![0.0f32; frames];
    Metronome::new(SAMPLE_RATE, &tempo, 2, MONO).render(&mut mono);
    let left: Vec<f32> = stereo
        .chunks_exact(stereo_channels as usize)
        .map(|frame| frame[0])
        .collect();
    assert_eq!(
        left, mono,
        "the stereo left channel equals the mono render frame-for-frame"
    );
}

#[test]
fn ac5_degenerate_config_does_not_panic_or_stall() {
    // A degenerate config (very high subdivision so frames_per_sub may be < 1,
    // collapsing several boundaries onto one frame) must not panic or stall: the
    // while-advance firing fires them all (last wins) and always advances, and
    // the .max(1) tick length keeps buf[cursor] in bounds. Just rendering
    // without panicking is the contract; we also confirm it produced sound.
    let tempo = tempo_120_4();
    let mut metronome = Metronome::new(SAMPLE_RATE, &tempo, 1_000_000, MONO);

    let mut out = vec![0.0f32; 4_096];
    metronome.render(&mut out); // must return; no panic, no infinite loop

    assert!(
        out.iter().any(|&s| s != 0.0),
        "even a degenerate subdivision still emits a click (no silent stall)"
    );
}

// ---------------------------------------------------------------------------
// AC6 — Engine integration (deterministic backend, CI)
// ---------------------------------------------------------------------------

#[test]
fn ac6_start_metronome_runs_and_excludes_playback() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let mut engine = Engine::new(backend);
    let tempo = tempo_120_4();

    assert!(
        !engine.is_metronome_running(),
        "a fresh engine has no metronome running"
    );

    engine
        .start_metronome(Metronome::new(SAMPLE_RATE, &tempo, 2, MONO))
        .expect("the metronome starts on the virtual backend");

    assert!(
        engine.is_metronome_running(),
        "start_metronome flips the metronome flag on"
    );
    assert!(
        !engine.is_playing(),
        "the metronome is not take playback (is_playing stays false)"
    );
}

#[test]
fn ac6_metronome_clicks_at_expected_boundaries_via_virtual_backend() {
    // Drive the engine-hosted metronome over ~1.5 bars and confirm non-zero
    // energy in the onset window of each expected beat boundary, with the accent
    // at frame 0, all through the deterministic VirtualBackend (no device).
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let driver = backend.clone();
    let mut engine = Engine::new(backend);
    let tempo = tempo_120_4();

    let transport = Transport::new(SAMPLE_RATE, &tempo, 2);
    engine
        .start_metronome(Metronome::new(SAMPLE_RATE, &tempo, 2, MONO))
        .expect("metronome starts");

    // ~1.5 bars: a bar is boundary(8) frames; pull a bit past 12 boundaries.
    let frames = transport.boundary_frame(13) as usize;
    let out = driver.pull_output(frames);
    assert_eq!(
        out.len(),
        frames * MONO as usize,
        "pull_output yields frames * channels samples"
    );

    // Each beat boundary (every `subdivision`-th index) has energy in its onset
    // window; the silent mid-gap before the first boundary has none.
    let window = 256usize;
    for index in 0..12u64 {
        let boundary = transport.boundary_frame(index) as usize;
        if boundary + window > out.len() {
            break;
        }
        assert!(
            peak_abs(&out, boundary, window) > 0.0,
            "boundary {index} (frame {boundary}) carries a click"
        );
    }
    // The accent click sits at frame 0.
    assert!(
        peak_abs(&out, 0, window) > 0.0,
        "the metronome's accent click is present at frame 0"
    );
    // Genuine silence exists in the gap after the accent tick, before boundary 1.
    assert_eq!(
        peak_abs(&out, 6_000, 256),
        0.0,
        "the stream is silent between clicks, not a constant tone"
    );
}

#[test]
fn ac6_start_playback_stops_a_running_metronome() {
    // Metronome and take playback are mutually exclusive: starting playback
    // drops the metronome, and starting the metronome drops playback.
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let mut engine = Engine::new(backend);
    let tempo = tempo_120_4();

    engine
        .start_metronome(Metronome::new(SAMPLE_RATE, &tempo, 2, MONO))
        .expect("metronome starts");
    assert!(engine.is_metronome_running());

    let take = Take::new(vec![0.25; 256], SAMPLE_RATE, MONO);
    engine
        .start_playback(&take)
        .expect("playback starts and takes the single output");
    assert!(
        !engine.is_metronome_running(),
        "starting playback stops the running metronome"
    );
    assert!(engine.is_playing(), "playback is now running");

    // And the reverse direction: starting the metronome stops playback.
    engine
        .start_metronome(Metronome::new(SAMPLE_RATE, &tempo, 2, MONO))
        .expect("metronome restarts");
    assert!(
        engine.is_metronome_running(),
        "the metronome is running again"
    );
    assert!(
        !engine.is_playing(),
        "starting the metronome stops take playback"
    );
}

#[test]
fn ac6_stop_clears_both_metronome_and_playback() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let mut engine = Engine::new(backend);
    let tempo = tempo_120_4();

    engine
        .start_metronome(Metronome::new(SAMPLE_RATE, &tempo, 2, MONO))
        .expect("metronome starts");
    assert!(engine.is_metronome_running());

    engine.stop();
    assert!(!engine.is_metronome_running(), "stop() ends the metronome");
    assert!(!engine.is_playing(), "stop() leaves nothing playing");
}

// ---------------------------------------------------------------------------
// AC7 — Runnable demo: verified by ear on a real machine, NOT a CI test.
//
// CI has no audio device, so this suite never instantiates `CpalBackend` nor
// opens a real stream. The `cargo run -p gooz-audio --example metronome` demo
// plays the ratio-locked click through the default output for a few seconds;
// the owner verifies it by ear (R-0004 AC7, SPEC-0004 §"Demo"). There is
// intentionally no integration test here.
//
// The documented-public-API check and the four toolchain gates
// (build / test / clippy / fmt), including `no_run` doc examples on
// device-opening code, are verified at QA sign-off (loop step 7), not by an
// integration test in this file.
// ---------------------------------------------------------------------------
