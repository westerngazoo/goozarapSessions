//! R-0008 demo (AC7): hum → distorted guitar riff, end to end.
//!
//! Records ~4 s from the default input, runs the [`hum_to_riff`] pipeline, and
//! loops the rendered riff through the default output. Verified by ear on a real
//! machine — not a CI test. Run with `cargo run -p gooz-studio`.

use std::thread::sleep;
use std::time::Duration;

use gooz_audio::{AudioBackend, CpalBackend, Engine, Take};
use gooz_dsp::{PitchGrid, Tempo};
use gooz_studio::{PipelineConfig, hum_to_riff};

const RECORD_SECS: u64 = 4;
const LOOPS: usize = 4;

fn main() {
    let backend = match CpalBackend::with_defaults() {
        Ok(backend) => backend,
        Err(err) => {
            eprintln!("audio device unavailable: {err}");
            return;
        }
    };
    let sample_rate = backend.sample_rate();
    let out_channels = backend.output_channels();

    println!("hum a melody for {RECORD_SECS}s — make some noise...");
    let mut engine = Engine::new(backend);
    if let Err(err) = engine.start_recording(sample_rate as usize * RECORD_SECS as usize) {
        eprintln!("could not start recording: {err}");
        return;
    }
    sleep(Duration::from_secs(RECORD_SECS));
    let take = engine.stop_recording();

    let grid = PitchGrid::harmonic(220.0, 9).expect("220 Hz harmonic grid is valid");
    let tempo = Tempo::new(92.0, 4.0).expect("92 BPM / 4 beats-per-bar is valid");
    let outcome = match hum_to_riff(
        take.samples(),
        take.sample_rate(),
        &grid,
        &tempo,
        &PipelineConfig::default(),
    ) {
        Ok(outcome) => outcome,
        Err(err) => {
            eprintln!("pipeline failed: {err}");
            return;
        }
    };

    println!(
        "heard {} notes; riff is {} bar(s); looping {LOOPS}x...",
        outcome.notes.len(),
        outcome.stem.bars
    );
    if outcome.stem.samples.is_empty() {
        eprintln!("no riff — no notes detected in the recording");
        return;
    }

    let looped = loop_and_adapt(&outcome.stem.samples, out_channels, LOOPS);
    let riff = Take::new(looped, outcome.stem.sample_rate, out_channels);
    if let Err(err) = engine.start_playback(&riff) {
        eprintln!("could not start playback: {err}");
        return;
    }
    sleep(Duration::from_secs_f64(riff.duration_secs() + 0.3));
    println!("done.");
}

/// Repeats the mono stem `loops` times and spreads each sample across every
/// output channel (the riff is mono; the device output may be stereo).
fn loop_and_adapt(mono: &[f32], out_channels: u16, loops: usize) -> Vec<f32> {
    let channels = out_channels.max(1) as usize;
    let mut out = Vec::with_capacity(mono.len() * loops * channels);
    for _ in 0..loops {
        for &sample in mono {
            for _ in 0..channels {
                out.push(sample);
            }
        }
    }
    out
}
