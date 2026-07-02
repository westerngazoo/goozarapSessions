# R-0009 — Beat builder

- **Status:** Accepted
- **Milestone:** M2
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-06-28
- **Depends on:** R-0002 (Euclidean `E(k,n)`, `BarGrid`, `Tempo`), R-0007 (synth
  primitives: deterministic noise, distortion); R-0003 (record/play, demo only)
- **Realized by:** SPEC-0009
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must turn Euclidean rhythms into a **playable drum beat**: given a
tempo and, per drum voice, a Euclidean spec `E(k, n)` (with an optional rotation)
and a kit, it places exactly `k` hits across `n` steps of a bar (R-0002), renders
each voice with a **synthesized kit** (R-0007 primitives), and assembles a
**bar-aligned, loopable drum stem** that repeats cleanly on the downbeat —
analogous to R-0008's `RiffStem`. The builder returns the mixed stem **plus the
per-voice patterns** (each voice's realized hit map), so the app can show and
edit each voice's `k/n` "sparse ↔ busy" control.

The kit for v0 is three voices — **kick, snare, hat**. This is the rhythm half of
Easy Mode: where hum-to-riff (R-0008) makes the melodic part, the beat builder
makes the groove under it. It is not the UI (R-0013), is not biased by an
influence model (R-0018), and uses synthesis only (no sampler).

## 2. Rationale

R-0002 already generates `E(k, n)` patterns and bar grids; R-0007 already
synthesizes and shapes sound deterministically. Until they are wired into voices
on a grid, the app can make a riff but not a beat — half a song. Euclidean
rhythms are the project's "no music theory" answer to groove: one integer `k`
(how many hits) over another `n` (how many slots) is the entire interface, and it
covers most of the world's grooves (`E(3,8)` is the tresillo behind trap and
latin). Exposing `k/n` per voice is the "sparse ↔ busy" slider made real.
Returning the per-voice patterns (not just audio) is what lets the dark-neon UI
later draw each voice's steps and lets this stage be tested precisely — assert
the hits, not just "non-silent."

## 3. Acceptance criteria

- **AC1 — Euclidean placement.** For each voice with spec `E(k, n)` (optionally
  rotated), the builder fires exactly `k` hits across the bar's `n` steps, at the
  grid step positions given by `E(k, n)` from `gooz-ratio` (R-0002); `k = 0`
  yields a silent voice, `k = n` fires every step.
- **AC2 — Synthesized kit.** Three deterministic voices are synthesized from
  R-0007-style primitives: **kick** (sine + downward pitch envelope + amp
  envelope, 808-style), **snare** (filtered noise + a short tonal body), **hat**
  (a brief high-passed noise burst). No samples; no external assets.
- **AC3 — `k/n` (+ rotation) controls.** Each voice is controlled by `(k, n,
  rotation)`. Increasing `k` increases the hit count by exactly that amount
  (density is `k`); rotation shifts the pattern by whole steps **without** changing
  the hit count. These are the sparse↔busy / groove controls.
- **AC4 — Loopable (bar-aligned) stem.** The mixed stem's length is exactly a
  whole number of bars (`bars · bar_samples`, derived from the tempo and sample
  rate), `bars ≥ 1` for a non-empty beat, and it loops **seamlessly** on the
  downbeat (a hit's decay tail that crosses the loop boundary wraps to the start
  rather than being clipped). An all-silent voice set (`k = 0` everywhere) yields
  an empty stem with `bars == 0`.
- **AC5 — Returns what it built.** The result exposes the mixed stem **and** the
  per-voice realized patterns (each voice's `n` and the list of firing step
  indices, length `k`), so the UI can render and edit each voice's `k/n`.
- **AC6 — Deterministic.** The same input (voices + tempo + config + sample rate)
  produces an identical result — identical stem samples and identical per-voice
  patterns.
- **AC7 — Bounded, clean audio.** The stem samples are within `[-1, 1]` and
  contain no NaN/inf.
- **AC8 — Demo, docs, gates.** A `cargo run` demo plays a looping beat on a real
  machine (verified by ear; not a CI gate). Every public item is documented; the
  builder is covered by tests; all four toolchain gates are green.

## 4. Constraints & non-goals

- The builder is **offline, pure, and deterministic**: given the inputs it
  returns the stem with no device and no hidden state. It lives in
  `crates/gooz-synth` (drum synthesis is already that crate's responsibility) and
  takes a direct dependency on `gooz-ratio` for `Pattern`/`Tempo`. The demo adds
  `gooz-audio` to play the loop.
- Caller supplies the **tempo and the per-voice `(k, n, rotation)` + kit** — no
  auto-generation, no groove templates library, no tempo detection (later /
  influence-model features).
- **Three voices only** for v0 (kick, snare, hat); more voices (clap, open-hat,
  toms), a sampler, swing/humanize, and per-voice separate stems are later work.
- **No influence-model biasing** (R-0018), **no UI** (R-0013), **no session
  persistence** (R-0010).
- Synthesis is offline block rendering; the live-engine path is later.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-28 | v0 kit is **kick + snare + hat** (3 voices) | Owner choice — the minimal kit that reads as a real beat; a hat is what makes a pattern groove. More voices are additive later. |
| 2026-06-28 | Return the **mixed stem + per-voice patterns** (not per-voice audio stems) | Owner choice — feeds the UI's per-voice `k/n` display and makes the stage precisely testable; per-voice *audio* stems wait until arrangement/export (R-0011/R-0012) need them. |
| 2026-06-28 | Controls are **independent `E(k,n)` + rotation per voice** | The "sparse↔busy" slider made real; rotation gives groove/phase cheaply via `gooz-ratio`'s `Pattern::rotate`. |
| 2026-06-28 | Stem is **bar-aligned and loops by wrapping decay tails** to the start | A drum loop must be seamless; unlike a riff trailing into silence (R-0008 padded), a tail crossing the loop point belongs at the top of the next repeat. |
| 2026-06-28 | Lives in **`gooz-synth`** with a direct `gooz-ratio` dep; no new crate | Drum synthesis is already `gooz-synth`'s stated responsibility (ARCHITECTURE §3/§4.2); the only new edge is `gooz-ratio` for the pattern math. |

## Changelog

- 2026-06-28 — created, accepted for M2.
