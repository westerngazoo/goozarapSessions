# R-0003 — Audio engine v0 (record & playback)

- **Status:** Accepted
- **Milestone:** M1
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-06-15
- **Depends on:** none (uses its own crate; the ratio-locked clock is R-0004)
- **Realized by:** SPEC-0003
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must have a working real-time audio engine that can **record** sound
from an input device into a take and **play** a take back through an output
device. The engine logic must be decoupled from the physical device behind an
`AudioBackend` seam, so it runs identically on a real device (cpal) and on a
deterministic in-memory backend used for tests — meaning record/playback
correctness is provable in CI with no sound card. Anything on the audio
callback path is allocation-free and lock-free (preallocated lock-free ring
buffers only). A small runnable demo records a few seconds from the default
microphone and plays it back through the default speakers, so the capability
is tangible on a real machine.

This is the engine's first heartbeat: moving samples in and out. The
ratio-locked clock/metronome is R-0004; pitch/DSP is R-0005.

## 2. Rationale

Everything audible in the app flows through this engine: Easy Mode's
record-a-hum loop (M2), beat playback (M2), session mixdown (M3). It is the
moment the project stops being a pure-math library and starts making sound. It
must be built with real-time discipline from day one — the audio thread never
allocates, locks, or blocks — because every later feature adds nodes to this
path and inherits its guarantees. Decoupling the device behind a backend seam
is what makes a real-time system testable at all: the deterministic backend
lets CI prove the sample path without hardware.

## 3. Acceptance criteria

- **AC1 — Round-trip record/playback.** A known signal fed through the input
  path, captured into a `Take`, then played back through the output path,
  reproduces that signal sample-for-sample over the recorded region (via the
  deterministic backend, in CI). Silence/zeros are produced when nothing is
  loaded to play.
- **AC2 — Backend seam.** The engine is generic over an `AudioBackend`. Two
  backends implement it: a deterministic in-memory backend (no device) and a
  cpal backend (real device). Engine and ring-buffer logic are identical across
  both; the deterministic backend is driven synchronously so tests are
  repeatable.
- **AC3 — Real-time-safe callback path.** The capture and render operations
  performed on the audio thread only push to / pop from preallocated lock-free
  ring buffers — no heap allocation, no locking, no I/O, no unbounded work.
  Buffer overrun on capture drops samples (never blocks); buffer underrun on
  render emits silence (never blocks). This is guaranteed structurally and
  documented; a buffer-overrun and an underrun case are each exercised by a
  test.
- **AC4 — Take model.** A `Take` holds interleaved `f32` samples plus sample
  rate and channel count; it reports its frame count and duration in seconds;
  recording then reading back is lossless.
- **AC5 — Transport control.** Recording and playback can each be started and
  stopped; stopping a recording yields the captured `Take`; starting playback
  of a take plays it to completion (or until stopped); state transitions are
  well-defined and a second start does not corrupt state.
- **AC6 — Typed errors.** Device, stream, and unsupported-configuration
  failures surface as a typed `AudioError`; library paths never panic. An
  unsupported sample format is reported, not aborted on.
- **AC7 — Runnable demo.** A demo (`cargo run`) records ~4 seconds from the
  default input and plays it back through the default output on a real machine.
  It is verified by ear by the owner; it is **not** a CI gate (CI has no audio
  device).
- **AC8 — Documented public API & gates.** Every public item is documented;
  examples that require a real device are marked no-run; the project builds with
  all four toolchain gates green.

## 4. Constraints & non-goals

- Real-time discipline is non-negotiable on the callback path (`CLAUDE.md` §2,
  `project-specifics.md` domain notes).
- v0 handles `f32` samples at a single device sample rate; **resampling**,
  other sample formats, and sample-rate conversion are out of scope.
- The **ratio-locked clock, metronome, and click track** are **R-0004**.
- **Pitch/onset tracking and any DSP** are **R-0005** / `gooz-dsp`.
- Simultaneous live monitoring (hear yourself while recording), multi-track
  mixing, a node graph, file import/export, and latency compensation are later
  requirements.
- Mono is the v0 target; the engine carries the device's channel count but does
  no channel-format conversion beyond what the demo needs to be audible.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-15 | Build the full slice: device-agnostic engine (CI-tested via the in-memory backend) + real cpal backend + a runnable record→playback demo | Owner choice — makes the first audio milestone something you can actually hear, while keeping the sample path provable in CI. |
| 2026-06-15 | First external dependencies: `cpal` (device I/O) and `ringbuf` (lock-free SPSC) | Owner-approved. `cpal` is the de-facto Rust cross-platform audio choice (already in the architecture); `ringbuf` gives the lock-free ring the real-time rule requires. Both sit behind the backend seam so the engine core stays light. |
| 2026-06-15 | Record-then-play is sequential in v0 (not simultaneous duplex) | Keeps the transport simple and the round-trip test deterministic; live monitoring is a later requirement. |
| 2026-06-15 | Real-device behavior is verified by ear, not in CI | CI has no sound card; the in-memory backend proves the logic, the demo proves the device wiring. Matches the architecture's "virtual backend so CI needs no sound card." |

## Changelog

- 2026-06-15 — created, accepted for M1.
