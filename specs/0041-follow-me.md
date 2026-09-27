# SPEC-0041 — Follow me

- **Status:** Accepted — architect round 1 addressed; QA round 1 FAIL addressed by a redesign of the tempo method (owner decision)
- **Realizes:** R-0041
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-09-21
- **Depends on:** SPEC-0005 (`analyze`), SPEC-0002 (`Tempo`), SPEC-0001 (grids)
- **Module(s):** `crates/gooz-dsp` (`follow.rs`), `crates/gooz-model`
  (`features.rs`, the move), `apps/gooz-studio` (`view.rs`, the wiring)

## 1. Motivation

Realize R-0041: let a take say how fast it is and where it sits.

## 2. Design

```
take ──┬── analyze ── pitch track ── voiced runs ≥ 80 ms ── median ──▶ root_hz: Option
       │
       └── loudness per 10 ms ── rises past 1 dB ── spread ±30 ms ──
               autocorrelation over 30–180 BPM, leaning to 120 ──
               strongest repeat ≥ 0.4 ? fold < 60 up once ──────────▶ bpm: Option
```

Pitch comes from the analysis R-0005 already runs. **Tempo does not**: it is
read from the signal's own loudness, which the transcription does not carry.

### The numbers

Every acceptance outcome turns on these, so they are the design, not detail.
Each was chosen against a signal measured to get through without it.

| Constant | Value | Why |
|---|---|---|
| `FRAME_STEP_SECS` | 10 ms | Fixed in time, so a take reads the same at 16, 44.1 and 48 kHz. |
| `FRAME_WINDOW_SECS` | 40 ms | Several periods of any sung pitch, so a steady note's loudness does not ripple. |
| `SILENCE_DB` | −60 dBFS | Loudness floor: an attack out of silence rises from here, not from −∞. |
| `RISE_FLOOR_DB` | 1 dB | A held note moves by hundredths of a dB; an onset by several. A held hum — which reported **122 BPM** under the old design — contributes no attacks at all. |
| `ATTACK_SPREAD_SECS` | ±30 ms | Unspread, a ±20 ms human wobble made successive beats miss each other and a 92 BPM take was refused. |
| `SLOWEST_SEARCHED_BPM` | 30 | A 40 BPM pulse repeats at no period inside 60–180; it has to be found at its own and doubled. |
| `MIN_PULSE_STRENGTH` | 0.4 | Three even hits score ≈ ⅔; two close hits and a distant third score ≈ ⅓ — which the old design reported as **120 BPM**, its spread gate unable to fire on two intervals. |
| `PRIOR_CENTRE_BPM` / width | 120 / 1 octave | Breaks the tie when a pulse repeats about equally at two octaves, without overruling one that is clearly stronger. |
| `MIN_RUN_SECS` | 80 ms | A sung note is at least this long; shorter runs are breath or consonants. |
| `MIN_VOICED_SECS` | 0.25 s | Total sung voicing a root may be read from. |

### Types (`gooz-dsp/src/follow.rs`)

```rust
/// What a take says about itself.
///
/// `None` means **the take did not say** — a knock has no pitch, one long note
/// has no pulse. This layer reports what it heard; it does not know what the
/// caller's defaults are, and inventing one here would make it impossible for
/// a UI to tell the user which of the two it actually followed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Follow {
    /// The take's tempo in BPM, folded into [`MIN_BPM`]..=[`MAX_BPM`].
    pub bpm: Option<f64>,
    /// The pitch the take sits around, in Hz.
    pub root_hz: Option<f64>,
}

/// Pure and infallible: everything it needs is already in the analysis.
pub fn follow(transcription: &Transcription) -> Follow;

/// `analyze` then `follow`.
pub fn follow_take(signal: &[f32], sample_rate: u32, cfg: &Config)
    -> Result<Follow, DspError>;
```

The pure form is the primary one. `analyze` costs ~350 ms on a few seconds of
audio, and the take path needs the transcription **twice** — once to choose the
grid and the clock, once to render on them. Without the split, following a take
doubled the pipeline's analysis cost. `pipeline.rs` gains
`riff_from_transcription` for the same reason, and R-0042 will want both.

### Tempo (AC1, AC3, AC4): autocorrelation of energy attacks

**Why not the median of onset intervals.** That was the first design, and three
rounds of review kept finding new ways it failed. Spectral-flux onsets fire on
the steady wobble of a held note — vibrato, or just a sustained harmonic tone —
and a wobble is perfectly regular, so no interval gate can refuse it: a held hum
reported 122 BPM, vibrato 165. Swing has two interval lengths and no single
median pulse: 2:1 swing at 120 reported 90.7 or 175.8 depending on how many
notes were sung. With exactly three onsets the spread gate could never fire.
Each gate fixed one case and the next review found another.

**What replaces it** is the method beat trackers use:

1. **Attacks, not onsets.** Loudness in dB every 10 ms over a 40 ms window;
   each frame's rise over the last, less a 1 dB floor. Pitch wobble at constant
   loudness contributes nothing.
2. **Spread** each attack over ±30 ms, so a hand-played beat that lands a
   little early still lines up with one that lands a little late.
3. **Autocorrelate** over the periods of 30–180 BPM, weighted toward 120 BPM by
   a one-octave log-Gaussian so that a pulse heard at two octaves resolves to
   the one people usually feel. A swung bar repeats at the beat; rests
   reinforce the period rather than break it.
4. **Refuse** when the strongest repeat is below 0.4 of the envelope's energy.
5. **Refine** between frames with a parabola through the peak and its
   neighbours, then **fold** a pulse found below 60 BPM up by one octave.

`tempo_of(signal, sample_rate)` is public and is now the project's **only**
tempo estimator. R-0015's `extract_features` uses it too, replacing a plain
median of intervals — so a reference with no pulse writes the format's `0.0`
instead of a tempo spectral flux invented, and a pulsed reference is measured
the same way a take is.

### Root (AC2, AC3): sung notes, read in runs

The median pitch of the take's **sung notes**. Voicing is read in runs of
continuous voiced frames; runs under 80 ms are set aside as breath, consonants
or a stray frame, and what remains must add up to a quarter of a second.

The previous gate measured voicing *density* — first against the whole take,
which refused a phrase with silence around it, then against the voiced span,
which QA showed still refused detached notes (each shorter than half its beat)
and two phrases with a breath between them. Both are ordinary singing. The real
question is whether there are sung notes, not how much of the take they fill.

The median is an order statistic, lower middle for even counts. It is the take's
**central pitch, not its tonic**: QA measured a two-octave C-major arpeggio
following to E, where the harmonic grid then snaps its C's to B. That is the
accepted non-goal of R-0041 §4, made visible — a key centre needs R-0037's
scales and a real key finder.

### What the caller does with the `Option`s (AC7)

`RiffView` gains `followed_bpm` and `followed_root_hz`. Without them the
`Option`s die one call after they are computed, the UI cannot say which of the
two it followed, and the decision that motivated the whole shape buys nothing.
They are also the only direct test surface for AC7.

Three consequences follow, and all three are in scope because the alternative
makes the product worse on its main path:

- **the beat follows the riff** — `beat_view(busy, bpm)`, so a take heard at
  126 BPM is not mixed against a 92 BPM loop in `export_master`;
- **a saved session tells the truth** — `build_song` writes the followed tempo
  and root into `Settings` instead of the constants, so a file does not claim
  92 BPM for a riff rendered at 126;
- **the described-song path reports `None`** — it follows a prompt, not a take.

### Wiring (AC7)

`easy_mode_grid(tense)` and `easy_mode_tempo()` gain take-aware siblings rather
than changing meaning:

```rust
pub(crate) fn followed_grid(tense: u8, follow: &Follow) -> PitchGrid
pub(crate) fn followed_tempo(follow: &Follow) -> Tempo
```

each falling back to `GRID_ROOT_HZ` / `TEMPO_BPM` when the take said nothing.
`riff_from_take` follows; `demo_riff` does not (owner decision).

## 3. Non-goals

No tempo curve, no meter detection, no tonic estimation, no accompaniment
(R-0042). See R-0041 §4.

## 4. Open questions

None.

## 5. Acceptance criteria mapping

- AC1 → a click train at a known BPM reports it within tolerance.
- AC2 → a steady sine at a known Hz reports it within a few cents.
- AC3 → a pitchless noise burst reports `root_hz: None` *and* still reports a
  tempo; one long sine reports `bpm: None` *and* still reports a root. The two
  must be shown independent, or a single `Option<Follow>` would have passed.
- AC4 → a click train at 300 BPM folds to 150; one at 30 folds to 120 (×4);
  an estimate that cannot fold reports `None`.
- AC5 → empty / zero rate / non-finite give the matching typed error, plus a
  hostile sweep (one sample, two samples, `f32::MIN`/`MAX`, rate 1 and
  `u32::MAX`).
- AC6 → two identical calls are equal.
- AC7 → a studio-level test that a take at a different tempo and pitch produces
  a riff whose grid root and tempo are the take's, and that `demo_riff` is
  byte-identical to before.
- AC8 → four gates + docs.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-26 | **Tempo is an autocorrelation of energy attacks**, replacing the median of onset intervals (owner decision) | QA round 1: a held hum reported 122 BPM, vibrato 165, three scattered notes 120, and swing flipped between 90.7 and 175.8 with the note count. Three rounds of gates had not converged; the method was wrong. |
| 2026-09-26 | **Swing is in scope** (owner decision) | Corrido tumbado and much trap are felt in triplets; a take sung with swing has to be followed at its beat. |
| 2026-09-26 | **One tempo estimator** for the project: R-0015 uses `tempo_of` too | Two estimators drift, and the old one was the thing that failed. R-0015's output changes for references with no pulse (now `0.0`) — pinned by tests. |
| 2026-09-26 | **The root is read from sung runs**, not voicing density | QA round 1: density refused detached notes and two phrases with a breath. |
| 2026-09-26 | **A `bpm` from the webview must be in range to count as followed** | QA round 1: `beat_view(_, Some(1e-300))` overflowed the beat builder into a panic; `1e9` rendered a two-sample beat. |
| 2026-09-26 | **The beat is re-fetched for every new riff**, and the result shows the tempo it was laid out at | QA round 1: a beat fetched earlier was mixed under a riff at another tempo and saved under settings claiming the new one; the label said "92 bpm" for everything. |
| 2026-09-21 | One `analyze` call, not `pitch_track` + `detect_onsets` separately | `analyze` already does both in one pass and is the reviewed entry point; calling the halves separately would double the work and duplicate its validation. |
| 2026-09-21 | A voiced *fraction* gate, not just a voiced *count* | Three spurious voiced frames in a snare hit would otherwise be enough to report a key, and a wrong key is worse than no key: the caller's fallback is at least a known quantity. |
| 2026-09-21 | `followed_grid` / `followed_tempo` are new (and private), rather than changing `easy_mode_*` | The demo wants the old meaning, and changing the existing functions under it would silently re-tune a fixed reference. (An earlier draft claimed R-0027's path was a second such caller; it is not — `describe.rs` uses `GRID_ROOT_HZ` directly and builds its own `Tempo`.) |
| 2026-09-21 | `demo_riff` calls `hum_to_riff` directly rather than going through `riff_from_take` | It used to be implemented in terms of it, which made "the demo does not follow" impossible: measured, the demo hum reports 126.4 BPM and 333 Hz, so it would have re-tuned itself and broken its own existing test. A golden test now pins the demo's ratio sequence, bar count and length. |

## Changelog

- 2026-09-26 — QA round 1 (FAIL): tempo method redesigned, root gate replaced, R-0015 moved onto the same estimator, beat guarded and re-fetched.

- 2026-09-21 — created; proposed for architect review.
