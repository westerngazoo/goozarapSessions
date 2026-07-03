# R-0015 — Ingest & feature extraction

- **Status:** Accepted
- **Milestone:** M4
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-02
- **Depends on:** R-0014 (model registry), R-0005 (analysis), R-0001/R-0002 (ratio math)
- **Realized by:** SPEC-0015
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must **ingest reference audio and extract a compact, inspectable
feature profile** describing it — the raw material an influence model is trained
on. Given a reference recording (samples + sample rate) and a pitch grid, the
extractor produces a `FeatureProfile`: overall stats (duration, loudness,
brightness), a rhythm profile (estimated tempo, onset density), and a
**ratio/harmony histogram** (the reference's pitches snapped onto the grid,
weighted by how long they sound). The profile is written into the model's
directory (R-0014) as `features.json` and recorded in its manifest.

This is the **ingest half of M4**: it turns reference tracks into features. It is
**not** training (R-0016) and contains **no ML** — only deterministic DSP
(`gooz-dsp`) and ratio math (`gooz-ratio`).

## 2. Rationale

An influence model "digests influences" — but it can only train on *features*,
not raw audio. Before any training (R-0016) can happen, the app must reduce a
reference track to a small, meaningful, deterministic profile. Keeping the
profile **ratio-native** (a histogram over grid degrees, not raw Hz) matches the
project's math-first identity and makes it directly usable to bias the ratio
engine later (R-0018, M7). Keeping it **inspectable JSON** honours the "no black
box" stance and makes the whole stage testable without a trained model.

## 3. Acceptance criteria

- **AC1 — Extract a profile.** `extract_features(samples, sample_rate, grid,
  cfg)` returns a `FeatureProfile` for a valid reference recording.
- **AC2 — Overall stats.** The profile reports `duration_secs`, `rms`
  (loudness), and `brightness` (a zero-crossing-rate proxy), all finite.
- **AC3 — Rhythm profile.** The profile reports an estimated `tempo_bpm` (from
  inter-onset intervals; `0` when too few onsets) and an `onset_density` (onsets
  per second).
- **AC4 — Ratio histogram.** The profile lists the reference's pitches snapped to
  the grid as `(num, den, weight)` entries whose weights are non-negative and sum
  to `1.0` (± ε) when any pitch was found; empty when none.
- **AC5 — Persist to the model.** The registry can write a profile into a model's
  directory as `features.json` (recorded in the manifest) and read it back
  losslessly.
- **AC6 — Typed errors / determinism.** Empty / zero-rate / non-finite / too-short
  input returns a typed error (propagated from analysis); the same input always
  yields the same profile; no panics.
- **AC7 — Docs & gates.** Every public item is documented; covered by tests; all
  four toolchain gates are green.

## 4. Constraints & non-goals

- Deterministic DSP + ratio math only: depends on `gooz-dsp` + `gooz-ratio`.
  **No candle / ML** enters `gooz-model` in this requirement.
- **No training** (R-0016), no timbre *embeddings* from a neural net — the v0
  "timbre" features are simple signal statistics (rms, brightness). Learned
  embeddings arrive with training.
- Monophonic-oriented analysis (reuses R-0005); polyphonic separation is out of
  scope.
- Tempo is a rough **estimate** from onset spacing, not a beat-tracked grid.
- Reference audio is used locally only and is **not** stored in the profile
  (only derived features), consistent with the copyright stance
  (`docs/ARCHITECTURE.md` §8).

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | The harmony feature is a **ratio histogram over grid degrees**, weighted by note duration | Ratio-native (matches the engine), compact, and directly usable to bias generation later; duration-weighting reflects what actually sounds. |
| 2026-07-02 | v0 "timbre" = signal statistics (rms, zero-crossing brightness), not neural embeddings | Keeps R-0015 candle-free and deterministic; learned timbre embeddings belong with training (R-0016). |
| 2026-07-02 | The profile stores **only derived features**, never the reference samples | Copyright/privacy: reference audio stays local and is not redistributed in the session. |
| 2026-07-02 | Tempo is `60 / median(inter-onset interval)`, `0` with < 2 onsets | A simple, deterministic estimate; real beat-tracking is out of scope for v0. |

## Changelog

- 2026-07-02 — created, accepted for M4.
