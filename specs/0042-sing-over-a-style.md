# SPEC-0042 — Sing over a style

- **Status:** Accepted — architect design review round 1 (request changes) addressed; three owner decisions recorded
- **Realizes:** R-0042
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-09-26
- **Depends on:** SPEC-0041 (`follow`, `tempo_of`), SPEC-0040 (the record/stop
  commands and the mode toggle), SPEC-0026 (`plan_sound`, the preset table),
  SPEC-0025 (`parse_intent`), SPEC-0009 (`build_beat`), SPEC-0012 (mixdown)
- **Module(s):** `apps/gooz-studio` (`accompany.rs`, `view.rs`),
  `crates/gooz-model` (`preset.rs`), the Tauri shell, the UI

## 1. Motivation

Realize R-0042: a take and a style in, the singer over a styled track out — at
the singer's tempo, entering on a downbeat.

## 2. Design

```
take ── analyze (once) ─┬─ follow ──────────────────── bpm?, root?
                        ├─ first sung note ─────────── anchor
                        └─ notes ── quantize ───────── the cards
style ── parse_intent ── plan_sound ─── lanes + the style's own tempo
                                             │
clock  = the voice's bpm  or  the style's tempo      (never the text: chips carry none)
meter  = 4/4
voice  = [count-in silence] + take, faded, normalized, padded to whole bars
track  = build_beat(clock, the take's sample rate, lanes, the voice's bar count)
```

### Types (`apps/gooz-studio/src/accompany.rs`)

```rust
/// Your take, with a track in a style, at your tempo.
pub struct Accompaniment {
    /// What the style became, on the clock it was rendered at.
    pub plan: SoundPlan,
    /// The take, entering on the downbeat of bar 2. `part == Part::Voice`.
    pub voice: RiffView,
    /// The styled drums, exactly as long as the voice.
    pub track: BeatView,
}

pub fn accompany_take(samples: &[f32], sample_rate: u32, style: &str, tense: u8)
    -> Result<Accompaniment, DspError>
```

Voice and track are a `RiffView` and a `BeatView` because those are what the
shell already plays, saves (`build_song`) and exports (`export_master`). That
only holds if their invariants hold, which the first draft left unstated —
**same sample rate, same length, a whole number of bars** — so they are stated
and tested here.

### The anchor: the first sung note (AC2)

The first *note* of the transcription, not the first *onset*. Measured, the
onset detector puts a spurious onset at t = 0 under a −40 dBFS room noise floor
(so nothing would move and the singer would sit 1.5 beats off the drums), and
fires on a breath or a click before the first note. `assemble_notes` discards
segments with no voiced frames, so `notes[0]` lands on the sung note in every
case measured. A take with no note at all is `DspError::Silent`.

### The count-in (AC2, owner decision)

The voice is **delayed**, not trimmed, so the first note lands on the downbeat
of the bar after the one it started in:

```
entry = (floor(anchor / bar) + 1) · bar        pad = entry − anchor
```

The drums always play at least one bar alone, and the singer enters on beat 1.
Nothing is cut — not a breath, not a consonant — so there is no pre-roll to
guess (the architect measured the detector firing *early* on sharp attacks, so
any pre-roll would have put the singer behind the kick), and the recording is
the stem as recorded, only later.

### The clock (AC1, AC4, owner decision)

- the voice's tempo, when `follow` heard one; otherwise
- **the style's own tempo** — a new column in the preset table: corrido 105,
  trap 140, metal 160, free 92. `SoundPlan` carries it as `style_bpm`.

The style *text* is never consulted for tempo in this mode. The chips send a
preset id and no number, so there is nothing to read; and `MusicalIntent`
cannot say whether a tempo was stated or defaulted (the parser writes 92 either
way), so reading it would be correct only by coincidence. Stated-tempo
precedence belongs to R-0029's free-text box, with an `Option` there.

The meter is forced to 4/4 (R-0042 §4): a style that parses to 6/8 would lay
its bar 1.5× the session's, and export would cut the drums mid-bar.

### Voice and track, as one mix (AC6, AC7)

- **One sample rate**: the track is rendered at the take's rate. `export_master`
  refuses stems of different rates, and a 44.1 kHz or 16 kHz microphone plus a
  48 kHz track would fail every export.
- **One length**: the voice is padded to a whole number of bars, and the track
  is rendered for exactly that many. Unequal loops drift from the first repeat,
  and `mixdown` — which wraps each stem by its own length — would replay the
  singer's first second at the end.
- **One level**: the voice is peak-normalized to −1 dBFS. A laptop take peaking
  at −20 dBFS otherwise sits ~19 dB under drums normalized to full scale.
- **No clicks**: 5 ms fades at both ends of the take — it is cut wherever the
  user tapped stop.

### What the result says about itself (AC4, AC5, AC6)

`RiffView` gains two fields, for every path:

- `bpm: f64` — the clock it was **laid out at**. `build_song` writes this into
  `Settings`, instead of `followed_bpm.unwrap_or(92)`, which was right only
  because three unrelated constants happen to be 92.
- `part: Part` — `Guitar` (hum→riff), `Instrument` (R-0040's sampled figure),
  `Voice` (this). `build_song` names the stem from it, so a voice is no longer
  saved as `00-guitar.wav`, and neither is R-0040's knock. The session format
  is unchanged: a voice is stored as `StemKind::Other` named `"voice"`.

`followed_bpm` / `followed_root_hz` keep meaning *what came from the take*; the
UI labels the result "tu tempo" or "tempo del estilo" from them.

### The key (AC5)

Carried on `voice.followed_root_hz` and written into `Settings`. The note cards
are the take's notes quantized onto the followed grid. The drums do not read
it — and they are not unpitched either (the kick sweeps to ~45 Hz, the snare
has a 180 Hz body), so the UI must not say the track is "en tu tono".

### Shell and UI (AC8)

- **Tap to stop** (owner decision): tap to start, tap again to stop, in every
  mode; the capture cap rises from 12 s to 30 s.
- `record_stop_accompany(tense, style)` beside the other two stop commands, and
  `styles()` returning the preset ids, so the chips are the preset table and
  cannot drift from it.
- A third mode, *sobre un estilo*, shows the chips. The result plays voice and
  track from **one** play button, both started at the same `AudioContext` time
  with loops of equal length. The busy slider does not re-fetch a generic beat
  over the styled track in this mode.

## 3. Non-goals

Live accompaniment, bass (R-0033), harmony (R-0043), a generated melody over the
singer, pickup-note detection, free-text styles and stated-tempo precedence
(R-0029), non-4/4 (R-0035), a `StemKind::Voice` in the session format.

## 4. Open questions

None.

## 5. Acceptance criteria mapping

- **AC1** → a take sung at a known tempo, styled `trap`, gives a track whose bar
  is the *followed* tempo's (compared to `voice.followed_bpm`, not to the input
  number) and not trap's 140 — the numbers chosen so no preset tempo coincides.
- **AC2** → a take whose first note starts 0.7 s in, **under a −40 dBFS noise
  floor and after a breath**, has that note on the downbeat of bar 2 to within
  one analysis hop, measured against where the fixture put it — not re-detected
  with the same detector.
- **AC3** → the same take under `trap` and `corrido` gives different tracks, and
  the snare lands where each preset puts it (corrido on beat 3).
- **AC4** → a held tone styled `metal` is laid out at 160, `followed_bpm` is
  `None`, and the label says the tempo is the style's.
- **AC5** → `followed_root_hz` is the take's root (260 Hz, not a 220-family
  pitch), `build_song` writes it, and the track's audio is identical for two
  takes with the same tempo and different pitches.
- **AC6** → `voice.samples.len() == track.samples.len()`, same rate, whole bars;
  `build_song` + mixdown at 44.1 kHz succeeds; muting either stem changes the mix.
- **AC7** → typed errors (empty, zero rate, non-finite, out-of-range, silent,
  shorter than the window), determinism, bounds.
- **AC8** → the chips equal the preset table; the command compiles in the
  desktop-shell job; the UI flow verified in the browser.
- **AC9** → four gates + docs.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-27 | **Count-in, not trim** (owner decision) | Lossless; no pre-roll to guess; the singer always enters on a downbeat after a bar of drums. |
| 2026-09-27 | **Each style has its own tempo** (owner decision): corrido 105, trap 140, metal 160, free 92 | Architect review: "the style's own tempo" did not exist — every chip gave 92, Easy Mode's default wearing the style's name. |
| 2026-09-27 | **Tap to stop, 30 s cap** (owner decision) | A fixed 3.5 s window made every accompaniment a one- or two-bar loop. |
| 2026-09-27 | Anchor on the first **note**, not the first onset | Architect measurement: a −40 dBFS noise floor put the first onset at t = 0; a breath put it 170 ms early. |
| 2026-09-27 | The clock is the voice or the style — **never the text** | Chips carry no number, and `MusicalIntent` cannot tell a stated 92 from a default one. |
| 2026-09-27 | `RiffView` carries `bpm` and `part` | The saved tempo was right by coincidence; voices and knocks were saved as "guitar". |
| 2026-09-26 | Voice and track are a `RiffView` and a `BeatView` | The shell already plays, saves and exports that pair; their invariants are now stated and tested. |
| 2026-09-26 | Style text goes through `parse_intent` | One path for chips today and free text tomorrow. |

## Changelog

- 2026-09-26 — created; proposed for architect review.
- 2026-09-27 — architect round 1 (request changes: anchor, stem invariants, a requirement contradiction, a coincidental clock, a style tempo that did not exist, vacuous test mappings) and three owner decisions: count-in, per-style tempos, tap to stop.
