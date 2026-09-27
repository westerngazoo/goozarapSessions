# R-0040 — Play my sound: a recording becomes an instrument you can hear

- **Status:** Accepted
- **Milestone:** M8
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-09-13
- **Depends on:** R-0039 (sampler) ✅, R-0003 (capture) ✅, R-0013 (studio shell) ✅,
  R-0001 (grids) ✅
- **Realized by:** SPEC-0040
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The owner must be able to **record any sound and hear it played across the ratio
grid**, in the app, without typing anything.

R-0039 built the instrument; nothing can reach it. This requirement is the wire:
a recorded take becomes a `Sampler`, the grid's degrees become a rising figure
played through it, and the result arrives in the studio shell as an ordinary
riff — the same waveform, the same play/save/export, the same note cards.

## 2. Rationale

The owner's ask was *"que grabe cualquier sonido y pueda moverlo, casi casi
generar un instrumento"* — **grabe** and **pueda**. Both are about doing it, not
about the engine being capable of it. Everything the engine needs has been
merged; what is missing is one function, one command, and one button.

It also makes two existing controls honest for the first time. The smooth↔tense
slider picks the odd-limit, which decides **which degrees exist**, so moving it
audibly changes the scale the sound is played over. And the "esto escuché" cards
already display ratios — here they show the scale itself, which is what the
owner asked to see when they asked for scales.

## 3. Acceptance criteria

- **AC1 — A recording plays across the grid.** One call takes a recorded take
  and returns a playable riff in which the take sounds once per grid degree,
  ascending, **on the beat** — whatever silence or room noise preceded the
  sound inside the capture window — and each hit measurably at its own ratio.
- **AC2 — Any sound works.** A hum, a knock, a click, a noise burst: none is
  rejected, and none requires a detectable pitch. A pitched take and an
  unpitched take both return a populated riff.
- **AC3 — The scale is visible.** The returned view's note cards are the grid's
  degrees, in order, so the user sees the scale they are hearing.
- **AC4 — The slider changes the scale.** A tenser setting yields a riff over at
  least as many degrees as a smoother one, and the degrees it adds are the more
  complex ratios.
- **AC5 — Bad input is a typed error, not a crash — and the user is told.** An
  empty take, a zero sample rate, a non-finite or out-of-range sample, or a take
  with **no sound in it** (a muted mic, a denied permission, room hiss alone)
  reports a typed `DspError`; nothing panics. The app shows the error rather than
  substituting something else for the user's sound.
- **AC6 — Deterministic and bounded.** The same take and setting always produce
  identical audio; output is finite and within `[-1, 1]`.
- **AC7 — Reachable from the app.** The shell exposes it as a command, and the
  UI offers it as a mode that stays on screen with every result (amended
  2026-09-27, see the decision log), reusing the existing playback, waveform,
  save, and export paths.
- **AC8 — Tests, docs, gates.** The library path is deviceless and fully tested;
  every public item documented; all four toolchain gates green.

## 4. Constraints & non-goals

- **Not note input.** Humming a *melody* to play through the recording, or
  placing notes one by one, is R-0036 (#64). Here the figure is the grid itself.
- **No instrument picker** (R-0031): this is a second way to hear a take, not a
  general "choose your instrument" surface.
- **No session persistence of the recording** — the riff saves and exports like
  any other, but reopening a song does not restore its sampled instrument. That
  needs the session format extended (R-0010) and is its own requirement.
- No scale *selection* (R-0037, #65): the grid is the harmonic series at the
  slider's odd-limit, as everywhere else in Easy Mode today.
- The Tauri shell is outside the cargo workspace, so its command is covered by
  the desktop-shell CI job and by the library tests beneath it, not by workspace
  unit tests.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-27 | **The modes stay on screen, and picking one from a result goes back to the mic** (owner decision, "arreglo rápido ahora") | The owner entered *mi instrumento* and could not find the way back. The modes lived inside the intro, so a result hid them, and its only exit, "↺ redo", reads as "record again". The pills now sit in the top bar, a pill on a result discards it and returns to the mic in that mode, and the exit says "← volver". Results were never kept, so nothing is lost that was kept before. This amends AC7's "next to the record button". |
| 2026-09-26 | **The take is cut to the sound inside it** before it becomes an instrument | Architect review, blocking. A take is a 3.5 s capture window; played whole, each hit replayed the lead-in first, so a knock 0.4 s in landed at beats 0.61, 1.49, 2.41, 3.35. Every earlier test fixture started at sample 0, which the app never produces. |
| 2026-09-26 | **Each hit is capped at one beat, with a short fade** (owner decision) | A held hum sounded all its degrees at once — a chord filling up, not a climb. Every figure degree is at or above `1:1`, so capping the sound caps every hit; the sampler itself still lets one-shots ring (R-0039). The fade answers R-0039's reason for not cutting: clicks. |
| 2026-09-26 | **Silence is a typed error, not an instrument** | A muted mic was accepted; −60 dBFS hiss was normalized +60 dB and played as the user's instrument. `DspError::Silent`, below −50 dBFS. |
| 2026-09-26 | **A sampled card shows the ratio and no pitch** | Degree `1:1` is whatever pitch the recording had, which nothing measured; a card saying "330 Hz" under a knock stated a number that is not what sounds. `NoteView::hz` / `cents` are `Option`s. |
| 2026-09-26 | **Refuse** a sample outside `[-1, 1]`, rather than clamp | `Sampler` holds audio (R-0039). A hot device sample fails this mode while hum mode accepts it; the difference is that this mode *plays the samples back*. |
| 2026-09-26 | **Neither the take's key nor its tempo is followed here** | Architect review of R-0041's interaction: a single knock has no pulse under R-0041's own gates, and the grid root has no audible effect on a sampled figure (degrees are ratios; the sampler ignores Hz). The figure should run on the *session* clock; until there is one, 92 BPM is it. |
| 2026-09-13 | The figure is **the grid itself, ascending** — one hit per degree | It is the shortest path from "I hit the table" to "that is an instrument", it needs no second recording, and it literally demonstrates what the owner asked for: the sound moved across the scale. A generated melody would be prettier and would hide what is being shown. |
| 2026-09-13 | The recording's **root octave is the octave the figure starts at**, passed explicitly | R-0039's blocking finding was that an implicit register is a bug waiting to happen. The caller states it; nothing is inferred. |
| 2026-09-13 | Returns the existing `RiffView`, not a new type | The shell already plays, draws, saves, and exports a `RiffView`. A parallel type would duplicate all four paths to say the same thing. |
| 2026-09-13 | Reuses the existing `record_start` capture, with a second stop command | One button to record, two ways to hear it back. Adding a capture path would duplicate the `!Send` recorder thread for no gain. |

## Changelog

- 2026-09-13 — created, accepted for M8.
- 2026-09-26 — architect review round 1 (request changes): AC1 and AC5 sharpened; five decisions added.
- 2026-09-27 — AC7 amended: the modes stay on screen with every result and are the way back from one (owner decision).
