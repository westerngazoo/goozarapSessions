# SPEC-0042 — Sing over a style

- **Status:** Proposed — architect review pending
- **Realizes:** R-0042
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-09-26
- **Depends on:** SPEC-0041 (`follow`, `riff_from_transcription`), SPEC-0026
  (`plan_sound`), SPEC-0025 (`parse_intent`), SPEC-0009 (`build_beat`),
  SPEC-0012 (mixdown)
- **Module(s):** `apps/gooz-studio` (`accompany.rs`), the Tauri shell, the UI

## 1. Motivation

Realize R-0042: a take and a style in, the singer over a styled track out — at
the singer's tempo, starting where they started.

## 2. Design

```
take ──analyze (once)──▶ Transcription
                            │
              ┌─────────────┼──────────────────────────┐
              ▼             ▼                          ▼
           follow      first onset                 notes
        (bpm, root)        │                          │
              │            ▼                          ▼
   style ──parse_intent──▶ intent ─(voice wins)─▶ plan_sound ──▶ SoundPlan
                           │                          │
                  trim lead-in to                build_beat at
                  the first onset               the plan's tempo
                           │                          │
                           ▼                          ▼
                      voice: RiffView           track: BeatView
```

### Types (`apps/gooz-studio/src/accompany.rs`)

```rust
/// Your take, with a track in a style, at your tempo.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Accompaniment {
    /// What the style became, at the take's tempo — inspectable, as in R-0027.
    pub plan: SoundPlan,
    /// The take, its lead-in trimmed so the first onset is beat 1. Its notes
    /// are what was sung, on the grid it was sung in; its `followed_*` fields
    /// say which of tempo and key came from the singer.
    pub voice: RiffView,
    /// The styled drums, at the same tempo, covering the take.
    pub track: BeatView,
}

pub fn accompany_take(
    samples: &[f32],
    sample_rate: u32,
    style: &str,
    tense: u8,
) -> Result<Accompaniment, DspError>
```

The voice is a `RiffView` and the track a `BeatView` **on purpose**: those are
the two types the shell already plays, draws, saves (`build_song`) and exports
(`export_master`). R-0041 already made `build_song` write the followed tempo
and root into `Settings`. So AC6 — one mix, and each part on its own — is the
existing riff + beat paths, not a new one.

### Tempo: the voice wins (AC1, AC4)

```rust
let mut intent = parse_intent(style);
if let Some(bpm) = heard.bpm {
    intent.tempo_bpm = bpm;           // the singer overrides the style text
}
let plan = plan_sound(&intent.normalized());
```

`parse_intent` is the same path the describe feature uses, so a chip sends
`"trap"` and a future free-text box (R-0029) sends a sentence, with no API
change. An unrecognized style falls to the neutral preset — and `plan.preset`
says `"free"`, so the UI cannot claim a style it did not play.

R-0041's reported range (60–180) sits inside `MusicalIntent`'s (40–250), so a
followed tempo is never clamped by `normalized()`.

### Phase: the first onset is beat 1 (AC2)

`StemPlacement` places stems by whole bars; a sub-bar offset cannot be
metadata. So the alignment is in the audio: the voice is the take from
`first_onset − PRE_ROLL` onward, and both stems start at bar 0.

- `PRE_ROLL` keeps the attack. Onset detection lands on the rise of spectral
  flux, a few milliseconds after a sung note physically begins; cutting exactly
  there clips the consonant.
- No onset at all (a held tone with no attack detected) → no trim.
- The notes shown on the cards are shifted by the same amount, so a card and
  its sound agree.
- The untrimmed recording is not lost: it is what the shell recorded, and the
  session keeps takes separately from stems.

### The track (AC3)

`build_beat` at the plan's tempo, with the plan's lanes (`beat_specs_from`,
made `pub(crate)` in `describe.rs` rather than copied), for
`bars = ceil(voice_secs / bar_secs).max(1)` so the track covers the whole take.

### The key (AC5)

Carried on `voice.followed_root_hz` and into saved `Settings` by R-0041's
`build_song`. The drums are rendered unpitched and do not read it — the result
must not imply otherwise.

### Shell and UI (AC8)

`record_stop_accompany(tense, style)` beside the other two stop commands,
sharing `record_start` and `stop_and_take`. The UI gains one row of style chips
(`corrido · trap · metal · libre`) shown in a third mode, *cantar sobre un
estilo*; the result screen plays the voice and the track together from one
play button, and save / export pass both as the riff and the beat.

## 3. Non-goals

Live accompaniment, bass (R-0033), harmony (R-0043), a generated melody over the
singer, pickup-note detection, free-text styles (R-0029), non-4/4 (R-0035).

## 4. Open questions

None.

## 5. Acceptance criteria mapping

- AC1 → a take at 126 BPM with style `"trap a 140"` yields a track whose bar
  length is 126's, not 140's and not the preset default.
- AC2 → a take whose first note starts 0.7 s in yields a voice whose first onset
  is within `PRE_ROLL` + one analysis hop of t = 0. A take starting at 0 is not
  trimmed.
- AC3 → the same take under `"trap"` and `"corrido"` gives different track
  patterns, each equal to what `plan_sound` would build for that preset.
- AC4 → a held tone with style `"trap"` gets the trap preset's tempo, and
  `voice.followed_bpm` is `None`.
- AC5 → `voice.followed_root_hz` is the take's root, `build_song` writes it, and
  the track audio is identical whatever the take's pitch.
- AC6 → `build_song(voice, track)` then `export_master` produces a mix; each
  stem is separately present in the song.
- AC7 → typed errors, a hostile sweep, determinism, bounds.
- AC8 → the command compiles in the desktop-shell job; the UI calls it (verified
  in the browser).
- AC9 → four gates + docs.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-26 | Voice and track are a `RiffView` and a `BeatView`, not a new mix type | The shell already plays, saves and exports exactly that pair, and R-0041 made the save honest about tempo and key. A new type would duplicate four paths. |
| 2026-09-26 | Style text goes through `parse_intent` | One path for chips today and free text tomorrow; an unknown style is visibly the neutral preset, not silently something. |
| 2026-09-26 | Align by trimming the voice, not by shifting the track | Keeps both stems at bar 0, which every mix and export path already assumes; the alternative needs sub-bar placement the session format does not have. |
| 2026-09-26 | One `analyze` for follow, alignment and notes | It is ~350 ms, and all three read the same transcription. |

## Changelog

- 2026-09-26 — created; proposed for architect review.
