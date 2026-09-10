# R-0038 — Pitch shift (ratio-native)

- **Status:** Accepted
- **Milestone:** M2
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-09-10
- **Depends on:** R-0001 (`Ratio`), R-0005 (YIN — how the result is verified)
- **Realized by:** SPEC-0038
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must be able to **move a recorded sound to another pitch by a
frequency ratio**. Given a buffer and a `Ratio` (3:2, 5:4, 2:1 …), it returns
that buffer sounding at the shifted pitch. This is the DSP primitive the
architecture has always listed for `gooz-dsp` and never had, and it is the
**blocker for R-0039** — recording any sound and playing it across the grid as
an instrument.

It is ratio-native by design: the caller asks for "a fifth up" as `3:2`, never
as semitones, so the whole engine keeps speaking one language.

## 2. Rationale

"Voice as the universal instrument" only generalises to *any* sound once a
recording can be moved around the grid. A knock on a table is a timbre; shifting
it by the grid's ratios is what turns it into an instrument. Nothing else in the
engine can do this today: the synths generate pitch, they cannot *transpose a
recording*.

## 3. Acceptance criteria

- **AC1 — Ratio-native shift.** Shifting a signal by a `Ratio` produces audio
  whose measured fundamental is that ratio times the original, verified with the
  existing YIN tracker (R-0005) within a small cents tolerance.
- **AC2 — Identity.** Shifting by `1:1` returns the input unchanged (or within
  floating-point noise).
- **AC3 — Both directions.** Shifting up (`3:2`) and down (`2:3`) both work, and
  shifting by a ratio then by its inverse recovers the original pitch.
- **AC4 — Typed errors, no panics.** Empty input, a zero sample rate, or
  non-finite samples are reported as a typed `DspError`; nothing panics.
- **AC5 — Deterministic and clean.** The same input and ratio always produce
  identical samples; output is finite and bounded in `[-1, 1]`.
- **AC6 — Tests, docs, gates.** Golden-signal tests (a known sine, measured back
  with YIN); every public item documented; four gates green.

## 4. Constraints & non-goals

- Lives in `crates/gooz-dsp`, pure and offline. No device, no allocation
  concerns on an audio thread (this is not called from the callback).
- **Length is allowed to change** — see the decision log. Preserving length
  independently of pitch (time-stretch) is a **later** requirement.
- No formant correction, no polyphonic separation, no time-stretch.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-10 | The shift is expressed as a **`Ratio`**, never semitones or cents | The whole engine speaks ratios; a semitone API would be the one place a user has to know theory. |
| 2026-09-10 | v0 is **varispeed (resampling)**: pitch and length change together, like a classic sampler or tape | It is exactly right for the use that motivated this (R-0039, one-shot samples across a grid), it is artifact-free and deterministic, and it is honestly verifiable. A phase vocoder that holds length constant is real work and buys nothing for a one-shot; it becomes its own requirement the day a whole riff must be transposed without changing tempo. |
| 2026-09-10 | The API is a **seam**, so a length-preserving implementation can arrive later without touching callers | Same stance as the parser seam in R-0025: pick the honest simple thing now, leave the upgrade path open. |

## Changelog

- 2026-09-10 — created, accepted for M2.
