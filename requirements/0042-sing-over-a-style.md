# R-0042 — Sing over a style: your take, with a track in a style, at your tempo

- **Status:** Accepted
- **Milestone:** M8
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-09-26
- **Depends on:** R-0041 (follow me), R-0026 (genre presets), R-0009 (beat
  builder), R-0012 (mixdown & export)
- **Realized by:** SPEC-0042
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The owner must be able to **sing, pick a style, and hear themselves over a track
in that style — at the tempo they sang, starting where they started**.

This is the owner's request in their own words: *"si comienzo a cantar, ¿me
puedes crear pistas de acompañamiento en un estilo de música?"* R-0041 made the
take say how fast it is and where it sits; the style engine (R-0025–R-0027)
already turns a style into drum patterns. This requirement joins them.

## 2. Rationale

Before R-0041 the two halves could not be joined honestly: a styled track at
92 BPM under a singer at 120 is not accompaniment. Now they can, and this is the
point at which the product does the thing the owner asked for rather than two
things next to each other.

## 3. Acceptance criteria

- **AC1 — The track is at the singer's tempo.** Given a take with a pulse and a
  style, the rendered track's bar length matches the take's tempo, not the
  style's default and not a tempo written in the style text.
- **AC2 — The singer enters on a downbeat.** The take's first sung note lands
  on the downbeat of bar 2, after **exactly one** bar of drums — a count-in —
  however long the singer waited after tapping. Matching the tempo but not the
  phase puts the singer up to a beat off the drums, which sounds wrong at any
  BPM.
- **AC3 — The style is audible.** Different styles produce different tracks from
  the same take: the drum patterns are the chosen preset's, not a generic beat.
- **AC4 — A take with no pulse still gets a track.** When the take says nothing
  about tempo (a held note, free time), the style's own tempo is used — each
  style has one — and the result says so rather than claiming to have
  followed.
- **AC5 — The key is followed and kept.** The take's root is carried in the
  result and in a saved session's settings, so parts that *have* pitch (bass,
  harmony — later requirements) can use it. Drums have no pitch; the result
  must not pretend the key changed the drums.
- **AC6 — One mix, and each part on its own.** The result can be played as a
  mix, and the voice and the track are separate stems — saveable, exportable,
  and individually mutable through the existing session and mixdown paths.
- **AC7 — Typed errors, no panics, deterministic, bounded.** Bad input is a
  typed `DspError`; the same take and style always produce the same audio; all
  output is finite and within `[-1, 1]`.
- **AC8 — Reachable from the app.** The studio offers the styles that exist as
  one-tap choices beside the record button, and the result arrives through the
  existing play / save / export paths.
- **AC9 — Tests, docs, gates.** Deviceless and fully tested; every public item
  documented; all four toolchain gates green.

## 4. Constraints & non-goals

- **After, not live.** You record, then you hear yourself over the track. A
  track that reacts *while* you sing needs a real-time tempo tracker on the
  audio thread — a different problem, and its own milestone.
- **Drums only, today.** The engine's styles are drum patterns; there is no bass
  (R-0033, #55) and no harmony (R-0043) yet. The key is followed and recorded
  (AC5) precisely so those can use it the day they land. This requirement does
  not add a generated melody over the singer — see the decision log.
- **Your first note is beat 1.** v0 assumes the singer starts on the downbeat. A
  pickup note (an anacrusis) will be aligned as though it were the downbeat.
- **Styles are the four that exist**: corrido/tumbado, trap, metal, and a
  neutral free style. Free-text style prompts are R-0029's describe UI.
- One tempo for the whole take, 4/4 (R-0035 gives the engine other meters).

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-26 | **The voice wins on tempo.** The style's tempo is used only when the take has no pulse | It is the whole point of R-0041. Someone who sings at 120 and asks for "trap a 140" wants a trap track they can sing over, and a track they cannot keep up with is not that. |
| 2026-09-27 | **The first sung note enters on a downbeat after a bar of drums — the voice is delayed, not trimmed** (owner decision) | Supersedes the 2026-09-26 trim. `StemPlacement` places stems by whole bars, so the alignment has to be in the audio; delaying it is lossless, needs no guessed pre-roll, and gives a count-in. The earlier row claimed the untrimmed take was kept as a session `Take`; nothing in the product writes one, and with nothing cut there is nothing to keep. |
| 2026-09-27 | **Exactly one bar of drums before the singer** (owner decision) | Whole bars of lead-in before the first note's bar are dropped: a 10 s wait had become five bars of drums over room noise on every loop. |
| 2026-09-27 | **Each style has its own tempo** (owner decision) | Every style chip gave 92 BPM — Easy Mode's default wearing the style's name. |
| 2026-09-27 | **Tap to start, tap to stop**, up to 30 s (owner decision) | A fixed 3.5 s capture made every accompaniment a one- or two-bar loop. |
| 2026-09-26 | **Your voice plus the styled drums — no generated melody over you** | A generated line would compete with the singer for the same space; accompaniment sits *under* a voice. What makes a track feel like it is in your key is bass and harmony, which are the next requirements, not a melody. |
| 2026-09-26 | **Styles as one-tap chips**, one per preset that exists | Zero typing, and it does not pretend to understand styles the engine has no pattern for. A text box that accepts "bossa nova" and quietly plays the neutral preset would be a small lie in the UI. |
| 2026-09-26 | **Record, then accompany** — not live | A live track needs tempo tracking on the real-time path, which has its own constraints (CLAUDE.md, real-time audio discipline). Getting the offline version right first also answers what the live version should sound like. |

## Changelog

- 2026-09-26 — created, accepted for M8.
- 2026-09-27 — architect design review round 1 and three owner decisions: count-in, per-style tempos, tap to stop.
- 2026-09-27 — QA round 1 (FAIL on AC2) and a fourth owner decision: exactly one bar in.
