# R-0012 — Mixdown & export

- **Status:** Accepted
- **Milestone:** M3
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-02
- **Depends on:** R-0010 (session format), R-0011 (arrangement)
- **Realized by:** SPEC-0012
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

A song must **render to audio and export to WAV**. Mixdown reads the arrangement
(R-0011) and sums the placed, non-muted stems — each at its bar offset, scaled by
its level, looped to fill the song — into a single master buffer. Export writes
that master to a **WAV file**, and also writes **each stem to its own WAV**. This
is the step that turns a saved session into something you can share or drop into
another DAW.

## 2. Rationale

Everything upstream (stems, arrangement) is internal state; a song only becomes a
*deliverable* when it renders to a standard audio file. Mixdown is also the first
consumer that actually *interprets* the arrangement — placements, mute, level —
so it validates the model end to end. Per-stem export gives the user (or a later
influence-model ingest) the isolated parts, not just the mix.

## 3. Acceptance criteria

- **AC1 — Mixdown.** `Song::mixdown()` returns a master (`sample_rate` +
  `samples`) that sums every non-muted placement at its `start_bar` offset,
  scaled by its `level`, looping each stem across the arrangement's length.
- **AC2 — Mute & level honoured.** A muted placement contributes nothing; a
  placement at level `l` contributes its samples scaled by `l`.
- **AC3 — WAV master export.** `Song::export_master(path)` writes a mono WAV of
  the mixdown at the master sample rate that reads back with the expected frame
  count and rate.
- **AC4 — Per-stem export.** `Song::export_stems(dir)` writes one WAV per stem
  and returns the written paths.
- **AC5 — Bounded output.** Master samples stay within `[-1, 1]` (a peak limiter
  applies only if the sum would clip); no NaN/inf.
- **AC6 — Typed errors, no panic.** An invalid arrangement, mismatched stem
  sample rates, or an unwritable path returns a typed `SessionError`; an empty
  song (no placements) yields an empty master, not an error.
- **AC7 — Deterministic.** The same song mixes to identical samples.
- **AC8 — Docs & gates.** Every public item is documented; covered by tests; all
  four toolchain gates are green.

## 4. Constraints & non-goals

- Depends on `gooz-session` model + `hound` for WAV; **no** engine/synth/DSP
  dependency (mixing is sample summing, not effects).
- **Mono, linear-gain mixing only.** No pan, no per-section automation, no FX,
  no resampling — all placed stems must share the master sample rate (else a
  typed error).
- Master WAV is **16-bit PCM** (the compatible default); higher bit depths /
  float WAV are later.
- Loudness normalization beyond a clip-safety limiter is out of scope.
- Not the UI export button (R-0013) — this is the library capability it calls.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | Placed stems **loop** from `start_bar` to the arrangement end | Stems are bar-aligned loops; filling the song is the musically-expected default and keeps the master the arrangement's length. |
| 2026-07-02 | Require all placed stems to **share the master sample rate** | v0 has no resampler; mixing mismatched rates would detune — better a typed error than silent corruption. |
| 2026-07-02 | Master is **16-bit PCM WAV** via `hound` | The most universally readable format; float/24-bit are a later refinement. |
| 2026-07-02 | Clip-safety: normalize only if the summed peak exceeds 1.0 | Prevents hard clipping without altering already-safe mixes. |

## Changelog

- 2026-07-02 — created, accepted for M3.
