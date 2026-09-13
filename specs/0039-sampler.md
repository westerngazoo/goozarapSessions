# SPEC-0039 — Sampler

- **Status:** Accepted — implemented; architect review of the design in flight
- **Realizes:** R-0039
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-09-13
- **Depends on:** SPEC-0038 (`shift_pitch`), SPEC-0007 (`render_notes`, the
  layout and mixing this mirrors), SPEC-0006 (`QuantizedNote`), SPEC-0001 (`Ratio`)
- **Module(s):** `crates/gooz-synth` (`sampler.rs`)

## 1. Motivation

Realize R-0039: turn a recorded buffer into an instrument playable across the
ratio grid.

## 2. Design

```
recording ──┐
            ├─ for each note:  shift_pitch(recording, sr, degree ⊗ 2^octave)
notes ──────┘                        │
                                     └─▶ mix at onset_secs ─▶ normalize ─▶ distortion
```

The structure deliberately mirrors `render_notes` (R-0007): same onset
placement, same peak normalization, same FX tail. Only the voice differs — a
shifted copy of the recording instead of a plucked string. Readers who know one
can read the other.

### Types

```rust
/// A recording made playable across the ratio grid.
///
/// The recording's own pitch is the instrument's root: degree `1:1` is the
/// sound as recorded, and every other degree is that sound shifted by a ratio.
#[derive(Debug, Clone, PartialEq)]
pub struct Sampler {
    recording: Vec<f32>,
}

impl Sampler {
    pub fn new(recording: Vec<f32>) -> Sampler;
    pub fn recording(&self) -> &[f32];
}

pub fn render_sampled_notes(
    sampler: &Sampler,
    notes: &[QuantizedNote],
    sample_rate: u32,
    cfg: &RenderConfig,
) -> Vec<f32>;
```

`RenderConfig` is reused as-is. Its `decay` field is inert for a sampler — a
recording has its own decay — and that is stated in the function's docs rather
than papered over with a second config type.

**`Distortion::None` is added.** Both existing curves are non-linear, so every
renderer's final stage changed the signal unconditionally. For a sampler that is
wrong twice over: musically, a user who records their own sound is entitled to
hear it back as they played it; and structurally, **AC2 is not testable without
it** — "the source times a single gain factor" cannot hold through a `tanh`.
The variant is additive, nothing serializes `Distortion`, and the existing
curves are untouched.

### The shift ratio (AC1, AC2, AC4)

```rust
fn voice_ratio(note: &QuantizedNote) -> Result<Ratio, RatioError> {
    let mut ratio = note.degree;
    for _ in 0..note.octave.unsigned_abs() {
        ratio = if note.octave > 0 {
            ratio.stack(Ratio::OCTAVE)?     // ×2
        } else {
            ratio.unstack(Ratio::OCTAVE)?   // ÷2
        };
    }
    Ok(ratio)
}
```

Exact rational arithmetic end to end. `stack`/`unstack` already return
`RatioError` on overflow, so an absurd octave is typed, not a panic (**AC4**);
such a note is **skipped**, matching how `render_notes` skips a note with a
non-finite frequency rather than failing the whole render.

`note.freq_hz` is deliberately **unused**. Under R-0039's decision that the
recording is the root, an absolute frequency has no meaning for this instrument
— honouring it would require a float shift factor, which is exactly what
`shift_pitch`'s `Ratio` parameter exists to prevent.

### Rendering (AC5, AC6)

```rust
pub fn render_sampled_notes(...) -> Vec<f32> {
    if sample_rate == 0 || notes.is_empty() || sampler.recording.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<f32> = Vec::new();
    for note in notes {
        let Ok(ratio) = voice_ratio(note) else { continue };
        let Ok(voice) = shift_pitch(&sampler.recording, sample_rate, ratio) else { continue };
        let onset = (note.onset_secs * f64::from(sample_rate)).round().max(0.0) as usize;
        let end = onset + voice.len();
        if out.len() < end { out.resize(end, 0.0); }
        for (dst, src) in out[onset..end].iter_mut().zip(&voice) { *dst += src; }
    }
    normalize_peak(&mut out);
    for x in &mut out { *x = cfg.distortion.apply(*x, cfg.drive); }
    out
}
```

**Onsets are guarded.** `onset_secs` is caller data, and
`(1e18 · sample_rate).round() as usize` saturates to `usize::MAX`, whose
`resize` aborts the process — the same float-cast saturation the architect found
in `shift_pitch` and the same class again. A note whose onset is non-finite,
negative, or past `MAX_RENDER_SAMPLES` (ten minutes at 192 kHz) is skipped.

While writing this guard I confirmed `render_notes` (R-0007) has the identical
bug on `main` and panics today; it is filed as issue #71 rather than fixed here,
because a defect in another requirement deserves its own reviewable diff.

Total — no `Result` — for the same reason `render_notes` is: the honest product
answer to "this note cannot be rendered" is silence for that note, not a failed
song. `shift_pitch`'s errors (`OutputTooLong` on an extreme octave, and the
guard errors, which cannot fire here because the recording is checked non-empty
and the rate non-zero) fall into the same skip.

`normalize_peak` turned out to be **already duplicated byte-for-byte** between
`render.rs` and `beat.rs` — a pre-existing §2 violation. It moves to a private
`mix` module with three callers, so this requirement removes a duplication
rather than adding one.

## 3. Non-goals

No instrument trait, no picker (R-0031), no capture wiring, no session
persistence of the recording, no velocity layers or loop points, no gating to
`duration_secs`. See R-0039 §4.

## 4. Open questions

None.

## 5. Acceptance criteria mapping

- AC1 → render the same sample at `1:1` and `3:2`, measure both with YIN, assert
  a fifth apart (the shift is verified by *ear*, not by re-reading the ratio).
- AC2 → at `1:1`/octave 0 the placed segment equals the source times one gain
  factor: assert `out[i] / src[i]` is constant across the buffer.
- AC3 → a noise burst renders across every degree of a grid, non-empty and
  bounded, with no pitch detection anywhere in the path.
- AC4 → an octave up measures double and an octave down half (YIN); an octave
  large enough to overflow `stack` is skipped, and the render still returns.
- AC5 → empty recording / empty notes / zero rate each yield an empty buffer;
  bounds and finiteness sweep over a full-scale recording.
- AC6 → two identical calls are equal.
- AC7 → four gates + docs.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-13 | Add `Distortion::None` | Every renderer ends in a distortion stage, and both existing curves are non-linear — so nothing could play a recording back unchanged. That is wrong musically (the user's sound is the instrument) and it makes AC2 untestable. Additive; nothing serializes the enum. |
| 2026-09-13 | Guard the onset against float-cast saturation | Third instance of this class in the codebase (after `shift_pitch`'s `ceil`, and R-0007's, now issue #71). `onset_secs` is caller data and `as usize` saturates. |
| 2026-09-13 | Mirror `render_notes`' structure rather than generalize it | The two share onset placement, normalization, and FX, but a premature trait would freeze a seam before R-0031 shows what it must carry. `normalize_peak` is shared as a private helper — shared code, not shared abstraction. |
| 2026-09-13 | An unrenderable note is **skipped**, and the function stays total | Matches `render_notes`, and matches the honesty rule: a song with one silent note is a better answer than no song. |
| 2026-09-13 | `RenderConfig` is reused even though `decay` is inert | A second near-identical config type costs every caller a choice it does not care about; one documented inert field is cheaper and more honest than the duplication. |

## Changelog

- 2026-09-13 — created; proposed for architect review.
- 2026-09-13 — amended during implementation: `Distortion::None`, the onset guard, and `normalize_peak`'s pre-existing duplication.
