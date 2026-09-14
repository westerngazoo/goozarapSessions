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
- **AC2 — The recording is the root.** At degree `1:1` and the sampler's own
  root octave, the sample is placed **unshifted** — the rendered segment is the
  source times a single gain factor, not a resampled approximation. ("A single
  gain factor" is a claim about the bypass curve; the default render ends in a
  non-linear distortion and is not expected to hold it.)
- **AC2b — The instrument's register is the instrument's business.** A note's
  `octave` counts octaves above the *pitch grid's* root, which is a per-song
  setting. Re-rooting a song must not transpose a sampled part: the sampler
  carries its own root octave, and the shift is measured against that.
- **AC3 — A pitchless sound is still an instrument.** A knock, a click, or a
  noise burst renders across the whole grid without error and without anyone
  having to detect a pitch in it first.
- **AC4 — Octaves are ratio arithmetic.** A note's octave is applied as `2:1`
  stacked onto its degree; an octave up measures double, an octave down half.
  An octave so extreme the ratio cannot be formed yields a **typed**
  `RatioError::Overflow` — observed as a value, not merely as an absence of a
  panic — and that note is skipped while the rest of the part still renders.
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
| 2026-09-13 | The sampler carries a **`root_octave`**; the shift is `degree ⊗ 2^(octave − root_octave)` | Architect review, blocking. `QuantizedNote::octave` counts octaves above the *pitch grid's* root, and `Settings.root_hz` is a per-song, serialized, user-settable value. Without this field "the recording is the root" silently meant "the grid's root pitch **is** the recording's pitch" — measured, the same 440 Hz note plays unshifted against a 440 Hz root and **three octaves up, four times shorter** against a 55 Hz root. Re-rooting a song would have transposed every sampled part with no other change. Default 0 preserves the simple case; the caller states where its instrument lives. |
| 2026-09-13 | `Sampler` holds **audio**: samples outside `[-1, 1]` are refused too | QA run, AC5 FAIL. Two overlapping notes from a recording holding `2.0e38` sum past `f32::MAX`; `normalize_peak` then computes `1.0 / inf == 0.0`, and `inf · 0.0` is NaN — across the whole part. The invariant the renderer already assumed is now the one the type enforces. |
| 2026-09-13 | A buffer quieter than `1.0 / f32::MAX` is **not** normalized | QA run, AC5 FAIL. `gain = 1.0 / peak` stops being representable exactly there — measured, a peak of `2.938736e-39` normalized to `inf`, and `1.0373148` through the soft clip. Amplifying a buffer that quiet by 10^45 is noise, not audio. |
| 2026-09-13 | `Sampler::new` is **fallible**: a non-finite recording is refused | Architect review, blocking. One NaN anywhere makes every shift fail, so the whole part returned an empty buffer — not one silent note. Checking once, at construction, puts the error where the user can still record again, and gives the type an invariant. |
| 2026-09-13 | A skipped note is **silently** dropped, and that is a recorded decision rather than an accident | The audio path should stay total. But unlike `render_notes`, which only skips inputs a voiced note cannot produce, this renderer skips *legitimate, reachable* musical requests — so "why did my note not sound?" has no answer today. When R-0029's UI or R-0031's picker needs it, the answer is a skip report alongside the audio, not a `Result`. Named here so it is a choice, not an artifact of a `let-else`. |
| 2026-09-13 | A note plays as a **one-shot**: the shifted copy rings out in full rather than being gated to `duration_secs` | Matches the let-ring behaviour `render_notes` already has (R-0007), and gating would put a click at the end of every note — the exact artifact QA's edge-hold test was added to prevent. Gating becomes a config option the day someone wants it. |
| 2026-09-13 | **No `Instrument` trait yet.** The sampler is a second renderer beside `render_notes`, not an abstraction over both | Issue #67 assumed an `Instrument` seam exists in `gooz-synth`; it does not — `render_notes` is a free function with Karplus-Strong hard-coded. Two implementations with no caller that switches between them do not justify a trait (CLAUDE.md §2, "three similar lines beat the wrong abstraction"). The seam earns itself in R-0031, where something actually has to choose. |
| 2026-09-13 | Lives in `crates/gooz-synth` | ARCHITECTURE §3 already lists a sampler as that crate's responsibility, and `gooz-synth` already depends on `gooz-dsp`, where `shift_pitch` lives. No new edge in the crate graph. |

## Changelog

- 2026-09-13 — created, accepted for M8.
