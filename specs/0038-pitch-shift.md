# SPEC-0038 — Pitch shift (ratio-native)

- **Status:** Accepted — architect-reviewed (round 1: request changes, addressed)
- **Realizes:** R-0038
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-09-10
- **Depends on:** SPEC-0001 (`Ratio`), SPEC-0005 (YIN, for verification)
- **Module(s):** `crates/gooz-dsp` (`shift.rs`)

## 1. Motivation

Realize R-0038: move a recorded buffer to another pitch by a `Ratio`, so R-0039
can turn any recording into an instrument spanning the grid.

## 2. Design

### Why varispeed, stated plainly

Playing a buffer back at rate `r` multiplies its pitch by `r` and divides its
length by `r`. That is the classic sampler / tape behaviour, and for a one-shot
sample — the case that motivated this requirement — it is not a compromise, it
is the desired sound.

A phase vocoder would hold length constant, but it costs an STFT pipeline,
introduces smearing artifacts on transients (exactly what a percussive sample
is), and buys nothing for a one-shot. It becomes its own requirement when a
whole riff must be transposed without changing tempo.

### API

```rust
/// Shifts `signal` by `ratio`, resampling it. Pitch scales by the ratio;
/// length scales by its inverse.
pub fn shift_pitch(
    signal: &[f32],
    sample_rate: u32,
    ratio: Ratio,
) -> Result<Vec<f32>, DspError>;
```

`Ratio` is already reduced and positive by construction (R-0001), so the shift
factor cannot be zero, negative, or NaN — the type removes a whole error class
before it reaches the DSP.

### Method

Reading the input at a step of `ratio` (as `f64`) and writing one output sample
per step:

```
out_len = ceil(signal.len() / ratio)
out[i]  = lerp(signal, i * ratio)
```

with **linear interpolation** between the two neighbouring input samples, and
the last sample held at the boundary. Linear interpolation is the honest v0
choice: it is exact at integer positions (so `1:1` is bit-identical, **AC2**),
deterministic, and its own error is a gentle high-frequency roll-off.

The output length is `ceil(len · den / num)`, computed **in integer
arithmetic**. The float form `ceil(len / (num/den))` overshoots for some ratios
— `5 / (1/49)` evaluates to `245.00000000000003` — and appends a duplicated
final sample.

**Aliasing (limitation, not a claim of absence).** An upward shift is
decimation, and there is no pre-filter, so content above `SR / (2·ratio)` folds
back down: a 15 kHz tone shifted `2:1` at 48 kHz returns at 18 kHz. **AC1**
therefore holds while `f0 · ratio < SR/2`. This is classic sampler behaviour and
acceptable for R-0039's one-shots; the interpolation roll-off argument above is
about interpolation error and does **not** address decimation. A decimation
pre-filter is a later requirement, and a test pins today's behaviour so the
change is deliberate rather than silent.

Bounds: `|lerp(a, b)| <= max(|a|, |b|)`, so an input inside `[-1, 1]` cannot
leave it (**AC5**) — no clamping needed, and none is applied so the caller keeps
its headroom information.

### Validation

Calls the crate-wide guard (`validate::input`), shared with the analysis entry
points so the same bad input reports the same error through every door: empty
signal → `EmptySignal`, zero rate → `InvalidSampleRate`, any non-finite sample →
`NonFiniteSample` (**AC4**).

One **new error variant** is required. `Ratio` guarantees positive and non-zero
but not *bounded*, and varispeed multiplies length: `1:u64::MAX` overflows the
length arithmetic into a capacity-overflow abort, and `1:10_000` asks for nearly
three hours of audio from one second. Both are now
`DspError::OutputTooLong`, bounded by `MAX_OUTPUT_SECS` (600 s) at the given
sample rate. This is what makes `sample_rate` load-bearing: the resampling is
dimensionless, but "too long" is a duration.

## 3. Code outline

```rust
pub fn shift_pitch(signal: &[f32], sample_rate: u32, ratio: Ratio) -> Result<Vec<f32>, DspError> {
    validate(signal, sample_rate)?;
    let step = ratio.num() as f64 / ratio.den() as f64;
    if step == 1.0 {
        return Ok(signal.to_vec()); // exact identity, not merely close
    }
    let out_len = (signal.len() as f64 / step).ceil() as usize;
    Ok((0..out_len)
        .map(|i| sample_at(signal, i as f64 * step))
        .collect())
}

/// Linearly interpolates the signal at a fractional index, holding the edges.
fn sample_at(signal: &[f32], pos: f64) -> f32 { /* floor, frac, lerp */ }
```

## 4. Non-goals

- No time-stretch (length is *expected* to change), no formant correction, no
  polyphony, no real-time/streaming variant.
- Not called from the audio callback — offline only.

## 5. Open questions

None.

## 6. Acceptance criteria mapping

- AC1 → golden-signal test: a 220 Hz sine shifted by `3:2` measured back with
  `pitch_track` reads ~330 Hz within tolerance.
- AC2 → `shift_pitch(x, sr, 1:1) == x` exactly.
- AC3 → up and down tested; `3:2` then `2:3` recovers the original pitch.
- AC4 → empty / zero-rate / NaN each return the matching `DspError`.
- AC5 → determinism + finite + bounded sweep.
- AC6 → four gates, docs with a runnable example.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-09-10 | Varispeed resampling, not a phase vocoder | Right for one-shot samples (R-0039), artifact-free on transients, deterministic, and honestly testable. Length-preserving shift is a separate requirement when a use case demands it. |
| 2026-09-13 | Upward shifts **alias**; documented and pinned by a test rather than filtered | A decimation pre-filter is cheap but is its own requirement; what was not acceptable was the previous text claiming the operation had no aliasing, which measurement disproved (15 kHz × `2:1` returns at 18 kHz). |
| 2026-09-13 | Output length is **integer** arithmetic, and an implausible length is a typed error | Float `ceil` appended a duplicated sample for some ratios, and an unbounded `Ratio` could abort the process on a capacity overflow — a panic in library code, which CLAUDE.md §6 forbids. |
| 2026-09-10 | Linear interpolation for v0 | Exact at integer positions (so `1:1` is bit-identical), deterministic, and its error is a gentle roll-off rather than audible aliasing. Higher-order interpolation is a drop-in upgrade behind the same signature. |
| 2026-09-10 | Take a `Ratio`, not a float or semitones | The type guarantees positive and non-zero, removing an error class; and it keeps the engine speaking one language. |
| 2026-09-10 | No output clamping | `|lerp(a,b)| <= max(|a|,|b|)`, so bounded input stays bounded; clamping would silently destroy the caller's headroom information. |

## Changelog

- 2026-09-10 — created; proposed for architect review.
