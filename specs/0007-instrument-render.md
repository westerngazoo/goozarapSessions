# SPEC-0007 — Instrument render v0 (Karplus-Strong guitar + distortion)

- **Status:** Accepted
- **Realizes:** R-0007
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-06-19
- **Depends on:** SPEC-0006 (`QuantizedNote`)
- **Module(s):** `crates/gooz-synth`

## 1. Motivation

Realizes R-0007: render R-0006's quantized notes into guitar audio — a
Karplus-Strong plucked string per note, mixed and run through a selectable
distortion. The output stage of voice-to-instrument.

## 2. Design

`gooz-synth` comes to life with three small modules; depends on `gooz-dsp`
(for `QuantizedNote`).

```
crates/gooz-synth/Cargo.toml   gooz-dsp.workspace = true   (inward edge; already
                                present from scaffold — drop the unused gooz-ratio dep)
crates/gooz-synth/src/
├── string.rs      KarplusString — the plucked-string voice (private)
├── distortion.rs  Distortion enum + apply()
├── render.rs      RenderConfig + render_notes()
└── lib.rs         re-export Distortion, RenderConfig, render_notes,
                  and `pub use gooz_dsp::{QuantizedNote, Ratio};` — without these
                  an external caller cannot construct `render_notes`'s input
```

### `KarplusString` (string.rs, private)

The classic plucked-string delay line.

```rust
struct KarplusString { buf: Vec<f32>, pos: usize, decay: f32 }
```

- `pluck(freq_hz: f64, sample_rate: u32, decay: f32, seed: u64) -> KarplusString`:
  delay length `n = round(sample_rate / freq).max(2)`; fill `buf` with `n`
  samples of deterministic noise in `[-1, 1]` from a fixed-seed LCG (no real
  RNG). The fundamental is `sample_rate / n` (integer-delay tuning: exact at the
  low end, with a small error that grows with frequency — see AC1's band).
  - **Pinned LCG** (so renders are reproducible and golden-able): state starts at
    `seed`, advances `state = state·6364136223846793005 + 1442695040888963407`
    (wrapping), and each sample is `((state >> 40) as f32 / 2^24) · 2 − 1` ∈
    `[-1, 1)`. The per-note `seed = SEED ^ index`, `SEED = 0x9E37_79B9_7F4A_7C15`.
- `next_sample(&mut self) -> f32`: `out = buf[pos]`; write back
  `decay · 0.5 · (buf[pos] + buf[(pos + 1) % n])`; advance `pos`; return `out`.
  The averaging low-pass + `decay (< 1)` make the tone ring down naturally.
- Tail length (let-ring): the envelope after `k` samples ≈ `decay^(k/n)`, so it
  falls to a small `EPS` (1e-3) at `tail = ceil(n · ln(EPS)/ln(decay))` samples,
  clamped to `[n, MAX_TAIL]` (`MAX_TAIL = 5 · sample_rate`). Each note is
  rendered for `tail` samples (its natural decay), **independent of the grid
  duration** — that is what "let ring" means.

### `Distortion` (distortion.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Distortion { SoftClip, HardClip }

impl Distortion {
    pub fn apply(self, x: f32, drive: f32) -> f32 {
        let d = drive.max(1e-3);
        match self {
            Distortion::SoftClip => (d * x).tanh() / d.tanh(), // warm; →x as d→0
            Distortion::HardClip => (d * x).clamp(-1.0, 1.0),  // aggressive
        }
    }
}
```

For input in `[-1, 1]`, both keep output in `[-1, 1]` (`tanh(d·x)/tanh(d) ≤ 1`
for `|x| ≤ 1`; clamp is bounded by construction). A small `drive` → ~clean.

### `RenderConfig` + `render_notes` (render.rs)

```rust
pub struct RenderConfig { pub decay: f32, pub distortion: Distortion, pub drive: f32 }
// Default { decay: 0.996, distortion: SoftClip, drive: 2.0 }

pub fn render_notes(notes: &[QuantizedNote], sample_rate: u32, cfg: &RenderConfig) -> Vec<f32>;
```

Pipeline:

1. Guard: `sample_rate == 0` or `notes` empty → return `Vec::new()`. Likewise, a
   non-empty `notes` slice in which *every* note is skipped (step 2) yields an
   empty buffer.
2. For each note (index `i`), skip if `freq_hz` is non-finite or `≤ 0`.
   `onset_sample = round(onset_secs · sample_rate).max(0)`. Pluck a
   `KarplusString` (`cfg.decay`, seed = `SEED ^ i`), generate its `tail`
   samples, and **mix** (`+=`) them into the output buffer starting at
   `onset_sample`, growing the buffer as needed. Tails of earlier notes sum with
   later notes (let-ring; AC4).
3. Normalize the mixed buffer so its peak magnitude is `≤ 1` (two-pass; skip if
   already silent), so the distortion sees a full-scale `[-1, 1]` signal.
4. Apply `cfg.distortion.apply(x, cfg.drive)` to every sample. Output is bounded
   in `[-1, 1]` (step 3 guarantees the input range).
5. Return the buffer (no NaN/inf — inputs are finite and all ops are finite).

`decay` is clamped to a safe `(0, 1)` range (e.g. `0.5..=0.99999`) so the tail
formula is well-defined and the string always decays.

## 3. Code outline

```rust
// distortion.rs apply() — see §2.

// render.rs
pub fn render_notes(notes: &[QuantizedNote], sample_rate: u32, cfg: &RenderConfig) -> Vec<f32> {
    if sample_rate == 0 || notes.is_empty() { return Vec::new(); }
    let decay = cfg.decay.clamp(0.5, 0.999_99);
    let mut out: Vec<f32> = Vec::new();
    for (i, note) in notes.iter().enumerate() {
        if !note.freq_hz.is_finite() || note.freq_hz <= 0.0 { continue; }
        let onset = (note.onset_secs * sample_rate as f64).round().max(0.0) as usize;
        let mut voice = KarplusString::pluck(note.freq_hz, sample_rate, decay, SEED ^ i as u64);
        let tail = voice.tail_len();
        if out.len() < onset + tail { out.resize(onset + tail, 0.0); }
        for s in 0..tail { out[onset + s] += voice.next_sample(); }
    }
    normalize_peak(&mut out, 1.0);
    for x in &mut out { *x = cfg.distortion.apply(*x, cfg.drive); }
    out
}
```

## 4. Non-goals

- Other instruments (bass/drums/FM/sampler), other FX (delay/reverb).
- Engine wiring / playback — R-0008.
- Fractional-delay tuning correction; per-note dynamics from cents. v0 accepts
  integer-delay tuning error above ~1 kHz (AC1's band).
- Real RNG.
- **Waveshapers live in `gooz-synth` for v0.** `docs/ARCHITECTURE.md` §3 intends
  distortion curves to live in `gooz-dsp`; with a single consumer and two curves
  that would be premature abstraction (CLAUDE.md §2). Promotion to a `gooz-dsp`
  waveshaping module is deferred until a second consumer or richer FX arrives.
- Inputs are assumed to come from `quantize_notes` (bounded onset times); no
  upper clamp on `onset_sample` in v0.

## 5. Open questions

None — settled in the decision log.

## 6. Acceptance criteria

Maps to R-0007 AC1–AC7; qa owns `tests/acceptance_r0007.rs`.

- [ ] AC1 — single note in the integer-tuned band (`n ≳ 48`, i.e. `f ≲ sr/48 ≈
      1 kHz at 48 kHz; test at 440 Hz): autocorrelation period ≈ `sample_rate /
      f` within ~1 %. (Above that band the integer-delay error grows past 1 % —
      a documented v0 limitation, §4; do not test there.)
- [ ] AC2 — energy in a late window < an early window (plucked decay).
- [ ] AC3 — silence before the first onset; non-zero from the onset.
- [ ] AC4 — two notes sum; first note rings past `onset+duration`; length spans
      last onset + tail.
- [ ] AC5 — SoftClip/HardClip each alter the signal; higher drive saturates more;
      output bounded in `[-1, 1]`; **SoftClip at low drive ≈ identity** and
      **HardClip at drive = 1.0 ≈ identity** (HardClip at low drive is near-
      silent, by design — not "clean").
- [ ] AC6 — deterministic (re-render equality on the same notes+config — not a
      frozen byte buffer); empty/zero-rate/all-skipped → empty; bad-freq skipped;
      no NaN; no panic.
- [ ] AC7 — doc examples on public items; tests; four gates green.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-19 | Tail length computed analytically (`ceil(n·ln(EPS)/ln(decay))`, clamped) | Deterministic, bounded "let ring" without a runtime peak-tracking loop. |
| 2026-06-19 | Normalize to full scale *before* distortion | The drive control is then meaningful (acts on a `[-1, 1]` signal) and the output is guaranteed bounded. |
| 2026-06-19 | `KarplusString` is private; tests go through `render_notes` | Smaller public surface; AC1/AC2 are observable on the rendered buffer (autocorrelation / windowed energy). `Distortion::apply` is public for direct AC5 unit tests. |
| 2026-06-19 | Per-note seed = `SEED ^ index` | Deterministic yet varied plucks (each string's noise differs), so repeated notes aren't bit-identical drones. |
| 2026-06-19 | Architect review (REQUEST CHANGES) applied: scope AC1 to the integer-tuned band (`n ≳ 48`, ≲ ~1 kHz; test at 440 Hz); scope AC5 "≈ clean" to SoftClip-low-drive and HardClip-at-drive-1.0 (HardClip low drive is near-silent by design); pin the LCG constants + `SEED`/`EPS`/`MAX_TAIL`; document all-skipped → empty; note waveshapers stay in `gooz-synth` for v0; determinism tested by re-render equality, not frozen bytes | Findings 1–5 + the architecture note from the SPEC-0007 review. Findings 1–2 were AC-vs-design consistency (would have made the red tests assert a correct implementation broken). |

## Changelog

- 2026-06-19 — created; accepted alongside R-0007.
