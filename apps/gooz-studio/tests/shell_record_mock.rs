//! Mock-device integration test for the studio shell's record→riff path
//! (R-0013 v0).
//!
//! Uses gooz-audio's deviceless [`VirtualBackend`] as a stand-in microphone, so
//! the flow the Tauri `record_stop_analyze` command wraps — capture a take, then
//! [`riff_from_take`] — is exercised in CI without any hardware.

use std::f64::consts::{PI, TAU};

use gooz_audio::{Engine, VirtualBackend};
use gooz_studio::riff_from_take;

const SAMPLE_RATE: u32 = 48_000;
const MONO: u16 = 1;
const BLOCK: usize = 512;

/// A short hum: two grid tones (220 Hz then 330 Hz) separated by a gap, each
/// faded in and out — enough for the analyzer to find two notes.
fn hum() -> Vec<f32> {
    let sr = f64::from(SAMPLE_RATE);
    let tone_len = (0.35 * sr) as usize;
    let gap_len = (0.12 * sr) as usize;
    let mut out = Vec::new();
    for freq in [220.0_f64, 330.0] {
        for n in 0..tone_len {
            let t = n as f64 / sr;
            let fade = (PI * n as f64 / tone_len as f64).sin();
            out.push((0.6 * fade * (TAU * freq * t).sin()) as f32);
        }
        out.resize(out.len() + gap_len, 0.0);
    }
    out
}

#[test]
fn records_a_hum_through_the_mock_device_and_makes_a_riff() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let driver = backend.clone(); // drives capture after the engine takes ownership
    let mut engine = Engine::new(backend);

    let signal = hum();
    engine
        .start_recording(signal.len())
        .expect("recording starts on the virtual backend");
    driver.feed_input(&signal);
    let take = engine.stop_recording();

    assert_eq!(
        take.samples().len(),
        signal.len(),
        "the mock device captures the hum losslessly"
    );

    let view = riff_from_take(take.samples(), take.sample_rate(), 30)
        .expect("a non-empty hum yields a riff");
    assert!(view.bars >= 1, "a non-empty riff is at least one bar");
    assert!(!view.notes.is_empty(), "the hummed tones are heard");
    assert!(
        view.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0),
        "riff audio stays bounded in [-1, 1]"
    );
}

#[test]
fn silence_through_the_mock_device_yields_no_riff() {
    let backend = VirtualBackend::new(SAMPLE_RATE, MONO, BLOCK);
    let driver = backend.clone();
    let mut engine = Engine::new(backend);

    let silence = vec![0.0_f32; SAMPLE_RATE as usize]; // 1 s of nothing
    engine
        .start_recording(silence.len())
        .expect("recording starts");
    driver.feed_input(&silence);
    let take = engine.stop_recording();

    let view = riff_from_take(take.samples(), take.sample_rate(), 30)
        .expect("silence is a valid, finite signal");
    assert_eq!(view.bars, 0, "no onsets → no riff → zero bars");
    assert!(view.samples.is_empty(), "an empty riff has no samples");
    assert!(view.notes.is_empty());
}
