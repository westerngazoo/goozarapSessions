# SPEC-0041 — Follow me

- **Status:** Accepted — architect-reviewed (round 1: request changes, addressed)
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
take ──analyze──┬─ onsets  ──▶ estimate_bpm ──▶ fold into range ──▶ Option<f64>
                └─ pitches ──▶ median voiced  ──────────────────▶ Option<f64>
```

One `analyze` call, not two: it already returns the pitch track and the onsets
together, so following a take costs exactly one pass over it.

### The numbers

Every acceptance outcome turns on these, so they are the design, not detail.
Each was chosen by measuring a signal that got through without it.

| Constant | Value | What it rejects, measured |
|---|---|---|
| `MIN_ONSETS` | 3 | Two notes 9 s apart reported a confident **103.2 BPM**. One interval is a gap, not a tempo. |
| `MAX_IOI_SPREAD` | 0.15 | A free-time hum (spread 0.54) reported **117.8 BPM**. A long-short feel is *not* rejected: `0.4 s`/`0.8 s` is one grid of `0.4 s`, reported as 150 BPM. |
| `MAX_FOLDS` | 2 | At eight folds (256×) every take with ≥2 onsets landed in range, making `None` unreachable and the `Option` decorative. |
| `MIN_VOICED_FRAMES` | 8 | A handful of frames is not a phrase. |
| `MIN_VOICED_SECS` | 0.25 | Nor is 20 ms of it. |
| `MIN_VOICED_DENSITY` | 0.5, **within the voiced span** | Measured against the whole take instead, a 1.5 s hum with 3 s of lead-in reported **no root**, while the same hum with 2 s reported 220 Hz — a cliff edge on the most ordinary shape a take has. |

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

### Tempo (AC1, AC3, AC4)

`estimate_bpm` **moves here** from `gooz-model::features` (R-0015's one call
site imports it back, with its unit test). Two changes, not "unchanged":

- it returns **`Option<f64>`** rather than a `0.0` sentinel — and the sentinel
  was not even complete, since `estimate_bpm(&[0.0, f64::MIN_POSITIVE])`
  returned `inf`. `gooz-model` applies `.unwrap_or(0.0)` at its own format
  boundary, where `0.0` is genuinely part of the documented file format;
- the sort is `total_cmp` rather than `partial_cmp().unwrap_or(Equal)`.

The median is an **order statistic, lower middle for even counts** — never the
average of the two middle values, which invents a number the take never
contained (`0.4 s` and `0.8 s` averaged to `0.6 s`, a pulse matching neither).
The lower middle specifically, so a 50/50 long-short pattern does not flip its
answer between 150 BPM and nothing depending on how many notes were played.

Folding: a median IOI routinely lands on half or double the felt pulse, so the
raw estimate is doubled while below `MIN_BPM` and halved while above `MAX_BPM`,
up to a bounded number of steps. If it still does not land in range — or if
there were fewer than two onsets — the answer is `None`, not a number nobody
asked for. Range: `60..=180`.

### Root (AC2, AC3)

The median of the voiced frames' `f0_hz`. Voiced means `f0_hz.is_some()`; YIN
has already made that call (R-0005).

Two gates before the median counts as *heard*:
- at least `MIN_VOICED_FRAMES` voiced frames, and
- voiced frames are at least `MIN_VOICED_FRACTION` of all frames.

The second is what keeps a knock from reporting a key: percussive noise
produces a scattering of spurious voiced frames, and a median over three of
them is a number with no signal in it.

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
| 2026-09-21 | One `analyze` call, not `pitch_track` + `detect_onsets` separately | `analyze` already does both in one pass and is the reviewed entry point; calling the halves separately would double the work and duplicate its validation. |
| 2026-09-21 | A voiced *fraction* gate, not just a voiced *count* | Three spurious voiced frames in a snare hit would otherwise be enough to report a key, and a wrong key is worse than no key: the caller's fallback is at least a known quantity. |
| 2026-09-21 | `followed_grid` / `followed_tempo` are new (and private), rather than changing `easy_mode_*` | The demo wants the old meaning, and changing the existing functions under it would silently re-tune a fixed reference. (An earlier draft claimed R-0027's path was a second such caller; it is not — `describe.rs` uses `GRID_ROOT_HZ` directly and builds its own `Tempo`.) |
| 2026-09-21 | `demo_riff` calls `hum_to_riff` directly rather than going through `riff_from_take` | It used to be implemented in terms of it, which made "the demo does not follow" impossible: measured, the demo hum reports 126.4 BPM and 333 Hz, so it would have re-tuned itself and broken its own existing test. A golden test now pins the demo's ratio sequence, bar count and length. |

## Changelog

- 2026-09-21 — created; proposed for architect review.
