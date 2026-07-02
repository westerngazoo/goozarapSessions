//! goozarapSessions Easy Mode desktop shell (Tauri) — R-0013 v0.
//!
//! A thin bridge: the commands call `gooz_studio`'s tested backend
//! ([`gooz_studio::demo_riff`], [`gooz_studio::riff_from_take`]) and use
//! `gooz_audio` for microphone capture. All music logic lives in the reviewed
//! crates; this crate only connects them to the webview. It is not part of the
//! workspace merge gate — build with `cargo tauri dev` (or `cargo run`) on a
//! desktop.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{sleep, JoinHandle};
use std::time::Duration;

use gooz_audio::{AudioBackend, CpalBackend, Engine};
use gooz_studio::{RiffView, demo_riff as demo_riff_view, riff_from_take};
use tauri::State;

/// Max seconds of microphone audio buffered per take.
const MAX_RECORD_SECS: usize = 12;

/// A microphone capture running on its own thread. cpal streams are `!Send`, so
/// the engine is created, driven, and dropped entirely on this thread; only the
/// stop flag and the join handle cross back to the command layer.
struct Capture {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Result<(Vec<f32>, u32), String>>,
}

/// Shared recorder state behind the two record commands.
#[derive(Default)]
struct Recorder(Mutex<Option<Capture>>);

/// Runs the built-in demo hum through the pipeline (no microphone).
#[tauri::command]
fn demo_riff() -> RiffView {
    demo_riff_view()
}

/// Begins capturing from the default input device. No-op if already recording.
#[tauri::command]
fn record_start(recorder: State<'_, Recorder>) -> Result<(), String> {
    let mut slot = recorder.0.lock().map_err(|_| "recorder lock poisoned")?;
    if slot.is_some() {
        return Ok(());
    }
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = stop.clone();
    let handle = std::thread::spawn(move || -> Result<(Vec<f32>, u32), String> {
        let backend = CpalBackend::with_defaults().map_err(|e| e.to_string())?;
        let sample_rate = backend.sample_rate();
        let mut engine = Engine::new(backend);
        engine
            .start_recording(sample_rate as usize * MAX_RECORD_SECS)
            .map_err(|e| e.to_string())?;
        while !stop_thread.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(20));
        }
        let take = engine.stop_recording();
        Ok((take.samples().to_vec(), take.sample_rate()))
    });
    *slot = Some(Capture { stop, handle });
    Ok(())
}

/// Stops capture, runs the Easy Mode pipeline on the take, and returns the riff.
#[tauri::command]
fn record_stop_analyze(recorder: State<'_, Recorder>) -> Result<RiffView, String> {
    let capture = recorder
        .0
        .lock()
        .map_err(|_| "recorder lock poisoned")?
        .take()
        .ok_or("not recording")?;
    capture.stop.store(true, Ordering::Relaxed);
    let (samples, sample_rate) = capture
        .handle
        .join()
        .map_err(|_| "recording thread panicked".to_string())??;
    riff_from_take(&samples, sample_rate).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .manage(Recorder::default())
        .invoke_handler(tauri::generate_handler![
            demo_riff,
            record_start,
            record_stop_analyze
        ])
        .run(tauri::generate_context!())
        .expect("error while running the goozarapSessions shell");
}
