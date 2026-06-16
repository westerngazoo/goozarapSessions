//! Acceptance tests for R-0003 — audio engine v0 (record & playback),
//! realized by SPEC-0003.
//!
//! TDD red: this file is written **before** the implementation exists. It is
//! the executable contract that the new `gooz-audio` items (`AudioError`,
//! `Take`, the `record_channel`/`playback_channel` SPSC rings, the
//! `AudioBackend` trait + `VirtualBackend` + `AudioStream`, and `Engine<B>`)
//! must satisfy. Until those types land it will not compile — that is the
//! intended red state for loop step 3.
//!
//! **No audio device in CI.** Every test here drives the deterministic,
//! synchronous `VirtualBackend` only; `CpalBackend` (the real device) is never
//! instantiated. Real-device behaviour is verified by ear via the demo
//! (AC7) — see the note below — not in CI.
//!
//! One section per acceptance criterion (AC1–AC8); every test fn is prefixed
//! with the criterion id it verifies.
//!
//! ## AC → test mapping
//!
//! | AC  | Verified by                                                          |
//! |-----|----------------------------------------------------------------------|
//! | AC1 | `ac1_round_trip_reproduces_signal_sample_for_sample`,               |
//! |     | `ac1_round_trip_take_carries_backend_format`,                       |
//! |     | `ac1_pull_output_before_any_playback_is_silence`,                   |
//! |     | `ac1_playback_of_empty_take_is_silence`                             |
//! | AC2 | `ac2_engine_is_generic_over_audio_backend`,                         |
//! |     | `ac2_backend_accessor_reflects_construction`,                       |
//! |     | `ac2_virtual_backend_drives_callbacks_synchronously`,               |
//! |     | `ac2_virtual_backend_is_cloneable_as_a_driver_handle`               |
//! | AC3 | `ac3_capture_overrun_drops_excess_without_panic`,                   |
//! |     | `ac3_record_sink_drains_exactly_capacity_on_overrun`,               |
//! |     | `ac3_render_underrun_zero_fills_the_tail`,                          |
//! |     | `ac3_render_into_empty_ring_is_all_silence`,                        |
//! |     | `ac3_load_render_round_trips_a_small_buffer`,                       |
//! |     | `ac3_capture_returns_count_stored_within_capacity`                  |
//! | AC4 | `ac4_frames_is_samples_over_channels_mono`,                         |
//! |     | `ac4_frames_is_samples_over_channels_stereo`,                       |
//! |     | `ac4_duration_secs_is_frames_over_sample_rate`,                     |
//! |     | `ac4_is_empty_reflects_sample_count`,                               |
//! |     | `ac4_samples_round_trip_is_lossless`,                               |
//! |     | `ac4_accessors_reflect_construction`                                |
//! | AC5 | `ac5_recording_state_transitions`,                                  |
//! |     | `ac5_playback_state_transitions`,                                   |
//! |     | `ac5_second_start_recording_is_a_noop_preserving_capture`,          |
//! |     | `ac5_start_playback_while_playing_replaces_cleanly`,                |
//! |     | `ac5_stop_recording_when_idle_yields_empty_stamped_take`,           |
//! |     | `ac5_stop_returns_to_idle`                                          |
//! | AC6 | `ac6_audio_error_is_a_typed_std_error`,                             |
//! |     | `ac6_audio_error_variants_are_comparable`                           |
//! | AC7 | verified by ear via the `record_playback` demo on a real machine;   |
//! |     | **not** a CI test — see the AC7 note (no device is opened here).     |
//! | AC8 | verified at QA sign-off (doc tests + clippy + fmt), not here         |
//!
//! ## API assumptions beyond SPEC-0003 §2/§3
//!
//! SPEC-0003 §2/§3 fix every constructor, accessor, trait method, and error
//! name these tests call; nothing was inferred. The notes below pin the exact
//! spelling/semantics the tests rely on so any mismatch is caught at the
//! implementation step, not blamed on a test typo:
//!
//! * `AudioError` is a fieldless enum (`NoInputDevice`, `NoOutputDevice`,
//!   `UnsupportedConfig`, `StreamBuild`, `StreamPlay`) deriving
//!   `Debug, Clone, Copy, PartialEq, Eq` and implementing `std::error::Error`
//!   + `Display` — verbatim spec §2 ("AudioError (error.rs)").
//! * `Take::new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Take`
//!   with `samples() -> &[f32]`, `sample_rate() -> u32`, `channels() -> u16`,
//!   `frames() -> usize`, `duration_secs() -> f64`, `is_empty() -> bool` —
//!   verbatim spec §2 ("Take (take.rs)"). `Take: Clone` is **not** assumed; the
//!   tests never clone a `Take`.
//! * Ring channels: `record_channel(capacity: usize) -> (Recorder, RecordSink)`
//!   and `playback_channel(capacity: usize) -> (PlaybackFeed, Player)`, with
//!   `Recorder::capture(&mut self, &[f32]) -> usize`,
//!   `RecordSink::drain(&mut self, &mut Vec<f32>)`,
//!   `PlaybackFeed::load(&mut self, &[f32]) -> usize`, and
//!   `Player::render(&mut self, &mut [f32])` — verbatim spec §2/§3 ("ring.rs").
//!   Per spec §2: a `record_channel(n)` holds exactly `n` samples (ringbuf 0.4
//!   `HeapRb::new(n)`, no reserved slot), so the overrun test asserts drops
//!   against `n`, not `n − 1`.
//! * `VirtualBackend::new(sample_rate: u32, channels: u16, block: usize)`,
//!   `#[derive(Clone)]`, with `feed_input(&self, &[f32])` and
//!   `pull_output(&self, frames: usize) -> Vec<f32>`, plus the `AudioBackend`
//!   trait methods `sample_rate() -> u32`, `channels() -> u16`,
//!   `open_input(Box<dyn FnMut(&[f32]) + Send>) -> Result<AudioStream, AudioError>`,
//!   `open_output(Box<dyn FnMut(&mut [f32]) + Send>) -> Result<AudioStream, AudioError>`
//!   — verbatim spec §2 ("backend.rs"). Per spec, `feed_input`/`pull_output`
//!   are no-ops when no callback is registered, and `pull_output` always
//!   returns a `Vec<f32>` of exactly `frames * channels` samples (zeros when no
//!   output stream is open).
//! * `Engine::new(backend) -> Engine<B>` with `backend() -> &B`,
//!   `is_recording() -> bool`, `is_playing() -> bool`,
//!   `start_recording(capacity_frames: usize) -> Result<(), AudioError>`,
//!   `stop_recording() -> Take`, `start_playback(&Take) -> Result<(), AudioError>`,
//!   and `stop()` — verbatim spec §2 ("engine.rs"). `pull_output` is called on
//!   a cloned driver handle held alongside the engine, per spec §2's `Clone`
//!   note and the AC1 walkthrough in §2 ("engine.rs").

use gooz_audio::{
    AudioBackend, AudioError, Engine, Take, VirtualBackend, playback_channel, record_channel,
};

/// The v0 device sample rate the round-trip and format tests pin against.
const SAMPLE_RATE: u32 = 48_000;
/// v0's mono target (R-0003 §4).
const MONO: u16 = 1;
/// A callback block size chosen so the test signals are **not** whole
/// multiples of it — this exercises `VirtualBackend`'s chunked feeding and the
/// ring's partial-`push_slice`/`pop_slice` paths.
const BLOCK: usize = 64;

/// A deterministic, easily-checked test signal: a short ramp whose length is
/// deliberately not a multiple of `BLOCK` (250 vs 64) so chunking is exercised.
fn ramp(len: usize) -> Vec<f32> {
    (0..len).map(|i| i as f32 * 0.001).collect()
}

// ---------------------------------------------------------------------------
// AC1 — Round-trip record/playback (deterministic backend, CI)
// ---------------------------------------------------------------------------

#[test]
fn ac1_round_trip_reproduces_signal_sample_for_sample() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    // A cloned handle drives input/output after the engine takes ownership.
    let driver = backend.clone();
    let mut engine = Engine::new(backend);

    let signal = ramp(250);

    engine
        .start_recording(signal.len())
        .expect("recording starts on the virtual backend");
    driver.feed_input(&signal);
    let take = engine.stop_recording();

    // Captured take reproduces the fed signal sample-for-sample. The virtual
    // backend is a lossless copy (no DSP, no resampling) so this is exact.
    assert_eq!(
        take.samples(),
        signal.as_slice(),
        "the captured take must equal the fed signal sample-for-sample"
    );

    engine
        .start_playback(&take)
        .expect("playback starts on the virtual backend");
    let played = driver.pull_output(signal.len());
    assert_eq!(
        played, signal,
        "playback must reproduce the take sample-for-sample over the recorded region"
    );
}

#[test]
fn ac1_round_trip_take_carries_backend_format() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let driver = backend.clone();
    let mut engine = Engine::new(backend);

    let signal = ramp(130);
    engine
        .start_recording(signal.len())
        .expect("recording starts");
    driver.feed_input(&signal);
    let take = engine.stop_recording();

    assert_eq!(take.sample_rate(), SAMPLE_RATE, "take is stamped 48000 Hz");
    assert_eq!(take.channels(), MONO, "take is stamped mono");
}

#[test]
fn ac1_pull_output_before_any_playback_is_silence() {
    // No output stream is open: pull_output returns all zeros (AC1 silence rule;
    // spec §2 "feed_input/pull_output are no-ops when no callback is registered").
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let frames = 200usize;
    let out = backend.pull_output(frames);
    assert_eq!(
        out.len(),
        frames * MONO as usize,
        "pull_output returns frames * channels samples"
    );
    assert!(
        out.iter().all(|&s| s == 0.0),
        "silence is produced when nothing is loaded to play"
    );
}

#[test]
fn ac1_playback_of_empty_take_is_silence() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let driver = backend.clone();
    let mut engine = Engine::new(backend);

    let empty = Take::new(Vec::new(), SAMPLE_RATE, MONO);
    engine
        .start_playback(&empty)
        .expect("playback of an empty take starts cleanly");

    let out = driver.pull_output(128);
    assert!(
        out.iter().all(|&s| s == 0.0),
        "playing an empty take yields silence"
    );
}

// ---------------------------------------------------------------------------
// AC2 — Backend seam (only the virtual backend is usable in CI)
// ---------------------------------------------------------------------------

/// The same engine code path is exercised through the `AudioBackend` seam via a
/// generic helper. This compiles only if `Engine<B>` is generic over
/// `B: AudioBackend` and `VirtualBackend: AudioBackend` — the seam itself.
fn round_trip<B: AudioBackend>(backend: B, driver: &VirtualBackend, signal: &[f32]) -> Vec<f32> {
    let mut engine = Engine::new(backend);
    engine
        .start_recording(signal.len())
        .expect("recording starts");
    driver.feed_input(signal);
    let take = engine.stop_recording();
    engine.start_playback(&take).expect("playback starts");
    driver.pull_output(signal.len())
}

#[test]
fn ac2_engine_is_generic_over_audio_backend() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let driver = backend.clone();
    let signal = ramp(100);
    let played = round_trip(backend, &driver, &signal);
    assert_eq!(
        played, signal,
        "the generic engine path round-trips over the virtual backend"
    );
}

#[test]
fn ac2_backend_accessor_reflects_construction() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let engine = Engine::new(backend);
    assert_eq!(
        engine.backend().sample_rate(),
        SAMPLE_RATE,
        "backend() exposes the constructed sample rate"
    );
    assert_eq!(
        engine.backend().channels(),
        MONO,
        "backend() exposes the constructed channel count"
    );
}

#[test]
fn ac2_virtual_backend_drives_callbacks_synchronously() {
    // The backend's own AudioBackend impl: open_input then feed_input must run
    // the registered capture callback synchronously (deterministic, repeatable).
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let (recorder, mut sink) = record_channel(64);
    let mut rec = recorder;
    let _stream = backend
        .open_input(Box::new(move |data: &[f32]| {
            rec.capture(data);
        }))
        .expect("virtual input opens");

    let signal = ramp(50);
    backend.feed_input(&signal);

    let mut drained = Vec::new();
    sink.drain(&mut drained);
    assert_eq!(
        drained, signal,
        "feed_input synchronously drove the capture callback"
    );
}

#[test]
fn ac2_virtual_backend_is_cloneable_as_a_driver_handle() {
    // Clone shares the same callback slots, so a clone can drive output that an
    // engine (holding the original) opened. Round-trip proves the shared state.
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let driver = backend.clone();
    let mut engine = Engine::new(backend);

    let signal = ramp(80);
    engine
        .start_recording(signal.len())
        .expect("recording starts");
    driver.feed_input(&signal);
    let take = engine.stop_recording();
    engine.start_playback(&take).expect("playback starts");

    assert_eq!(
        driver.pull_output(signal.len()),
        signal,
        "a cloned driver observes the engine-opened streams"
    );
}

// ---------------------------------------------------------------------------
// AC3 — Real-time-safe callback path (rings exercised directly)
// ---------------------------------------------------------------------------

#[test]
fn ac3_capture_overrun_drops_excess_without_panic() {
    // Capacity n holds exactly n samples (ringbuf 0.4, no reserved slot).
    let capacity = 4usize;
    let (mut recorder, _sink) = record_channel(capacity);
    let input = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
    let stored = recorder.capture(&input);
    assert_eq!(
        stored, capacity,
        "overrun stores exactly capacity samples and drops the excess"
    );
    assert!(
        stored < input.len(),
        "capture never blocks and never stores more than capacity"
    );
}

#[test]
fn ac3_record_sink_drains_exactly_capacity_on_overrun() {
    let capacity = 4usize;
    let (mut recorder, mut sink) = record_channel(capacity);
    let input = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0];
    recorder.capture(&input);

    let mut out = Vec::new();
    sink.drain(&mut out);
    assert_eq!(
        out,
        vec![1.0f32, 2.0, 3.0, 4.0],
        "the sink yields exactly the capacity samples that were stored, in order"
    );
}

#[test]
fn ac3_render_underrun_zero_fills_the_tail() {
    // Load fewer samples than the render block: the tail is zero-filled.
    let (mut feed, mut player) = playback_channel(8);
    let loaded = feed.load(&[1.0f32, 2.0, 3.0]);
    assert_eq!(loaded, 3, "load reports how many samples were queued");

    let mut output = [9.0f32; 6];
    player.render(&mut output);
    assert_eq!(
        output,
        [1.0f32, 2.0, 3.0, 0.0, 0.0, 0.0],
        "underrun emits silence for the unfilled tail (never blocks)"
    );
}

#[test]
fn ac3_render_into_empty_ring_is_all_silence() {
    // A render against a ring that was never loaded fills the block with zeros
    // (this is how AC1's "silence when nothing is loaded" is produced).
    let (_feed, mut player) = playback_channel(8);
    let mut output = [7.0f32; 5];
    player.render(&mut output);
    assert_eq!(
        output, [0.0f32; 5],
        "rendering an empty ring produces all silence"
    );
}

#[test]
fn ac3_load_render_round_trips_a_small_buffer() {
    let (mut feed, mut player) = playback_channel(8);
    let buffer = [0.25f32, -0.5, 0.75, -1.0];
    let loaded = feed.load(&buffer);
    assert_eq!(loaded, buffer.len(), "the whole buffer fits and is queued");

    let mut output = [0.0f32; 4];
    player.render(&mut output);
    assert_eq!(
        output, buffer,
        "load then render reproduces the buffer exactly"
    );
}

#[test]
fn ac3_capture_returns_count_stored_within_capacity() {
    // Within capacity, capture stores everything and reports the full length.
    let (mut recorder, mut sink) = record_channel(8);
    let input = [0.1f32, 0.2, 0.3];
    assert_eq!(
        recorder.capture(&input),
        input.len(),
        "within capacity, capture stores the whole input"
    );

    let mut out = Vec::new();
    sink.drain(&mut out);
    assert_eq!(out, input, "the captured samples drain back unchanged");
}

// ---------------------------------------------------------------------------
// AC4 — Take model
// ---------------------------------------------------------------------------

#[test]
fn ac4_frames_is_samples_over_channels_mono() {
    let take = Take::new(vec![0.0; 6], SAMPLE_RATE, MONO);
    assert_eq!(take.frames(), 6, "mono: frames == sample count");
}

#[test]
fn ac4_frames_is_samples_over_channels_stereo() {
    // 6 interleaved samples across 2 channels == 3 frames.
    let take = Take::new(vec![0.0; 6], SAMPLE_RATE, 2);
    assert_eq!(
        take.frames(),
        3,
        "stereo: frames == samples / channels (6 / 2)"
    );
}

#[test]
fn ac4_duration_secs_is_frames_over_sample_rate() {
    // 48000 mono samples @ 48000 Hz is exactly one second (clean value).
    let one_second = Take::new(vec![0.0; SAMPLE_RATE as usize], SAMPLE_RATE, MONO);
    assert_eq!(
        one_second.duration_secs(),
        1.0,
        "48000 samples @ 48000 Hz mono is exactly 1.0 s"
    );

    // 24000 mono samples @ 48000 Hz is exactly half a second.
    let half_second = Take::new(vec![0.0; 24_000], SAMPLE_RATE, MONO);
    assert_eq!(half_second.duration_secs(), 0.5);

    // 48000 interleaved stereo samples @ 48000 Hz is 24000 frames -> 0.5 s.
    let half_second_stereo = Take::new(vec![0.0; SAMPLE_RATE as usize], SAMPLE_RATE, 2);
    assert_eq!(
        half_second_stereo.duration_secs(),
        0.5,
        "duration uses frames, not raw sample count"
    );
}

#[test]
fn ac4_is_empty_reflects_sample_count() {
    let empty = Take::new(Vec::new(), SAMPLE_RATE, MONO);
    assert!(empty.is_empty(), "a take with no samples is empty");
    assert_eq!(empty.frames(), 0);
    assert_eq!(empty.duration_secs(), 0.0);

    let nonempty = Take::new(vec![0.5; 3], SAMPLE_RATE, MONO);
    assert!(!nonempty.is_empty(), "a take with samples is not empty");
}

#[test]
fn ac4_samples_round_trip_is_lossless() {
    let signal = vec![0.0f32, 0.25, -0.5, 0.999, -1.0, 0.123_456_79];
    let take = Take::new(signal.clone(), SAMPLE_RATE, MONO);
    assert_eq!(
        take.samples(),
        signal.as_slice(),
        "samples() returns the exact stored f32 values (lossless)"
    );
}

#[test]
fn ac4_accessors_reflect_construction() {
    let take = Take::new(vec![0.1, 0.2, 0.3, 0.4], 44_100, 2);
    assert_eq!(take.sample_rate(), 44_100);
    assert_eq!(take.channels(), 2);
    assert_eq!(take.frames(), 2);
}

// ---------------------------------------------------------------------------
// AC5 — Transport control
// ---------------------------------------------------------------------------

#[test]
fn ac5_recording_state_transitions() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let mut engine = Engine::new(backend);

    assert!(!engine.is_recording(), "a fresh engine is not recording");
    engine.start_recording(128).expect("recording starts");
    assert!(engine.is_recording(), "start_recording flips the flag on");

    let _take = engine.stop_recording();
    assert!(
        !engine.is_recording(),
        "stop_recording returns to the not-recording state"
    );
}

#[test]
fn ac5_playback_state_transitions() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let mut engine = Engine::new(backend);

    assert!(!engine.is_playing(), "a fresh engine is not playing");
    let take = Take::new(vec![0.5; 64], SAMPLE_RATE, MONO);
    engine.start_playback(&take).expect("playback starts");
    assert!(engine.is_playing(), "start_playback flips the flag on");

    engine.stop();
    assert!(!engine.is_playing(), "stop() ends playback");
}

#[test]
fn ac5_second_start_recording_is_a_noop_preserving_capture() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let driver = backend.clone();
    let mut engine = Engine::new(backend);

    let first = ramp(40);
    engine.start_recording(256).expect("recording starts");
    driver.feed_input(&first);

    // A second start while already recording is a no-op returning Ok(()): the
    // in-flight capture is preserved (the stream is NOT torn down and rebuilt).
    engine
        .start_recording(256)
        .expect("a redundant start_recording is a no-op returning Ok");
    assert!(
        engine.is_recording(),
        "still recording after the no-op start"
    );

    let second = ramp(40);
    driver.feed_input(&second);

    let take = engine.stop_recording();
    let mut expected = first.clone();
    expected.extend_from_slice(&second);
    assert_eq!(
        take.samples(),
        expected.as_slice(),
        "the no-op second start preserved the in-flight capture from before it"
    );
}

#[test]
fn ac5_start_playback_while_playing_replaces_cleanly() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let driver = backend.clone();
    let mut engine = Engine::new(backend);

    let first = Take::new(vec![0.1; 64], SAMPLE_RATE, MONO);
    engine
        .start_playback(&first)
        .expect("first playback starts");
    assert!(engine.is_playing());

    // Starting playback while already playing replaces the current playback:
    // the old output stream is dropped and a new one opened — no panic.
    let second_signal = ramp(96);
    let second = Take::new(second_signal.clone(), SAMPLE_RATE, MONO);
    engine
        .start_playback(&second)
        .expect("a replacing start_playback succeeds without panic");
    assert!(engine.is_playing(), "still playing after the replace");

    // The freshly-opened stream renders the second take from its start.
    let played = driver.pull_output(second_signal.len());
    assert_eq!(
        played, second_signal,
        "the replacing playback renders the new take from the beginning"
    );
}

#[test]
fn ac5_stop_recording_when_idle_yields_empty_stamped_take() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let mut engine = Engine::new(backend);

    // Not recording: stop_recording returns a well-formed empty take stamped
    // with the backend's sample rate and channels (never a zero-channel take).
    assert!(!engine.is_recording());
    let take = engine.stop_recording();
    assert!(
        take.is_empty(),
        "an idle stop_recording yields an empty take"
    );
    assert_eq!(take.samples(), &[] as &[f32]);
    assert_eq!(
        take.sample_rate(),
        SAMPLE_RATE,
        "the empty take is stamped with the backend sample rate"
    );
    assert_eq!(
        take.channels(),
        MONO,
        "the empty take is stamped with the backend channel count (>= 1)"
    );
    assert_eq!(take.frames(), 0);
}

#[test]
fn ac5_stop_returns_to_idle() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let mut engine = Engine::new(backend);

    let take = Take::new(vec![0.2; 32], SAMPLE_RATE, MONO);
    engine.start_playback(&take).expect("playback starts");
    engine.start_recording(64).expect("recording starts");

    engine.stop();
    assert!(!engine.is_playing(), "stop() clears playback");
    assert!(!engine.is_recording(), "stop() clears recording");
}

// ---------------------------------------------------------------------------
// AC6 — Typed errors
// ---------------------------------------------------------------------------

#[test]
fn ac6_audio_error_is_a_typed_std_error() {
    fn assert_std_error<E: std::error::Error>(_: &E) {}
    assert_std_error(&AudioError::NoInputDevice);

    // Every variant has a non-empty Display message (no panic on library paths;
    // an unsupported sample format is *reported*, not aborted on).
    for err in [
        AudioError::NoInputDevice,
        AudioError::NoOutputDevice,
        AudioError::UnsupportedConfig,
        AudioError::StreamBuild,
        AudioError::StreamPlay,
    ] {
        assert!(
            !err.to_string().is_empty(),
            "every AudioError variant has a non-empty Display message"
        );
    }
}

#[test]
fn ac6_audio_error_variants_are_comparable() {
    // PartialEq lets Results be asserted directly (used across the suite).
    assert_eq!(AudioError::NoInputDevice, AudioError::NoInputDevice);
    assert_ne!(AudioError::NoInputDevice, AudioError::NoOutputDevice);
    assert_ne!(AudioError::StreamBuild, AudioError::StreamPlay);
    assert_ne!(AudioError::UnsupportedConfig, AudioError::NoOutputDevice);
}

// ---------------------------------------------------------------------------
// AC7 — Runnable demo: verified by ear on a real machine, NOT a CI test.
//
// CI has no audio device, so this suite never instantiates `CpalBackend` nor
// opens a real stream. The `cargo run -p gooz-audio --example record_playback`
// demo records ~4 s from the default input and plays it back through the
// default output; the owner verifies it by ear (R-0003 AC7, SPEC-0003 §"Demo").
// There is intentionally no integration test here.
//
// AC8 — Documented public API & four toolchain gates (build / test / clippy /
// fmt) plus `no_run` doc examples for device-opening code: verified at QA
// sign-off (loop step 7), not by an integration test in this file.
// ---------------------------------------------------------------------------
