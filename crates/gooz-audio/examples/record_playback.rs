//! R-0003 demo (AC7): record ~4 seconds from the default input and play it back
//! through the default output. Verified by ear on a real machine — not a CI
//! test. Run with `cargo run -p gooz-audio --example record_playback`.

use std::thread::sleep;
use std::time::Duration;

use gooz_audio::{AudioBackend, CpalBackend, Engine, Take};

const RECORD_SECS: u64 = 4;

fn main() {
    let backend = match CpalBackend::with_defaults() {
        Ok(backend) => backend,
        Err(err) => {
            eprintln!("audio device unavailable: {err}");
            return;
        }
    };

    let sample_rate = backend.sample_rate();
    let in_channels = backend.channels();
    let out_channels = backend.output_channels();
    println!(
        "recording {RECORD_SECS}s at {sample_rate} Hz ({in_channels}ch in / {out_channels}ch out) — make some noise..."
    );

    let mut engine = Engine::new(backend);
    let capacity_frames = sample_rate as usize * RECORD_SECS as usize;
    if let Err(err) = engine.start_recording(capacity_frames) {
        eprintln!("could not start recording: {err}");
        return;
    }
    sleep(Duration::from_secs(RECORD_SECS));

    let take = engine.stop_recording();
    println!("captured {:.1}s, playing back...", take.duration_secs());

    let playable = adapt_channels(&take, out_channels);
    if let Err(err) = engine.start_playback(&playable) {
        eprintln!("could not start playback: {err}");
        return;
    }
    sleep(Duration::from_secs_f64(playable.duration_secs() + 0.3));
    println!("done.");
}

/// Adapts a recorded take to the output channel count (non-real-time control
/// code): average each input frame to mono, then replicate across the output
/// channels. v0 assumes input and output share a sample rate (no resampling).
fn adapt_channels(take: &Take, out_channels: u16) -> Take {
    let in_channels = take.channels().max(1) as usize;
    let out_channels_usize = out_channels.max(1) as usize;
    if in_channels == out_channels_usize {
        return Take::new(take.samples().to_vec(), take.sample_rate(), out_channels);
    }
    let mut adapted = Vec::with_capacity(take.frames() * out_channels_usize);
    for frame in take.samples().chunks(in_channels) {
        let mono = frame.iter().sum::<f32>() / frame.len() as f32;
        adapted.extend(std::iter::repeat_n(mono, out_channels_usize));
    }
    Take::new(adapted, take.sample_rate(), out_channels)
}
