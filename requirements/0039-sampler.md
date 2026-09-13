# R-0039 — Sampler: any recorded sound becomes an instrument

- **Status:** Accepted
- **Milestone:** M8
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-09-13
- **Depends on:** R-0038 (pitch shift) ✅, R-0001 (`Ratio`, grids), R-0006
  (`QuantizedNote`), R-0007 (the render layer this mirrors)
- **Realized by:** SPEC-0039
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

A **recorded buffer** — a hit on a table, a click, a voice, a door — must become
a **playable instrument**: given a set of quantized notes, the project renders
that recording across the ratio grid, one shifted copy per note, mixed into a
loopable buffer.

This is the owner's request in their own words: *"que grabe cualquier sonido y
pueda moverlo, casi casi generar un instrumento."*

## 2. Rationale

ARCHITECTURE §1.2 calls voice the universal instrument. This generalizes the
claim to its real form: **any sound** is the timbre, and the ratio grid is what
makes it an instrument. It is arguably the purest expression of the product
thesis — the user needs no theory, no sample library, and no synthesis
knowledge; they need a microphone and a ratio.

R-0038 exists only to unblock this.

## 3. Acceptance criteria

- **AC1 — The grid plays the sample.** Notes at grid degrees render as the
  recording shifted by those degrees: a sample rendered at `3:2` measures a
  fifth above the same sample rendered at `1:1`, verified with the YIN tracker.
- **AC2 — The recording is the root.** At degree `1:1`, octave 0, the sample is
  placed **unshifted** — the rendered segment is the source times a single gain
  factor, not a resampled approximation of it.
- **AC3 — A pitchless sound is still an instrument.** A knock, a click, or a
  noise burst renders across the whole grid without error and without anyone
  having to detect a pitch in it first.
- **AC4 — Octaves are ratio arithmetic.** A note's octave is applied as `2:1`
  stacked onto its degree; an octave up measures double, an octave down half.
  An octave so extreme the ratio cannot be formed is a typed error, not a panic.
- **AC5 — Total and bounded.** An empty recording, an empty note list, or a zero
  sample rate yields an empty buffer rather than an error or a panic. All output
  is finite and within `[-1, 1]`.
- **AC6 — Deterministic.** The same recording, notes, and config always produce
  an identical buffer.
- **AC7 — Tests, docs, gates.** Deviceless and fully tested, every public item
  documented, all four toolchain gates green.

## 4. Constraints & non-goals

- **Deviceless.** The sampler takes a buffer it is handed. Capture is R-0003 and
  already exists; wiring a microphone button is not this requirement.
- **No instrument picker.** Choosing *between* the Karplus string and a sampler
  is R-0031 and does not exist yet — see the decision log. This requirement adds
  the second renderer, not the switch between them.
- **No UI** (R-0029), and **no session persistence of the sample** — a song
  reopening with its instrument requires extending the session format (R-0010),
  which is its own requirement.
- Not a professional sampler: no velocity layers, no loop points, no
  multisampling, no formant correction, no time-stretch. One recording, one
  root, shifted.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-13 | **The recording's own pitch is the root.** The sampler shifts by the note's `degree` stacked with its `octave`, never by a frequency ratio computed against an absolute target | Keeps the whole path in exact `Ratio` arithmetic — `shift_pitch` takes a `Ratio` precisely so no float tuning factor can creep in. It also makes AC3 fall out for free: a knock has no detectable pitch, and under this rule it does not need one. Tuning a sample to an absolute pitch is a different feature, and it would be the one place a float shift was required. |
| 2026-09-13 | A note plays as a **one-shot**: the shifted copy rings out in full rather than being gated to `duration_secs` | Matches the let-ring behaviour `render_notes` already has (R-0007), and gating would put a click at the end of every note — the exact artifact QA's edge-hold test was added to prevent. Gating becomes a config option the day someone wants it. |
| 2026-09-13 | **No `Instrument` trait yet.** The sampler is a second renderer beside `render_notes`, not an abstraction over both | Issue #67 assumed an `Instrument` seam exists in `gooz-synth`; it does not — `render_notes` is a free function with Karplus-Strong hard-coded. Two implementations with no caller that switches between them do not justify a trait (CLAUDE.md §2, "three similar lines beat the wrong abstraction"). The seam earns itself in R-0031, where something actually has to choose. |
| 2026-09-13 | Lives in `crates/gooz-synth` | ARCHITECTURE §3 already lists a sampler as that crate's responsibility, and `gooz-synth` already depends on `gooz-dsp`, where `shift_pitch` lives. No new edge in the crate graph. |

## Changelog

- 2026-09-13 — created, accepted for M8.
