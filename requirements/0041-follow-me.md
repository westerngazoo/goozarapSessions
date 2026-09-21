# R-0041 — Follow me: a take sets the tempo and the key

- **Status:** Accepted
- **Milestone:** M8
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-09-21
- **Depends on:** R-0005 (pitch + onset analysis) ✅, R-0002 (beat grids) ✅,
  R-0001 (pitch grids) ✅
- **Realized by:** SPEC-0041
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

A recorded take must be able to **say how fast it is and where it sits**, so
that everything generated around it can match the person who made it.

Today Easy Mode is hard-wired to 92 BPM and a 220 Hz grid root. Whatever the
user sings is bent onto those two constants. This requirement inverts that: the
take is measured, and the constants become the *fallback* rather than the rule.

## 2. Rationale

The owner asked to sing and get accompaniment in a style. The style engine
(R-0025–R-0027) already exists and already makes beats and melodies. What makes
its output unusable as accompaniment is that it cannot hear the singer: a track
at 92 BPM in a key the singer is not in is not accompaniment, it is
interference.

This is the smallest change that turns two separate features into one product,
and **R-0042 cannot be built without it**.

## 3. Acceptance criteria

- **AC1 — Tempo from the take.** A take with a steady pulse reports a BPM within
  a small tolerance of that pulse.
- **AC2 — Key from the take.** A take with a steady pitch reports a root within
  a small tolerance of that pitch, in Hz.
- **AC3 — Silence about what was not heard.** A take with no usable pulse, or no
  usable pitch, reports **nothing** for that field rather than a guess. The two
  are independent: a knock has a tempo and no pitch; one long note has a pitch
  and no tempo. "No usable pulse" includes a take that *has* onsets but no
  grid — two notes far apart, or free time — not only a take with none.
- **AC4 — Tempo lands in a musical range.** A reported BPM is always within the
  plausible range, folded by octaves (doubled or halved) when the raw estimate
  is outside it. An estimate that cannot be folded into range reports nothing.
- **AC5 — Typed errors, no panics.** An empty take, a zero sample rate, or a
  non-finite take reports a typed `DspError`; nothing panics for any input.
- **AC6 — Deterministic.** The same take always reports the same thing.
- **AC7 — Easy Mode follows, coherently.** The studio's take path uses the take's
  tempo and root when they were heard, and its existing constants when they were
  not. What was followed is carried in the view, so the UI can say which. The
  beat under the riff and the settings written into a saved session follow the
  same numbers — a riff at 126 BPM must not be mixed against a 92 BPM loop or
  saved under a file that claims 92. The demo path is unchanged, and a golden
  test proves it.
- **AC8 — Tests, docs, gates.** Deviceless and fully tested; every public item
  documented; all four toolchain gates green.

## 4. Constraints & non-goals

- **The root is the take's central pitch, not a tonic.** Establishing a key
  centre properly means weighing which pitches are structurally important, which
  is a music-theory judgement this project does not make yet. For accompanying a
  singer, the pitch they are actually singing around is the right answer and the
  honest one. Harmony (R-0043) will need more, and that is its requirement.
- **No tempo *tracking*.** One number for the whole take, not a curve. A take
  that speeds up reports one tempo.
- **No meter detection.** Beats per bar stays 4 until R-0035 gives the engine a
  true non-4/4 clock.
- **No accompaniment here.** Singing over a style is R-0042; this requirement
  only measures the take.
- Deviceless and offline. No model.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-21 | The result reports **`Option`s**, not defaults | The analysis layer knows what it heard; it does not know what Easy Mode's constants are, and it should not. Returning `None` lets the caller apply its own fallback and — more importantly — lets the UI tell the truth about which of the two it actually followed instead of claiming both. |
| 2026-09-21 | Follow is **automatic on the take path, and the demo path is untouched** (owner decision) | The magic belongs where the user just sang. The demo is a fixed, deterministic showcase and existing sessions were made under the old constants; silently re-tuning either would be a change nobody asked for. |
| 2026-09-21 | `estimate_bpm` **moves** from `gooz-model` to `gooz-dsp` | It is rhythm math over onset times, not model code; it lives in `gooz-model` only because R-0015 needed it first. Leaving it there means writing a second estimator here, and two estimators drift. `gooz-model` already depends on `gooz-dsp`, so the move costs no new edge. |
| 2026-09-21 | The `Option`s reach the **view**, not just the library | Architect review. Without `followed_bpm`/`followed_root_hz` on `RiffView` the information dies one call after it is computed, the UI cannot tell the truth about what it followed, and the decision above buys nothing. |
| 2026-09-21 | Following is **not enough on its own**: the beat and the saved settings follow too | Architect review, major. Before this, a take heard at 126 BPM produced a riff at 126 mixed against a 92 BPM beat and saved under settings claiming 92. Following the take made the product *worse* on its main path until all three agreed. |
| 2026-09-21 | Refusing to answer needed **gates, not just an `Option`** | Architect review, blocking. Measured, an ungated estimator reported 103.2 BPM for two notes nine seconds apart and 117.8 for a free-time hum. An `Option` that is never `None` is decorative. |
| 2026-09-21 | Tempo is **folded by octaves** into a musical range | A median inter-onset interval routinely lands at half or double the felt pulse; folding is the standard fix and keeps the reported number musically meaningful rather than arithmetically correct and useless. |

## Changelog

- 2026-09-21 — created, accepted for M8.
- 2026-09-21 — architect review round 1: AC3 and AC7 sharpened after three blocking findings.
