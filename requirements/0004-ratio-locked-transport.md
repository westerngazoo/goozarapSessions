# R-0004 — Ratio-locked transport (metronome)

- **Status:** Accepted
- **Milestone:** M1
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-06-18
- **Depends on:** R-0002 (beat math / `Tempo`), R-0003 (audio engine)
- **Realized by:** SPEC-0004
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must have a ratio-locked transport: a sample-accurate clock that
turns `gooz-ratio`'s exact beat math into an audible metronome played through
`gooz-audio`'s engine. Given a `Tempo` (BPM + beats-per-bar) and the device
sample rate, it computes beat and subdivision boundaries whose sample positions
are derived directly from the beat ratios — so timing is exact and never drifts.
It plays a short synthesized tick at each boundary with three distinct voices —
an accent on the bar's downbeat, a normal click on other beats, and a quieter
click on the in-between subdivisions — and runs continuously until stopped.

This is the first feature that marries the two cores built so far: the math
(`gooz-ratio`) drives the sound (`gooz-audio`). It completes milestone M1.

## 2. Rationale

A musician — and this app's voice-driven Easy Mode — needs a steady reference
to play against: you hum or beatbox *to the click*. Because the click's timing
comes straight from the beat-ratio grid, everything recorded against it lands
on the same exact grid the quantizer (R-0006) will later snap to, with no
accumulated drift over a long take. Subdivisions let the user feel finer
structure (eighths, triplets) without any notation. It is also the proof that
the engine's real-time path can host a *generated* source, not just play back a
recorded take — the pattern every later instrument (R-0007) will follow.

## 3. Acceptance criteria

- **AC1 — Sample-accurate boundaries.** The k-th boundary is at frame
  `round(k · frames_per_subdivision)` where
  `frames_per_subdivision = sample_rate · seconds_per_beat / subdivision`
  (a frame = one sample per channel; on mono output a frame is a sample),
  computed from `k` absolutely (no cumulative error). `boundary(0) = 0`;
  boundaries are strictly increasing for sane tempi. Worked example: 48000 Hz,
  120 BPM, subdivision 2 → boundaries 0, 12000, 24000, 36000, ….
- **AC2 — Beat/bar classification & accent.** Boundary `k` is an **Accent** when
  it falls on a beat (`k mod subdivision == 0`) whose beat index is a bar
  downbeat (`beat_index mod beats_per_bar == 0`); a **Beat** when it is a beat
  but not a downbeat; a **Subdivision** otherwise. Worked example over one
  4/4 bar with subdivision 2: Accent, Sub, Beat, Sub, Beat, Sub, Beat, Sub.
- **AC3 — Built from `Tempo`.** `gooz-ratio`'s `Tempo` exposes `bpm()` and
  `beats_per_bar()`; the transport is constructed from a `Tempo` plus the sample
  rate and subdivision, so the rhythm core drives the engine.
- **AC4 — Distinct click voices.** Three preallocated enveloped-sine ticks with
  descending prominence — accent (highest pitch, loudest), beat (lower pitch,
  medium), subdivision (lower pitch, quietest) — so the tiers are measurably
  distinct (e.g. by peak amplitude) and audibly different.
- **AC5 — Real-time-safe, block-invariant render.** The metronome render fills
  an output block, placing the correct click at each boundary, accenting
  downbeats, writing each frame's click value to every channel, and continuing a
  click that spans a block boundary. It performs no allocation, locking, or I/O
  on the callback path, and never stalls or panics on a degenerate config.
  Rendering a span as one large block equals rendering it as many small
  (frame-aligned) blocks (sample-for-sample).
- **AC6 — Engine integration.** `start_metronome` opens a continuous output
  stream driven by the metronome; it and take playback are mutually exclusive;
  `stop` ends it. Proven over several bars via the deterministic `VirtualBackend`
  (clicks at the expected sample positions, downbeats accented) — no device in CI.
- **AC7 — Demo, errors, docs, gates.** A runnable metronome demo plays the click
  on a real machine (verified by ear; not a CI gate). Errors are the typed
  `AudioError`; library paths never panic. Public items are documented (device
  examples `no_run`); all four toolchain gates are green.

## 4. Constraints & non-goals

- Real-time discipline on the callback path is non-negotiable (`CLAUDE.md` §2).
- v0 is a fixed tempo for the life of a run; **tempo changes mid-stream** are out
  of scope.
- Clicking an arbitrary **Euclidean / step pattern** (`E(k,n)`) is the beat
  builder, **R-0009** — this requirement is a straight metronome (beats +
  uniform subdivision).
- **Swing / groove humanization**, count-in UI, and syncing arbitrary recorded
  takes to the running clock are later requirements.
- The click is a minimal built-in tick; richer instrument sound design is
  `gooz-synth` (R-0007).

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-18 | Click on each beat **and** a uniform subdivision (3 voices: accent/beat/sub) | Owner choice — lets the user feel finer structure (eighths/triplets) without notation, while staying a metronome (not the R-0009 pattern builder). |
| 2026-06-18 | Synthesized two-pitch enveloped-sine tick generated in `gooz-audio` | Owner choice — audible and pleasant; the architecture places the metronome in `gooz-audio`. Three amplitude tiers over two pitches keep it simple. |
| 2026-06-18 | Boundaries computed from `k` absolutely (`round(k·samples_per_sub)`) | Sample-accurate and drift-free over long runs; the ratio-locked guarantee. |
| 2026-06-18 | Add `bpm()` / `beats_per_bar()` accessors to `gooz-ratio`'s `Tempo` | The transport needs them; they are obviously-missing getters. Additive, behind the existing R-0002 suite as a regression gate. |
| 2026-06-18 | Metronome and take playback share the single device output (mutually exclusive) | v0 has one output; starting one stops the other. Mixing multiple sources is a later mixer concern. |

## Changelog

- 2026-06-18 — created, accepted for M1.
