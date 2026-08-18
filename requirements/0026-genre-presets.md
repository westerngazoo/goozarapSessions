# R-0026 — Genre & style preset library

- **Status:** Accepted
- **Milestone:** M7
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-04
- **Depends on:** R-0025 (`MusicalIntent`), R-0002 (Euclidean `E(k,n)` + bar grids),
  R-0001 (harmonic grids), R-0009 (beat builder — the params' consumer)
- **Realized by:** SPEC-0026
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must turn a [`MusicalIntent`](0025-describe-intent.md) into a
**concrete, ratio-native plan** the engine can play: a tempo, a meter, a pitch
grid setting, and a per-voice rhythm spec (`E(k, n)` + rotation + level) for the
drum kit, plus the drive amount. The mapping is driven by a **genre preset
library** — data, not model weights — so "trap", "corrido/tumbado", "metal", and
a neutral "free" default each shape the plan differently, and the intent's
sliders (`tension`, `density`, `drive`) modulate the chosen preset.

This is the bridge between *what the description asked for* (R-0025) and *sound*
(R-0027 generation, R-0009 beat builder). Because presets are plain data and the
mapping is a pure function, the whole path is deterministic and unit-testable
with **no model in the loop**.

## 2. Rationale

R-0025 extracts intent but changes nothing musically. Something must decide that
"corrido tumbado in 6/8" means *a snare on beat 3, triplet-feel busy hats, a
sparse kick*, while "trap" means *half-time backbeat with rolling hats*. Encoding
that as a **reviewable data table** — rather than inside a model — keeps genre
knowledge inspectable, testable, and extendable by anyone (add a preset, no
retraining), and keeps generation deterministic. The intent's sliders then
modulate the preset so two "trap" prompts with different densities do not produce
identical beats.

## 3. Acceptance criteria

- **AC1 — Intent → plan.** A pure function maps a `MusicalIntent` to a plan
  carrying: tempo, meter, a pitch-grid setting (odd-limit derived from
  `tension`), per-voice rhythm specs (kick / snare / hat: `onsets k`, `steps n`,
  `rotate`, `level`), and `drive`. Deterministic: same intent ⇒ same plan.
- **AC2 — Genre presets are data.** A named preset table (at minimum: `trap`,
  `corrido`/`tumbado`, `metal`, and a neutral `free` fallback) selects the base
  rhythm shape. Selection uses the intent's `genre` tags; an unknown or empty
  genre falls back to `free` (never an error).
- **AC3 — Sliders modulate the preset.** Raising `density` raises the voices'
  onset counts `k`; raising `tension` raises the pitch-grid odd-limit; `drive`
  passes through to the render drive. Monotonic: more density ⇒ never fewer total
  onsets.
- **AC4 — Meter-aware rhythm.** The plan's per-voice `steps` follow the intent's
  meter — a compound meter (6/8) yields a step count divisible by its beat count,
  so the preset's accents (e.g. snare on beat 3) land on real beats.
- **AC5 — Always playable.** Every emitted plan is valid for the engine:
  `0 < k ≤ n`, `n > 0`, levels in `[0,1]`, tempo/meter within the engine's
  accepted range. A neutral (default) intent yields today's Easy Mode defaults —
  the honesty rule: no description ⇒ nothing surprising.
- **AC6 — North-star prompt.** The intent parsed from the owner's
  corrido-tumbado × black-metal prompt (135 BPM, 6/8, high tension/density/drive,
  genres {trap, tumbado, black metal}) yields a plan that is recognisably that:
  6/8 meter, 135 BPM, busy hats, a snare accent on beat 3, high odd-limit, high
  drive.
- **AC7 — Tests, docs, gates.** Pure and fully unit-tested (no model, no device);
  every public item documented; all four toolchain gates green.

## 4. Constraints & non-goals

- **Pure data + a pure function.** No ML, no randomness, no I/O. The influence
  model may *bias* preset choice later (R-0018) — not in this requirement.
- The plan is **engine-agnostic**: it is expressed in ratio/rhythm primitives
  (`k`, `n`, rotation, odd-limit, BPM, meter), **not** in synth or app types, so
  it cannot drag the app layer into a lower crate. The app adapts the plan to its
  own config types.
- **No generation** — rendering the plan into audio is R-0027. **No parsing** —
  that is R-0025. **No per-instrument prompts** (R-0032), **no 808/FX/6-8 engine
  work** (R-0033/R-0034/R-0035): where a preset asks for a capability the engine
  does not have yet, the plan still expresses it and the renderer degrades.
- Presets are a starting library, not a musicology claim; they are expected to be
  tuned by ear over time.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-04 | Presets are **data tables**, not model weights | Inspectable, testable, extendable without retraining; keeps generation deterministic (ARCHITECTURE §5 honesty rule). |
| 2026-07-04 | The output is an **engine-agnostic plan** in ratio primitives, adapted by the app | Keeps dependencies pointing inward: the plan lives with the intent (`gooz-model`), and the app maps it to its own beat config — a lower crate never learns app types. |
| 2026-07-04 | Genre selection **falls back to `free`** on unknown/empty tags | The honesty rule: a description we do not recognize still plays, from neutral defaults. |
| 2026-07-04 | Sliders **modulate** the chosen preset rather than replacing it | Two prompts of the same genre must differ; keeps the sparse↔busy / smooth↔tense language meaningful end to end. |
| 2026-07-04 | A preset may express a capability the engine lacks (e.g. 6/8 rhythm detail) | The plan is the musical intent made concrete; R-0033/34/35 raise the engine to meet it, and the renderer degrades until then. |

## Changelog

- 2026-07-04 — created, accepted for M7.
