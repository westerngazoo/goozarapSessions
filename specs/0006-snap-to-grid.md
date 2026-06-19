# SPEC-0006 — Snap-to-grid (quantize notes onto the ratio grids)

- **Status:** Implemented — QA PASS, architect APPROVE
- **Realizes:** R-0006
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-06-18
- **Depends on:** SPEC-0001 (`PitchGrid`/`Ratio`), SPEC-0002 (`Tempo`), SPEC-0005 (`NoteEvent`)
- **Module(s):** `crates/gooz-dsp`

## 1. Motivation

Realizes R-0006: quantize R-0005's note events onto `gooz-ratio`'s frequency and
beat grids — pitch, onset, and duration all snapped — producing grid-locked
notes. The "make it sound right" step between analysis (R-0005) and rendering
(R-0007).

## 2. Design

One new module in `gooz-dsp`; re-adds the `gooz-ratio` dependency.

```
crates/gooz-dsp/Cargo.toml   + gooz-ratio.workspace = true   (the inward edge)
crates/gooz-dsp/src/
├── quantize.rs   QuantizedNote + quantize_notes()
└── lib.rs        re-export QuantizedNote, quantize_notes, and (so callers can name
                  the argument and result types from gooz-dsp alone)
                  `pub use gooz_ratio::{PitchGrid, Tempo, Ratio};` — enumerated, not a glob
```

### `QuantizedNote` (quantize.rs)

```rust
pub struct QuantizedNote {
    pub degree: gooz_ratio::Ratio, // grid degree the pitch snapped to (octave-reduced)
    pub octave: i32,               // octaves above the grid root
    pub freq_hz: f64,              // exact snapped grid frequency (SnappedPitch.hz, renamed for clarity)
    pub cents_offset: f64,         // original pitch − snapped, in cents (signed)
    pub onset_step: u64,           // snapped onset as a global beat-grid step index
    pub onset_secs: f64,           // snapped onset time = onset_step · step_secs
    pub duration_secs: f64,        // snapped duration = (end_step − onset_step) · step_secs (≥ 1 step)
}
```

`degree`/`octave`/`freq_hz`/`cents_offset` come straight from
`PitchGrid::snap`'s `SnappedPitch`; the timing fields are the beat-grid snap.

### `quantize_notes` (quantize.rs)

```rust
pub fn quantize_notes(
    notes: &[NoteEvent],
    pitch_grid: &gooz_ratio::PitchGrid,
    tempo: &gooz_ratio::Tempo,
    subdivision: u32,
) -> Vec<QuantizedNote>;
```

- Precondition: R-0005 note events have `onset_secs ≥ 0` and `duration_secs > 0`
  (a voiced note). The clamps below are nonetheless explicit so the arithmetic
  is total.
- `step_secs = tempo.seconds_per_beat() / subdivision.max(1) as f64` — the beat
  grid's step duration (the bar subdivided into `beats_per_bar · subdivision`
  steps; `t = 0` is the grid origin / downbeat, per R-0006 §4). A `subdivision`
  of `0` is treated as `1` (documented in the public API, not a silent surprise).
- For each `note` (in input order, already onset-sorted from R-0005):
  1. **pitch** — `pitch_grid.snap(note.pitch_hz as f64)`. A note whose pitch is
     non-finite/non-positive makes `snap` return `Err` → **skip** the note
     (cannot occur for a voiced R-0005 note; keeps the API total). On `Ok(sp)`
     take `sp.degree`, `sp.octave`, `sp.hz`, `sp.cents_offset`.
  2. **onset** — `onset_step = (note.onset_secs / step_secs).round().max(0.0)`
     as `u64`; `onset_secs = onset_step as f64 · step_secs` (`t = 0` → step 0).
  3. **duration** — `end_raw = ((note.onset_secs + note.duration_secs) /
     step_secs).round().max(0.0)` as `u64` (clamped the **same** way as the
     onset — not left unclamped); the snapped end is
     `end_step = max(end_raw, onset_step + 1)`, which guarantees the duration is
     **always ≥ 1 step**; `duration_secs = (end_step − onset_step) as f64 ·
     step_secs`. (The `as u64` casts saturate on absurd inputs rather than
     panicking; R-0005's second-scale times never approach that.)
- **Result invariant (AC5):** output length equals the number of input notes
  whose `pitch_hz` is finite and `> 0`; the relative onset order of the input is
  preserved; notes that snap to the **same** `onset_step` are all retained —
  there is no merging, dedup, or overlap resolution (Advanced Mode). Empty input
  → empty output. No allocation beyond the result `Vec`; no panic (the only
  fallible call, `snap`, is handled by skipping).

This reuses R-0001's `PitchGrid::snap` (nearest degree, correct octave,
bitwise-fixed grid frequency, cents offset) and R-0002's `Tempo`
(`seconds_per_beat`) — no new pitch or beat math is invented here.

## 3. Code outline

```rust
// quantize.rs
pub fn quantize_notes(
    notes: &[NoteEvent],
    pitch_grid: &PitchGrid,
    tempo: &Tempo,
    subdivision: u32,
) -> Vec<QuantizedNote> {
    let step_secs = tempo.seconds_per_beat() / subdivision.max(1) as f64;
    notes
        .iter()
        .filter_map(|note| {
            let sp = pitch_grid.snap(note.pitch_hz as f64).ok()?; // skip unsnappable
            let onset_step = (note.onset_secs / step_secs).round().max(0.0) as u64;
            let end_raw = ((note.onset_secs + note.duration_secs) / step_secs).round().max(0.0) as u64;
            let end_step = end_raw.max(onset_step + 1); // duration always >= 1 step
            Some(QuantizedNote {
                degree: sp.degree,
                octave: sp.octave,
                freq_hz: sp.hz,
                cents_offset: sp.cents_offset,
                onset_step,
                onset_secs: onset_step as f64 * step_secs,
                duration_secs: (end_step - onset_step) as f64 * step_secs,
            })
        })
        .collect()
}
```

## 4. Non-goals

- Key detection (caller supplies the `PitchGrid`) and tempo/phase detection
  (caller supplies the `Tempo`; `t = 0` is the downbeat).
- Rendering to audio — R-0007; the record→riff pipeline — R-0008.
- Tolerance/strength controls, humanize, swing — Advanced Mode.

## 5. Open questions

None — settled in the decision log.

## 6. Acceptance criteria

Maps to R-0006 AC1–AC7; qa owns `tests/acceptance_r0006.rs`.

- [x] AC1 — pitch → nearest grid degree, correct octave, exact grid frequency
      (446 Hz on a 220-rooted grid → 440 Hz).
- [x] AC2 — signed cents offset (sharp +, flat −, on-pitch ≈ 0).
- [x] AC3 — onset → nearest step (`round(onset/step_secs)`); `t = 0` → step 0.
- [x] AC4 — duration → whole steps, always ≥ 1 step.
- [x] AC5 — order/count preserved; bad-pitch note skipped; empty → empty.
- [x] AC6 — deterministic, inward-only dep, no panic.
- [x] AC7 — doc examples on public items; tests; four gates green.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-18 | `quantize_notes` returns `Vec<QuantizedNote>` (no `Result`) | For well-typed input there is no failure mode: `Tempo`/`PitchGrid` are validated by construction, `subdivision` is clamped, and an unsnappable pitch is skipped — so the API is total and panic-free without a spurious error type. |
| 2026-06-18 | Snap onsets to a **global** step grid (`round(onset/step_secs)`) | Snapping the absolute time to the nearest grid step across bars is exactly beat-grid quantization, and avoids per-bar wrap-around bookkeeping; `step_secs` derives from `gooz-ratio`'s `Tempo`. |
| 2026-06-18 | Embed the snapped-pitch fields (degree/octave/freq/cents) flat in `QuantizedNote` | Self-contained result; `degree` is a `gooz_ratio::Ratio` (re-exported) so callers can read the musical interval directly. The `_secs` fields are kept strictly derived from the `_step` fields to avoid drift. |
| 2026-06-18 | Global `f64::round` time-snap (ties away from zero) differs from `BarGrid::quantize`'s per-bar ties-to-earlier | They are different operations — global absolute-time snap vs within-bar phase snap — so the tie convention need not match; noted so a future reader doesn't expect agreement. |
| 2026-06-18 | Architect review (REQUEST CHANGES) applied: clamp `end_raw` like the onset; unify the ≥1-step rule as `max(end_raw, onset_step+1)`; pin the AC5 invariant (count = finite/positive-pitch notes, input order preserved, same-step collisions retained, no merge); enumerate the `{PitchGrid, Tempo, Ratio}` re-exports (no glob); document `subdivision == 0 ⇒ 1`; note the saturating casts | Findings 1–8 of the SPEC-0006 review — all clarity/precision, no redesign. |

## Changelog

- 2026-06-18 — created; accepted alongside R-0006.
