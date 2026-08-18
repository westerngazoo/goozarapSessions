# R-0027 — Description-conditioned generation

- **Status:** Accepted
- **Milestone:** M7
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-04
- **Depends on:** R-0025 (`MusicalIntent`), R-0026 (`SoundPlan`), R-0009 (beat
  builder), R-0007 (instrument render), R-0001/R-0002 (ratio + rhythm core)
- **Realized by:** SPEC-0027
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must turn a [`SoundPlan`](0026-genre-presets.md) into **audio**: a
loopable **beat** (via the R-0009 builder) and a generated **melody** rendered as
an instrument (R-0007), both laid out on the plan's tempo, meter, and pitch grid.
Combined with R-0025 (parse) and R-0026 (plan), this closes the loop the
milestone promises: **a text description in, a playable song out**.

The melody is generated **ratio-natively**: its rhythm is a Euclidean pattern
sized by the plan's density, and its pitches walk the plan's harmonic grid, with
the plan's `tension` deciding how far into ratio complexity the walk may reach.
No note names, no model — the description biases the math (honesty rule).

## 2. Rationale

R-0025 understands the description and R-0026 makes it concrete, but neither
makes a sound. This requirement is the payoff: the owner types "corrido tumbado
en 6/8 a 135, 808 distorsionado, segundas menores" and hears it. It also proves
the seam works end to end before the language model (AC3 of R-0025) or the
influence model (R-0018) are involved at all — everything stays deterministic and
testable.

## 3. Acceptance criteria

- **AC1 — Plan → beat.** A `SoundPlan` maps to the beat builder's config
  (`VoiceRole → DrumKind`, `E(k, n)` + rotation + level preserved) and renders a
  non-empty, bar-aligned, bounded beat stem for `bars ≥ 1`.
- **AC2 — Plan → melody.** A generated melody exists: note events whose pitches
  come from the plan's harmonic grid (odd-limit) and whose onsets fall on the
  plan's beat grid; rendered through the instrument renderer with the plan's
  `drive`.
- **AC3 — The sliders are audible.** Higher `density` yields more melody notes;
  higher `tension` admits more complex ratios (the set of distinct degrees used
  never shrinks); `drive` reaches the render config.
- **AC4 — Description → song, end to end.** One call takes a **text description**
  and returns the plan plus the rendered beat and melody, so a prompt becomes
  audio in a single step.
- **AC5 — Deterministic.** The same description (or the same plan) always
  produces identical audio and identical notes.
- **AC6 — Bounded, clean, and playable.** All rendered audio is within `[-1, 1]`
  with no NaN/inf; a neutral (empty) description still produces a playable
  result, never an error.
- **AC7 — Tests, docs, gates.** Deviceless and fully unit-tested; every public
  item documented; all four toolchain gates green.

## 4. Constraints & non-goals

- Deterministic and **offline**: no model, no device, no randomness that is not
  seeded. The influence model biasing generation is **R-0018**, not this.
- Melody generation is **v0 and ratio-native** — a grid walk, not a composition
  engine. It is expected to be tuned by ear.
- Uses the engine as it exists today: the R-0007 instrument and the R-0009 kit.
  Where the plan asks for capability the engine lacks (a true 6/8 beat clock, an
  808 bass, reverb/EQ — R-0033/R-0034/R-0035), generation **degrades gracefully**
  rather than failing.
- **No UI** (that is R-0029), **no per-instrument prompts** (R-0032), no session
  persistence beyond what the studio already offers.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-04 | The melody's pitches **walk the grid ordered by ratio complexity**, with `tension` setting how deep the walk may reach | Ratio-first by construction: "tense" literally means "reaches more complex ratios", so the smooth↔tense language stays true from prompt to sound. |
| 2026-07-04 | The melody's rhythm is a **Euclidean pattern** sized by `density` | Same rhythmic language as the beat (R-0002/R-0009); one idea drives both, and density stays meaningful across the whole song. |
| 2026-07-04 | Generation lives in `apps/gooz-studio` | It is integration: the only layer that may depend on `gooz-model` (plan), `gooz-synth` (render), and `gooz-dsp` (notes) at once. Keeps lower crates free of app knowledge. |
| 2026-07-04 | Generation is **total** (no `Result` on the happy path) | `SoundPlan` is already validated (R-0026 AC5), so a plan can always be played; the honesty rule says an unrecognized description still sounds. |

## Changelog

- 2026-07-04 — created, accepted for M7.
