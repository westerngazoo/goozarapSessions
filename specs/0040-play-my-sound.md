# SPEC-0040 — Play my sound

- **Status:** Proposed — architect review pending
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

### The shell (AC7)

A `record_stop_instrument(tense)` command beside the existing
`record_stop_analyze(tense)`, sharing `record_start` and the same recorder
state. One button records; two commands are two ways to hear it back.

### The UI (AC7)

A mode toggle next to the record button — *tararear* (today's hum→riff) and
*mi instrumento* (this). Everything downstream is untouched: the same waveform
canvas, note cards, play loop, save session, export WAV.

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
| 2026-09-13 | `Distortion::Bypass`, not the guitar's default drive | R-0039 added the bypass curve precisely so a recording can come back as it was played. Defaulting to saturation here would undo that on the one path where the user's own sound is the point. |
| 2026-09-13 | The empty-take check lives in this layer, not in `Sampler` | They are different questions: an empty *instrument* is silence and legal (R-0039 AC5); an empty *take* means the recording failed and the user needs to know. |
| 2026-09-13 | `easy_mode_grid` / `easy_mode_tempo` are shared, not copied | Two modes reading two definitions of Easy Mode would drift, and the slider would stop meaning one thing. |

## Changelog

- 2026-09-13 — created; proposed for architect review.
