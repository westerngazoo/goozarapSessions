# SPEC-0003 — Audio engine v0 (record & playback)

- **Status:** Implemented — QA PASS, architect APPROVE (awaiting owner PR review)
- **Realizes:** R-0003
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-06-15
- **Depends on:** none (new crate `gooz-audio`; ratio-locked clock is R-0004)
- **Module(s):** `crates/gooz-audio`

## 1. Motivation

Realizes R-0003: the engine's first heartbeat — record sound into a `Take` and
play a `Take` back, with the device decoupled behind an `AudioBackend` seam so
the sample path is provable in CI and audible on a real machine via a demo.

## 2. Design

`gooz-audio` modules, std + two new deps (`cpal`, `ringbuf`):

```
crates/gooz-audio/src/
├── lib.rs        crate docs + re-exports
├── error.rs      AudioError — typed engine error
├── take.rs       Take — captured interleaved f32 samples + format
├── ring.rs       lock-free record/playback channels (the RT-safe sample path)
├── backend.rs    AudioBackend trait, AudioStream handle, VirtualBackend
├── cpal_backend.rs  CpalBackend — real device via cpal
└── engine.rs     Engine<B> — transport: start/stop record & playback
crates/gooz-audio/examples/
└── record_playback.rs  the runnable demo (real device, verified by ear)
```

Dependency direction stays inward: `gooz-audio` may use `gooz-ratio`/`gooz-dsp`
(already in its Cargo.toml) but v0 record/playback needs neither yet. `Cargo.toml`
pins `cpal = "0.15"` and `ringbuf = "0.4"`; in ringbuf 0.4 a `HeapRb::new(n)` holds
exactly `n` samples (no reserved slot), so a `record_channel(n)` has `n` samples of
headroom — the overrun test asserts drops against `n`, not `n − 1`.

### `AudioError` (error.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError {
    NoInputDevice,      // no default input device
    NoOutputDevice,     // no default output device
    UnsupportedConfig,  // device cannot provide an f32 stream we support
    StreamBuild,        // backend failed to build the stream
    StreamPlay,         // backend failed to start the stream
}
```

Fieldless (mirrors `RatioError`/`BeatError`); `Display` + `std::error::Error`;
cpal errors are mapped to these variants. A failed device default-config *query*
(distinct from device-absent) also maps to `UnsupportedConfig`. No panic on
library paths.

### `Take` (take.rs)

```rust
pub struct Take { samples: Vec<f32>, sample_rate: u32, channels: u16 }
```

- `new(samples, sample_rate, channels) -> Take`; accessors `samples() -> &[f32]`,
  `sample_rate() -> u32`, `channels() -> u16`.
- `frames() -> usize` = `samples.len() / channels` (the `channels == 0` guard
  returns 0 — pure defensiveness; a `Take` minted by the engine always carries
  the backend's `channels >= 1`, see `stop_recording`).
- `duration_secs() -> f64` = `frames() / sample_rate`.
- `is_empty() -> bool`. Pure, fully unit-testable; record→read-back is lossless.

### `ring.rs` — the real-time-safe sample path (AC3)

Two SPSC channels built on `ringbuf::HeapRb<f32>`, each split into an
audio-thread half and a control-thread half:

```rust
pub fn record_channel(capacity: usize) -> (Recorder, RecordSink);
pub fn playback_channel(capacity: usize) -> (PlaybackFeed, Player);

pub struct Recorder { /* HeapProd<f32> */ }     // audio thread
pub struct RecordSink { /* HeapCons<f32> */ }    // control thread
pub struct PlaybackFeed { /* HeapProd<f32> */ }  // control thread
pub struct Player { /* HeapCons<f32> */ }        // audio thread
```

- `Recorder::capture(&mut self, input: &[f32]) -> usize` — `push_slice`; returns
  how many were stored. **Overrun drops the rest (never blocks/allocates).**
- `Player::render(&mut self, output: &mut [f32])` — `pop_slice` into `output`,
  then zero-fill any remainder. **Underrun emits silence (never blocks/allocates).**
  A render against a ring that was never loaded fills the whole block with zeros
  — this is how "silence when nothing is loaded to play" (AC1) is produced.
- `RecordSink::drain(&mut self, out: &mut Vec<f32>)` — control-thread drain of
  all available samples (a stack scratch buffer; allocation here is fine, it is
  not the audio thread).
- `PlaybackFeed::load(&mut self, samples: &[f32]) -> usize` — control-thread
  `push_slice`; returns how many were queued.

`capture` and `render` are exactly the operations the audio callback runs: a
single `push_slice`/`pop_slice` over a preallocated ring plus a zero-fill —
no heap allocation, no locking, no I/O. This is the structural guarantee for AC3.

### `backend.rs` — the device seam (AC2)

```rust
pub trait AudioBackend {
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u16;
    fn open_input(&self, capture: Box<dyn FnMut(&[f32]) + Send + 'static>)
        -> Result<AudioStream, AudioError>;
    fn open_output(&self, render: Box<dyn FnMut(&mut [f32]) + Send + 'static>)
        -> Result<AudioStream, AudioError>;
}
```

- The callbacks are `Send + 'static` because a real backend moves them onto its
  audio thread.
- `AudioStream` is an opaque RAII handle that **stops the stream when dropped**.
  It keeps the underlying stream alive without requiring `Send` (cpal's `Stream`
  is `!Send` on some platforms), so it holds `Box<dyn Any>`:

  ```rust
  pub struct AudioStream { _keep: Box<dyn core::any::Any> }
  ```

- `VirtualBackend` (deterministic, in-memory, no device) implements the seam and
  is what CI uses:

  ```rust
  #[derive(Clone)]
  pub struct VirtualBackend { /* sample_rate, channels, block, shared cb slots */ }
  impl VirtualBackend {
      pub fn new(sample_rate: u32, channels: u16, block: usize) -> VirtualBackend;
      pub fn feed_input(&self, signal: &[f32]);          // drives the capture cb in `block`-sized chunks
      pub fn pull_output(&self, frames: usize) -> Vec<f32>; // drives the render cb, returns produced samples
  }
  ```

  `open_input`/`open_output` store the callback in a shared slot
  (`Arc<Mutex<Option<Box<…>>>>`); the `AudioStream` holds a clone of that `Arc`,
  and `feed_input`/`pull_output` invoke the stored callback synchronously. The
  `Mutex` is test-harness only — never on a real audio thread — so it does not
  violate the RT rule. `Clone` lets a test keep a driver handle after the engine
  takes ownership of the backend. Boundary semantics so tests are exact:
  `feed_input`/`pull_output` are **no-ops when no callback is registered** (so a
  `pull_output` before any output stream opens returns all zeros), and **dropping
  an `AudioStream` clears its slot** (modelling "stream stopped" — a subsequent
  `feed_input` does nothing).

### `cpal_backend.rs` — `CpalBackend` (AC2, AC6, AC7)

- `CpalBackend::with_defaults() -> Result<CpalBackend, AudioError>` — default
  host, default input/output devices (`NoInputDevice`/`NoOutputDevice` if
  absent), their default configs; requires `f32` sample format else
  `UnsupportedConfig`. Stores sample rate + channel count.
- `open_input`/`open_output` build a cpal stream whose data callback forwards to
  `capture`/`render` and whose **mandatory cpal error callback** maps stream
  errors to a logged/ignored no-op in v0 (never panics); map build/play failures
  to `StreamBuild`/`StreamPlay`, call `play()`, and return an `AudioStream`
  keeping the `cpal::Stream` alive.
- Because `AudioStream` holds a `!Send` `Box<dyn Any>` and `Engine<B>` holds
  `AudioStream`s, **`Engine<B>` is `!Send`** — an accepted v0 constraint
  (single-threaded control). M2+ will drive the engine from a UI thread via the
  SPSC command plane the architecture anticipates; out of scope here.
- Real-device behaviour is verified by ear (not a CI gate). Doc examples that
  open a device are marked `no_run`.

### `engine.rs` — `Engine<B>` transport (AC1, AC5)

```rust
pub struct Engine<B: AudioBackend> { /* backend + transport state */ }

impl<B: AudioBackend> Engine<B> {
    pub fn new(backend: B) -> Engine<B>;
    pub fn backend(&self) -> &B;
    pub fn is_recording(&self) -> bool;
    pub fn is_playing(&self) -> bool;
    pub fn start_recording(&mut self, capacity_frames: usize) -> Result<(), AudioError>;
    pub fn stop_recording(&mut self) -> Take;          // drains the sink into a Take(sample_rate, channels)
    pub fn start_playback(&mut self, take: &Take) -> Result<(), AudioError>;
    pub fn stop(&mut self);
}
```

- `start_recording`: build a `record_channel(capacity_frames * channels)`, move
  the `Recorder` into a capture closure, `open_input`, keep `(RecordSink,
  AudioStream)`. **A second `start_recording` while already recording is a no-op
  returning `Ok(())`** — the in-flight capture is preserved.
- `stop_recording`: drop the input stream (stops capture), drain the sink, and
  return a `Take` **always stamped with `backend.sample_rate()` /
  `backend.channels()`** (so `channels >= 1`). If not recording, returns a
  well-formed empty take carrying those same backend values.
- `start_playback`: build a `playback_channel(take.samples().len())`, `load` the
  take, move the `Player` into a render closure, `open_output`, keep the stream.
  **A `start_playback` while already playing replaces the current playback** —
  the old output stream is dropped (stopping it) and a new one opened.
- `stop`: drop any held streams; return to idle.
- Transport methods **propagate the backend's `AudioError` and introduce no new
  variants**; invalid transitions are handled by the no-op / replace policy
  above, never by error (so `AudioError` gains no "already recording" variant).

The round-trip test (AC1) drives a `VirtualBackend`: `start_recording` →
`feed_input(signal)` → `stop_recording()` yields a `Take` equal to `signal` →
`start_playback(&take)` → `pull_output(signal.len())` equals `signal`. A second
AC1 sub-case proves silence: `start_playback` of an **empty** take (or
`pull_output` before any playback opens) returns all zeros.

### Demo (`examples/record_playback.rs`, AC7)

`CpalBackend::with_defaults()` → `Engine` → record ~4 s → `stop_recording` →
adapt the take to the output channel count in a non-RT pass before playback:
if input channels ≠ output channels, **average the input frame's channels to
mono, then replicate that mono sample across the output channels** (the simplest
mapping that stays audible for the common 1↔2 channel case) → `start_playback` →
wait for the take's duration → exit. Run with
`cargo run -p gooz-audio --example record_playback`. This adaptation lives in
the demo, not the engine, keeping `capture`/`render` a pure `push_slice`/`pop_slice`.

## 3. Code outline

```rust
// ring.rs
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};

pub fn record_channel(capacity: usize) -> (Recorder, RecordSink) {
    let (prod, cons) = HeapRb::<f32>::new(capacity.max(1)).split();
    (Recorder { prod }, RecordSink { cons })
}

impl Recorder {
    pub fn capture(&mut self, input: &[f32]) -> usize { self.prod.push_slice(input) }
}
impl Player {
    pub fn render(&mut self, output: &mut [f32]) {
        let filled = self.cons.pop_slice(output);
        output[filled..].fill(0.0);
    }
}

// engine.rs (sketch of start_recording)
let (recorder, sink) = record_channel(capacity_frames * self.backend.channels() as usize);
let mut rec = recorder;
let stream = self.backend.open_input(Box::new(move |data: &[f32]| { rec.capture(data); }))?;
self.record = Some((sink, stream));
```

## 4. Non-goals

- Ratio-locked clock / metronome — R-0004.
- Pitch/onset tracking, any DSP — R-0005 / `gooz-dsp`.
- Live monitoring (hear yourself while recording), multi-track mixing, a node
  graph, file import/export, latency compensation — later requirements.
- Resampling, non-`f32` formats, sample-rate conversion — out of scope for v0.

## 5. Open questions

None — settled in the decision log.

## 6. Acceptance criteria

Maps to R-0003 AC1–AC8; qa owns `tests/acceptance_r0003.rs` (uses `VirtualBackend`
only — no device in CI).

- [x] AC1 — round-trip record→take→playback reproduces a known signal via the
      virtual backend; a second sub-case proves all-zero output when nothing is
      loaded.
- [x] AC2 — engine is generic over `AudioBackend`; virtual + cpal both implement
      it; identical engine/ring logic.
- [x] AC3 — capture/render only push/pop preallocated rings; overrun drops,
      underrun zero-fills; both exercised by a test.
- [x] AC4 — `Take` frames/duration correct; lossless record→read-back.
- [x] AC5 — start/stop record & playback; stop_recording yields the take; state
      transitions well-defined.
- [x] AC6 — typed `AudioError`; no panic on library paths.
- [x] AC7 — runnable demo records & plays back on a real machine (verified by
      ear; not a CI gate).
- [x] AC8 — public items documented (device examples `no_run`); four gates green.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-15 | `AudioStream` holds `Box<dyn Any>` (not `Send`) | cpal's `Stream` is `!Send` on some platforms; the handle stays on its owning thread, while the *callbacks* are `Send` so the backend can move them to the audio thread. |
| 2026-06-15 | Backend seam takes `Box<dyn FnMut>` callbacks (capture/render) | Keeps the trait object-safe (so `dyn AudioBackend` / generic `Engine<B>` both work) and matches how cpal drives data callbacks. |
| 2026-06-15 | `VirtualBackend` drives callbacks synchronously, `Mutex` slots, `Clone` | Deterministic, repeatable CI tests; the `Mutex` is test-harness only, never on an audio thread, so the RT rule is preserved. |
| 2026-06-15 | Sequential record-then-play; load whole take into the playback ring | Simplest correct v0; ring sized to the take. Streaming/duplex is a later requirement. |
| 2026-06-15 | Architect review (REQUEST CHANGES) applied: AC1 silence-when-empty given explicit render behaviour + a test sub-case; `stop_recording` always stamps the take with backend rate/channels (no zero-channel take); second-start policy made singular (record = no-op, playback = replace); transport propagates backend errors with no new variants; cpal mandatory error callback + `Engine<B>: !Send` noted; demo channel adaptation pinned (avg-to-mono then replicate); `VirtualBackend` boundary semantics (no-op without a callback, drop clears slot); config-query failure → `UnsupportedConfig` | Findings 1–9 of the SPEC-0003 review; findings 1–3 were the substantive ones (AC1 gap, empty-take format, ambiguous second-start). |

## Changelog

- 2026-06-15 — created; accepted alongside R-0003.
