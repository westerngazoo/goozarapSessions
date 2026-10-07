# R-0043 — Studio view: the mic at the centre, tracks you can drag

- **Status:** Draft — acceptance criteria proposed to the owner
- **Milestone:** M8
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-10-06
- **Depends on:** R-0011 (arrangement), R-0012 (mixdown & export), R-0013
  (studio shell), R-0040 (play my sound), R-0041 (follow me), R-0042 (sing over
  a style), R-0033 (808 bass)
- **Realized by:** SPEC-0043
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The studio must look and work like a studio: **one screen**, with the **mic at
its centre** and the genres around it, a **palette of instruments**, and the
song's **tracks laid out on a bar grid** underneath. The owner sings and the
tracks appear. They drag instruments in to add parts, drag clips along the bars
to move them, and silence, level or remove any track. Nothing on the screen is
a dead end.

The owner asked for it in their own words: *"crea una ui que parezca más
estudio … entro a crear instrumento y ya no puedo regresar … hazlo más como una
pieza musical que pueda arrastrar instrumentos y tenga el mic central o la parte
de improvisación para yo comenzar a cantar y poner un género"*.

## 2. Rationale

The shell grew one mode at a time: tararear (R-0008), mi instrumento (R-0040),
sobre un estilo (R-0042). Each mode was a separate trip from the mic to a result
and back, and the result showed one take at a time. A song is several parts at
once. The engine already has what a song needs, because R-0011's arrangement
places stems at bars, mutes them and sets their levels, but nothing on screen
reached it.

This view turns the existing pieces into one place to make a song. It is also
where the next requirements land: the genre cards ("qué instrumentos usa, qué
ritmo, cómo es una canción") and the backing track that follows the melody.

## 3. Acceptance criteria

- **AC1 — One screen, no dead ends.** The screen has four areas, all visible
  in every state, and every control in them is reachable whatever was done
  last:
  - a transport bar: play/stop, the tempo and whose it is, the root, the
    genre, save and export;
  - the mic at the centre, with the genres;
  - the instrument palette;
  - the tracks.

  While a take is recording, the controls that would change the song are off
  until it stops. The view replaces the three modes:
  - *tararear* becomes "sing, then drag in the guitar";
  - *mi instrumento* becomes the palette's "mi sonido";
  - *sobre un estilo* becomes the default flow.
- **AC2 — The first take starts the song.** With a genre chosen, the first
  sung take behaves as R-0042 does today:
  - it sets the tempo and the root (R-0041);
  - its voice enters on a downbeat after one bar;
  - the genre's drums and, for trap, its 808 (R-0033) appear as tracks.

  The transport bar shows the tempo and the root, and whether they came from
  the voice.
- **AC3 — Every later take is a new voice track** (owner decision). It is laid
  on the session's clock and does not change the tempo or the root. It enters
  on a downbeat by the same rule as the first take, and the earlier tracks are
  untouched.
- **AC4 — Drag an instrument in to add its part** (owner decision). Dropping an
  instrument from the palette onto the tracks area adds that instrument's
  track, made from what the session already holds:
  - **Batería:** the genre's drums at the session's tempo.
  - **808:** R-0033's 808 on the drums' kick, at the session's root.
  - **Guitarra:** the melody of a voice take, quantized to the session's grid,
    played on the guitar (today's hum → riff).
  - **Mi sonido:** record a sound; it plays a voice take's melody, or R-0040's
    rising figure when there is no voice.

  An instrument that cannot be made yet, such as a guitar with no voice to
  follow, says why and adds nothing. Instruments that do not exist yet
  (requinto, tololoche, …) are shown as coming, not selectable.
- **AC5 — Drag a clip along the bars** (owner decision). A clip moves by whole
  bars, snapping to the grid, never before bar 1. The song grows to fit the
  furthest clip. Moving a clip changes only that track.
- **AC6 — Each track can be muted, levelled and removed.** The level ranges
  from 0 to 1. Removing the last voice keeps the session's tempo and root.
- **AC7 — What you hear is what you export.** Playback is the session's own
  mixdown (R-0012): the same clips at the same bars and levels, with the same
  peak limit, so nothing clips. After any change, the next play reflects it.
  Save writes this session and export writes this mix, the same way R-0012 does.
- **AC8 — Genres around the mic** (owner decision). The working styles
  (corrido, trap, metal, free) can be chosen. Choosing another one re-renders
  the drums and the bass at the session's tempo and leaves the voices alone.
  The genres researched in `docs/genres.md` (tumbado, sierreño, norteño, tríos,
  blues) are shown marked as coming ("pronto") and cannot be chosen yet.
- **AC9 — The song lives in Rust.** The session (its tracks, clips, bars,
  levels and mutes) is held and changed by the backend, through commands the
  shell exposes. The UI draws it and sends gestures; it holds no audio math.
  The session's behaviour is tested without a device.
- **AC10 — Tests, docs, gates.** Every public item is documented. All four
  toolchain gates are green. The UI is verified in the browser preview against
  a mocked shell.

## 4. Constraints & non-goals

- **No latency compensation.** A later take is aligned by its first sung note,
  as in R-0042, not by measuring the round trip. Hearing the song while
  recording is allowed, and headphones keep the speakers out of the take.
- **Not in this requirement:**
  - the genre cards' content and the new genres' sounds (the next requirement);
  - harmony that follows the melody;
  - new instruments;
  - non-4/4 meters (R-0035).
- **No clip editing.** No trimming, splitting, copying, reordering lanes, undo,
  or sections and loop regions on screen. Those can come later; the session
  model already holds sections.
- **One session at a time.** No session browser; opening a saved session is a
  later step.
- **Offline rendering.** Every change re-renders offline, as every renderer does
  today. Nothing runs on the audio callback.

## 5. Open questions

None. The owner's decisions are logged below.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-10-06 | **Mic at the centre, tracks underneath** (owner) | The studio is for improvising first. Singing is the first gesture, and the tracks grow under it. |
| 2026-10-06 | **Drag instruments in, and drag clips along the bars** (owner) | "Una pieza musical que pueda arrastrar instrumentos": parts are added by dropping them in, and placed by moving them in time. Reordering lanes is left out of v1. |
| 2026-10-06 | **Another take is a new voice track** (owner) | Layering voices (a second voice, a chorus) is what a studio is for. Unwanted takes are muted or removed. |
| 2026-10-06 | **Future genres shown as "pronto"** (owner) | It says where the studio is going, and the chips switch on with the genre cards. |
| 2026-10-06 | Playback is the session's mixdown | One sum, rendered by the code that exports, makes "what you hear is what you export" true by construction. Clips at different bars cannot drift, and the UI holds no audio math (R-0033's lesson, taken one step further). |

## Changelog

- 2026-10-06 — drafted with the owner's four layout decisions.
