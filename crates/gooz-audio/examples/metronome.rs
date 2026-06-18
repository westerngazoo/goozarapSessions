//! R-0004 demo (AC7): play the ratio-locked metronome through the default
//! output for a few seconds. Verified by ear on a real machine — not a CI test.
//! Run with `cargo run -p gooz-audio --example metronome`.

use std::thread::sleep;
use std::time::Duration;

use gooz_audio::{AudioBackend, CpalBackend, Engine, Metronome};
use gooz_ratio::Tempo;

const RUN_SECS: u64 = 6;
const BPM: f64 = 120.0;
const BEATS_PER_BAR: f64 = 4.0;
const SUBDIVISION: u32 = 2; // eighth-note clicks

fn main() {
    let backend = match CpalBackend::with_defaults() {
        Ok(backend) => backend,
        Err(err) => {
            eprintln!("audio device unavailable: {err}");
            return;
        }
    };

    let tempo = match Tempo::new(BPM, BEATS_PER_BAR) {
        Ok(tempo) => tempo,
        Err(err) => {
            eprintln!("invalid tempo: {err}");
            return;
        }
    };

    let sample_rate = backend.sample_rate();
    let channels = backend.channels();
    println!(
        "metronome at {BPM} BPM, {BEATS_PER_BAR} beats/bar, 1/{SUBDIVISION}-beat clicks for {RUN_SECS}s..."
    );

    let metronome = Metronome::new(sample_rate, &tempo, SUBDIVISION, channels);
    let mut engine = Engine::new(backend);
    if let Err(err) = engine.start_metronome(metronome) {
        eprintln!("could not start metronome: {err}");
        return;
    }
    sleep(Duration::from_secs(RUN_SECS));
    engine.stop();
    println!("done.");
}
