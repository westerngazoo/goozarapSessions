//! R-0009 demo (AC8): Euclidean beat → looped playback.
//!
//! Builds the default trap-flavoured template and plays it through the default
//! output. Verified by ear on a real machine — not a CI test. Run with
//! `cargo run -p gooz-studio --bin beat`.

use std::thread::sleep;
use std::time::Duration;

use gooz_audio::{AudioBackend, CpalBackend, Engine, Take};
use gooz_dsp::Tempo;
use gooz_studio::{BeatConfig, build_beat};

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

    let tempo = Tempo::new(92.0, 4.0).expect("92 BPM / 4 beats-per-bar is valid");
    let cfg = BeatConfig::default();
    let stem = match build_beat(&tempo, sample_rate, &cfg) {
        Ok(stem) => stem,
        Err(err) => {
            eprintln!("beat build failed: {err}");
            return;
        }
    };

    if stem.samples.is_empty() {
        eprintln!("no beat rendered");
        return;
    }

    println!(
        "playing {} bar(s) × {LOOPS} loops (kick E(4,16), snare E(2,16)+4, hat E(7,16))...",
        stem.bars
    );

    let looped = loop_and_adapt(&stem.samples, out_channels, LOOPS);
    let beat = Take::new(looped, stem.sample_rate, out_channels);
    let mut engine = Engine::new(backend);
    if let Err(err) = engine.start_playback(&beat) {
        eprintln!("could not start playback: {err}");
        return;
    }
    sleep(Duration::from_secs_f64(beat.duration_secs() + 0.3));
    println!("done.");
}

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
