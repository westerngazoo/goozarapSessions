# SPEC-0042 — Sing over a style

- **Status:** Accepted — design review, implementation review and QA (FAIL on AC2) all addressed; four owner decisions recorded
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

### The anchor: where the first sung note physically begins (AC2)

`gooz_dsp::first_sung_note(signal, rate, transcription, cfg)` answers two
questions with two signals:

- **Which note** — the first run of continuous voicing at least 80 ms long in
  the pitch track **whose sound is still going 60 ms after it starts**. Room
  noise, a breath, a click and an unvoiced consonant are not voiced. A brief
  pitched squeak *is* — and the pitch window smears its voicing over its own
  length, so at 16 kHz (a 128 ms window) a 30 ms squeak reads as a ~160 ms
  voiced run; it is passed over because its *sound* does not last. (Requiring
  a longer voiced run instead skipped a 140 ms spoken syllable.) "Still going"
  is asked at one point, not as "above the gate the whole way": a 5 ms loudness
  window spans about one period of a sung pitch and ripples with its phase, so
  at a soft onset's gate crossing it dips straight back under.
- **Where it starts** — pitch frames are coarse and late (YIN needs its window
  substantially periodic), so the start is read from loudness: the contiguous
  stretch around the first sung frame where a 5 ms trailing RMS is at or above a
  gate. The gate is 10.5 dB under the note's level, or halfway in dB between
  the note and the room before it, whichever is higher. The search reaches back
  at most half a pitch window plus two hops — the most the first sung frame can
  be late.

**Why not the obvious anchors.** Both were tried and both failed QA:

| Anchor | Failure, measured against where the fixture put the note |
|---|---|
| the first onset | a −40 dBFS room fires an onset at t = 0; a breath fires one 170 ms early |
| the first transcribed *note* | a soft note's own onset is stamped after its first voiced frame, so the segment from the room-noise onset at 0.0 becomes "the first note" — **+700 ms** at −50 dBFS |
| that note, refined to "30 % of the early peak" in a ±window | an "s" before the vowel **+47 ms** at 16 kHz; a quiet take in a noisy room **+40 ms** |

With `first_sung_note`, every case QA built lands within one analysis hop, at
48, 44.1 and 16 kHz: soft onsets of 40–80 ms (inside their own swell), a
sibilant before the vowel, a quiet singer in a −45 dBFS room, loud breaths,
lip smacks, clipping, swing, drift, speech rhythm, legato, and a pitched squeak
before the song.

**Known limit — a room that drowns the voice.** The part of the gate that sits
halfway between room and note cannot be reached end to end: in every case built
(a −40 dBFS room, low rumble 8 dB under a quiet singer) the pitch tracker stops
hearing the voice before the room gets within 10.5 dB of it. The take is then
refused with a typed error — never laid against the noise. The room term is
tested directly on `first_sung_note` with a pitch track that does hear the
voice, so it holds the day the tracker improves.

**Known limit — the syllable's perceptual centre.** The beat belongs on the
vowel. A consonant loud enough to clear the gate (a plosive burst) is counted
as the note's start, so its vowel lands up to ~20 ms late. Inside the range a
listener hears as "together"; a vowel-onset model is a later refinement.

### Exactly one bar in (AC2, owner decisions)

```
kept_from = floor(anchor / bar) · bar      pad = bar − (anchor − kept_from)
voice     = [pad of silence] + take[kept_from..]
```

The first sung note lands on the downbeat of **bar 2**, after exactly one bar of
drums, however long the singer waited after tapping. Whole bars of lead-in
before the note's own bar are dropped — silence and room, which otherwise
became up to five bars of drums over room noise, replayed on every loop — and
only whole bars, so the phase is untouched. Nothing after the note's bar line is
ever cut. (`bar` is at least one sample; `bar_samples` guarantees it.)

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
- **One channel**: the shell downmixes with `gooz_audio::Take::mono`. Capture
  hands over interleaved frames, and a two-channel microphone read as one
  channel is half speed and an octave low — measured, a 262 Hz take read as
  131 Hz and its tempo halved. Pre-existing since R-0013; exposed by R-0042,
  where the raw voice *is* the product.
- **One level**: the voice is peak-normalized to −1 dBFS. A laptop take peaking
  at −20 dBFS otherwise sits ~19 dB under drums normalized to full scale. (A
  stop-tap click louder than the singing sets the gain; normalizing on the
  sung span is a later refinement.)
- **No clicks**: 5 ms fades at both ends of the take — it is cut wherever the
  user tapped stop.

### What the result says about itself (AC4, AC5, AC6)

`RiffView` gains two fields, for every path:

- `bpm: f64` and `beats_per_bar: f64` — the clock it was **laid out at**, read
  from the `Tempo` the audio was rendered on (not the plan's raw number, which a
  hand-edited plan could make NaN). `build_song` writes both into `Settings`.
  Before: `followed_bpm.unwrap_or(92)` — right only because three unrelated
  constants were 92 — and always 4 beats, which made a described 6/8 song's
  mixdown two-thirds the length of its drums. A described song now saves its
  own tempo instead of 92; that was a latent bug in R-0027's save path.
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
- A third mode, *sobre un estilo* (the modes live in the top bar and are the
  way back from a result since the SPEC-0040 amendment of 2026-09-27), shows the chips (fetched once, retried if
  the request fails, never duplicated). The result plays voice and track from
  **one** play button, both started at the same `AudioContext` time with loops
  of equal length. While a styled track is the beat, the busy slider and the
  beat button are **disabled**: either would restart the drums alone "now" while
  the voice kept its place, out of step on every drag.
- The stop commands are `async`: a 30 s take takes seconds to analyse, and a
  synchronous Tauri command runs on the UI thread.

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
| 2026-09-27 | **Exactly one bar in**: whole bars of lead-in before the note's bar are dropped (owner decision) | A 10 s wait became five bars of drums over room noise, replayed every loop. Dropping whole bars keeps the phase. |
| 2026-09-27 | The anchor is **`first_sung_note`**: the first sustained voiced run, located by a loudness gate | QA round 1, AC2 FAIL: the first transcribed note entered 700 ms late on a soft onset, and the unspecced attack refinement put a vowel 47 ms late behind an "s". |
| 2026-09-27 | **Downmix to mono** at capture | Architect review: a two-channel microphone was analysed and played at half speed, an octave low. |
| 2026-09-27 | `RiffView` carries **`beats_per_bar`**, and `bpm` comes from the rendered `Tempo` | QA: a described 6/8 song mixed down to two-thirds of its drums. Architect: a NaN plan tempo would have been saved as `null` and not loaded back. |
| 2026-09-27 | While a styled track plays, **the busy slider and beat button are off** | Architect review: either restarted the drums alone and knocked them out of step with the voice. |
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
- 2026-09-27 — implementation review (architect: request changes) and QA (FAIL on AC2): anchor rebuilt as `first_sung_note`, exactly one bar in, mono capture, `beats_per_bar`, locked playback, async stop commands.
- 2026-09-27 — architect round 1 (request changes: anchor, stem invariants, a requirement contradiction, a coincidental clock, a style tempo that did not exist, vacuous test mappings) and three owner decisions: count-in, per-style tempos, tap to stop.
