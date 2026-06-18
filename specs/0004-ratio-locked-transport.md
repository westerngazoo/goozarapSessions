# SPEC-0004 — Ratio-locked transport (metronome)

- **Status:** Accepted
- **Realizes:** R-0004
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-06-18
- **Depends on:** SPEC-0002 (`Tempo`), SPEC-0003 (engine, backend seam)
- **Module(s):** `crates/gooz-audio` (+ two accessors on `crates/gooz-ratio`)

## 1. Motivation

Realizes R-0004: a sample-accurate, ratio-locked metronome — `gooz-ratio`'s
beat math driving `gooz-audio`'s engine. Completes M1 and establishes the
pattern for hosting a *generated* source on the real-time path.

## 2. Design

`gooz-audio` gains two modules and one engine method; `gooz-ratio`'s `Tempo`
gains two getters. `gooz-audio`'s `Cargo.toml` adds `gooz-ratio` (the first
inward edge — the marriage of the two cores).

```
crates/gooz-ratio/src/beat.rs   + Tempo::bpm() / Tempo::beats_per_bar()
crates/gooz-audio/Cargo.toml     + gooz-ratio.workspace = true  (first inward edge)
crates/gooz-audio/src/
├── transport.rs   Transport (sample-accurate beat/subdivision clock) + ClickKind
├── metronome.rs   Metronome (RT-safe render source) + tick synthesis
├── engine.rs      + Engine::start_metronome / is_metronome_running
└── lib.rs         re-export Transport, ClickKind, Metronome
crates/gooz-audio/examples/metronome.rs   the by-ear demo
```

The clock works in **frames** (a frame = one sample per channel); on mono
output a frame is one sample, so the AC1 worked example is unchanged. The
metronome render is channel-aware: each frame's click value is written to every
channel of that frame.

### `Tempo` accessors (`gooz-ratio`)

```rust
impl Tempo {
    pub fn bpm(&self) -> f64 { self.bpm }
    pub fn beats_per_bar(&self) -> f64 { self.beats_per_bar }
}
```

Additive getters; the R-0002 acceptance suite is the regression gate.

### `ClickKind` + `Transport` (`transport.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickKind { Accent, Beat, Subdivision }

pub struct Transport {
    frames_per_sub: f64,
    subdivision: u32,
    beats_per_bar: u32,
}
```

- `Transport::new(sample_rate: u32, tempo: &Tempo, subdivision: u32) -> Transport`:
  `subdivision = subdivision.max(1)`;
  `frames_per_sub = sample_rate as f64 * tempo.seconds_per_beat() / subdivision as f64`;
  `beats_per_bar = (tempo.beats_per_bar().round() as i64).max(1) as u32`.
- `boundary_frame(&self, index: u64) -> u64` = `(index as f64 * frames_per_sub).round() as u64`
  — computed from `index` absolutely, so no cumulative drift (AC1). On mono
  output a frame is a sample, matching the AC1 worked example.
- `click_kind(&self, index: u64) -> ClickKind` (AC2):
  - if `index % subdivision != 0` → `Subdivision`;
  - else let `beat = index / subdivision`; if `beat % beats_per_bar == 0` →
    `Accent` else `Beat`.
- Accessors `subdivision()`, `beats_per_bar()`, `frames_per_sub()` for tests.

Pure and fully unit-testable against the ratio math; no audio, no allocation
concern.

### `Metronome` (`metronome.rs`) — the RT-safe render source

```rust
pub struct Metronome {
    transport: Transport,
    channels: u16,
    accent: Vec<f32>,
    beat: Vec<f32>,
    sub: Vec<f32>,
    pos: u64,                       // absolute output frame counter
    next_index: u64,                // next boundary's index
    active: Option<(ClickKind, usize)>, // currently-sounding click + cursor
}
```

- `Metronome::new(sample_rate: u32, tempo: &Tempo, subdivision: u32, channels: u16)
  -> Metronome` builds the `Transport`, stores `channels.max(1)`, and
  pre-synthesizes the three ticks (no allocation thereafter). Ticks are ~30 ms
  enveloped sines (AC4):
  - accent: 1000 Hz, peak 0.9;
  - beat: 800 Hz, peak 0.6;
  - subdivision: 800 Hz, peak 0.3;
  built by a private `tick(sample_rate, freq, amp, secs)` = `amp · cos(2π f t) ·
  (1 − i/n)` (linear decay) for `i in 0..n`, `n = round(sample_rate · secs).max(1)`.
  The `.max(1)` keeps the tick non-empty so the render's `buf[cursor]` is always
  in bounds (no panic). Cosine (not sine) gives a sharp attack — sample 0 is the
  peak `amp`, not a zero crossing — so even a degenerate config that retriggers
  the click every frame still emits sound rather than silence.
- `render(&mut self, output: &mut [f32])` (AC5), RT-safe — iterates `output` one
  **frame** at a time (`output.chunks_mut(channels)`), at absolute frame `pos`:
  - **fire every boundary at or before this frame** —
    `while boundary_frame(next_index) <= pos { active = Some((click_kind(next_index), 0)); next_index += 1; }`.
    This `<=`/while form (not `==`) cannot stall: even if a degenerate config
    collapses several boundaries onto one frame (`frames_per_sub < 1`), they all
    fire (last wins) and `next_index` always advances. Monophonic — a new click
    restarts the single voice; clicks are short and beats spaced, so at sane
    tempi this retrigger truncation is inaudible (and is intended behaviour, not
    a bug);
  - compute this frame's click value from the active click (or `0.0`), advance
    its cursor, clear `active` at the click's end, and **write that value to
    every channel of the frame**;
  - `pos += 1` per frame.
  Only reads preallocated `Vec`s and writes `output` — no allocation, locking,
  or I/O. Because `pos`/`next_index`/`active` persist across calls, rendering a
  span as one block equals rendering it as many small (frame-aligned) blocks
  (block-invariant).

### `Engine::start_metronome` (`engine.rs`)

```rust
pub fn start_metronome(&mut self, metronome: Metronome) -> Result<(), AudioError>;
pub fn is_metronome_running(&self) -> bool;
```

- Adds a `metronome: Option<AudioStream>` field beside `playback`.
- `start_metronome`: drop any take playback **and** any running metronome first
  (`self.playback = None; self.metronome = None;`), then `open_output` with a
  render closure that calls `metronome.render(out)`, store the stream. Metronome
  and take playback are mutually exclusive (one device output).
- `start_playback` likewise drops the metronome first.
- `stop()` clears both; `is_playing()` stays take-only; add
  `is_metronome_running()`.

### Demo (`examples/metronome.rs`, AC7)

`CpalBackend::with_defaults()` → `Tempo::new(120.0, 4.0)` → `Metronome::new(sr,
&tempo, 2, backend.channels())` (channel-aware, correct on a stereo device) →
`Engine::start_metronome` → run a few seconds → `stop`. Run with
`cargo run -p gooz-audio --example metronome`.

## 3. Code outline

```rust
// metronome.rs (render core) — channel-aware, while-advance boundary firing
pub fn render(&mut self, output: &mut [f32]) {
    for frame in output.chunks_mut(self.channels as usize) {
        while self.transport.boundary_frame(self.next_index) <= self.pos {
            self.active = Some((self.transport.click_kind(self.next_index), 0));
            self.next_index += 1;
        }
        let value = match self.active {
            Some((kind, cursor)) => {
                let buf = self.voice(kind);
                let sample = buf[cursor];
                let next = cursor + 1;
                self.active = if next < buf.len() { Some((kind, next)) } else { None };
                sample
            }
            None => 0.0,
        };
        for out in frame.iter_mut() {
            *out = value;
        }
        self.pos += 1;
    }
}
```

## 4. Non-goals

- Euclidean / arbitrary step-pattern clicks — R-0009.
- Mid-stream tempo changes, swing/humanization, count-in UI.
- Syncing arbitrary recorded takes to the running clock, multi-source mixing.
- Richer click sound design — `gooz-synth` (R-0007).

## 5. Open questions

None — settled in the decision log.

## 6. Acceptance criteria

Maps to R-0004 AC1–AC7; qa owns `tests/acceptance_r0004.rs` (VirtualBackend only).

- [ ] AC1 — `boundary_frame` exact, absolute, strictly increasing for sane tempi
      (48k/120/2 → 0, 12000, 24000, …).
- [ ] AC2 — `click_kind` accent/beat/subdivision classification over a bar.
- [ ] AC3 — `Tempo::bpm()`/`beats_per_bar()` exist; transport built from a `Tempo`.
- [ ] AC4 — three click voices with descending peak amplitude (0.9 / 0.6 / 0.3).
- [ ] AC5 — render places correct clicks, accents downbeats, writes every
      channel of each frame; one-block == many-small-blocks; while-advance firing
      never stalls; ticks non-empty so no panic; no allocation on the path.
- [ ] AC6 — `start_metronome` continuous output; mutually exclusive with
      playback; ticks at expected positions via `VirtualBackend`.
- [ ] AC7 — demo runs (by ear); typed errors, no panic; docs; four gates green.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-18 | Monophonic click (a boundary retriggers the single voice) | Clicks are ~30 ms and beats are spaced; overlap is inaudible, and one voice keeps the render trivially RT-safe. |
| 2026-06-18 | `Transport` is pure (sample math only); `Metronome` owns the audio | Keeps the clock unit-testable without audio and isolates the RT render. |
| 2026-06-18 | Metronome reuses the engine's single output (own slot, exclusive with playback) | v0 has one device output; a mixer comes later. |
| 2026-06-18 | Architect review (REQUEST CHANGES) applied: while-advance boundary firing (`<= pos`, not `==`) so a degenerate `frames_per_sub < 1` can't stall the clock; tick length `.max(1)` so `buf[cursor]` never panics; channel-aware frame render (clock in frames, click written to every channel) so the metronome is correct on a stereo device; explicit `gooz-ratio` Cargo edge; retrigger truncation noted as intended | Findings 1–5 of the SPEC-0004 review; findings 1–2 were the substantive RT-path correctness gaps (silent stall, panic). |

## Changelog

- 2026-06-18 — created; accepted alongside R-0004.
