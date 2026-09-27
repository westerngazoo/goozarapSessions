# SPEC-0040 — Play my sound

- **Status:** Accepted — architect-reviewed after implementation (round 1: request changes, addressed)
- **Realizes:** R-0040
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-09-13
- **Depends on:** SPEC-0039 (`Sampler`), SPEC-0013 (studio shell), SPEC-0001 (grids)
- **Module(s):** `apps/gooz-studio` (`instrument.rs`), `apps/gooz-studio/src-tauri`,
  `apps/gooz-studio/ui`

## 1. Motivation

Realize R-0040: make R-0039's sampler reachable. A recorded take, a grid, and a
button.

## 2. Design

```
record_start ──▶ (existing capture) ──▶ record_stop_instrument
                                              │
                                    instrument_from_take
                                              │
        Sampler::new(take) ──┐                │
                             ├─ render_sampled_notes ──▶ RiffView ──▶ UI
     grid degrees → notes ───┘                              (existing paths)
```

### The library call (AC1–AC6)

```rust
pub fn instrument_from_take(
    samples: &[f32],
    sample_rate: u32,
    tense: u8,
) -> Result<RiffView, DspError>
```

1. **Validate, then wrap.** An empty take is `EmptySignal` and a zero rate is
   `InvalidSampleRate` — checked here, because `Sampler::new` deliberately
   accepts an empty recording (silence is a valid instrument, an empty *take* is
   a failed recording). `Sampler::new` then rejects a non-finite take.
2. **The figure is the grid.** `easy_mode_grid(tense).degrees()` in ascending
   ratio order, one note per degree, each at successive beats:
   `onset_secs = i · tempo.seconds_per_beat()`, `octave = 0`, and the sampler
   rooted at octave 0 so degree `1:1` is the take exactly as recorded.
3. **Render** through `render_sampled_notes` with `Distortion::Bypass` — this is
   the user's own sound, and saturating it by default would be answering a
   question they did not ask.
4. **View.** Build a `RiffView` directly: the note cards are the degrees (so the
   scale is what the user sees, AC3), `bars` covers the figure, and `wave` is the
   same `peak_envelope` the other views use.

`easy_mode_grid`, `easy_mode_tempo`, and `odd_limit_for` become `pub(crate)`;
they are the existing definition of "Easy Mode", and a second copy would let the
two modes drift onto different grids.

### From take to instrument — what the first draft missed

A take is a **capture window**, not a sound. Three steps turn one into the
other, in `the_sound`:

1. **Cut to the sound** with `gooz_dsp::sound_span`: the first sample within
   −20 dB of the peak (minus a 5 ms pre-roll so the attack survives) to the last
   within −30 dB (so a decay tail survives but trailing hiss does not).
2. **Cap at one beat.** Every degree in the figure is at or above `1:1`, so the
   longest hit is the sound itself; capping the sound caps every hit, and a held
   hum climbs instead of stacking.
3. **Fade both cuts** — 2 ms in, 10 ms out — so neither end clicks.

Before any of that, the *whole* take is checked: a NaN or an out-of-range
sample is a typed error even if it sits in silence that would be cut away. A
take with nothing above −50 dBFS is `DspError::Silent`.

The rendered figure is **padded to whole bars** with the same rule the hum
pipeline uses (`pipeline::pad_to_bars`, now shared). Unpadded, the loop drifted
against the beat from the first repeat, and `mixdown` — which wraps each stem
by its own length — replayed its start mid-bar in every export.

Cards come from one mapping (`From<&QuantizedNote> for NoteView`) for sung
notes, and `sampled_card` for this path: **ratio only**, `hz` and `cents` are
`None`, because nothing measured the recording's pitch.

### The shell (AC7)

A `record_stop_instrument(tense)` command beside the existing
`record_stop_analyze(tense)`, sharing `record_start` and the same recorder
state. One button records; two commands are two ways to hear it back.

### The UI (AC7)

The result's heading travels **with the result** (`showResult(data, heading)`),
captured when recording starts — not read from whichever mode pill is lit when
the result arrives. The pills are disabled while anything is in flight.

In *mi instrumento* there is **no demo link and no demo fallback**: the hum demo
is a guitar built from a synthetic hum, and showing it under "tu instrumento"
after a failed take would be showing someone else's sound. A failed take shows
its typed error on the intro screen, where the user can simply record again.
Hum mode keeps R-0013's graceful fallback.


A mode toggle — *tararear* (today's hum→riff) and *mi instrumento* (this) —
in the top bar, on screen with every result. Picking a mode from a result
discards it and returns to the mic in that mode; the result's own exit,
"← volver", returns to the mic in the same one. Everything downstream is
untouched: the same waveform canvas, note cards, play loop, save session,
export WAV.

## 3. Non-goals

Note input (R-0036), instrument picker (R-0031), scale selection (R-0037),
session persistence of the recording (needs R-0010 extended). See R-0040 §4.

## 4. Open questions

None.

## 5. Acceptance criteria mapping

- AC1 → a take renders to a non-empty riff whose note count equals the grid's
  degree count, with onsets one beat apart.
- AC2 → a pitched hum and an unpitched noise burst both return a populated riff;
  no pitch tracking appears anywhere in the path.
- AC3 → the view's notes are exactly the grid's degrees, in the same order.
- AC4 → a tenser setting yields at least as many notes, and its degree set is a
  superset of the smoother one's.
- AC5 → empty / zero-rate / non-finite each return the matching typed error.
- AC6 → two identical calls are equal; bounds and finiteness sweep.
- AC7 → the command compiles in the desktop-shell CI job; the UI calls it.
- AC8 → four gates + docs.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-27 | The mode toggle moves to the top bar and doubles as the way back from a result (owner decision) | A result hid the toggle and offered only "↺ redo", which the owner did not read as a way back. See R-0040's decision log. |
| 2026-09-13 | `Distortion::Bypass`, not the guitar's default drive | R-0039 added the bypass curve precisely so a recording can come back as it was played. Defaulting to saturation here would undo that on the one path where the user's own sound is the point. |
| 2026-09-13 | The empty-take check lives in this layer, not in `Sampler` | They are different questions: an empty *instrument* is silence and legal (R-0039 AC5); an empty *take* means the recording failed and the user needs to know. |
| 2026-09-13 | `easy_mode_grid` / `easy_mode_tempo` are shared, not copied | Two modes reading two definitions of Easy Mode would drift, and the slider would stop meaning one thing. |

## Changelog

- 2026-09-13 — created; proposed for architect review.
- 2026-09-26 — architect review round 1: take trimming, one-beat cap, silence as an error, bar padding, honest cards, UI error path. The review came after implementation; that ordering is noted here rather than hidden.
- 2026-09-27 — the mode toggle moves to the top bar; a mode picked from a result goes back to the mic; "↺ redo" becomes "← volver" (owner decision).
