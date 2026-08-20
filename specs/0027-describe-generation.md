# SPEC-0027 — Description-conditioned generation

- **Status:** Proposed — architect review pending
- **Realizes:** R-0027
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-04
- **Depends on:** SPEC-0025 (`MusicalIntent`), SPEC-0026 (`SoundPlan`), SPEC-0009
  (beat builder), SPEC-0007 (render), SPEC-0006 (`QuantizedNote`)
- **Module(s):** `apps/gooz-studio` (`describe.rs`)

## 1. Motivation

Realize R-0027: make a `SoundPlan` audible — beat plus a generated melody — and
expose one call that takes a text description all the way to sound.

## 2. Design

Generation is **integration**, so it lives in `apps/gooz-studio`: the only crate
allowed to depend on `gooz-model` (the plan), `gooz-dsp` (notes/grids), and
`gooz-synth` (render) together. No lower crate learns about descriptions.

```
prompt ──parse_intent──▶ MusicalIntent ──plan_sound──▶ SoundPlan
                                                          │
              ┌───────────────────────────────────────────┴──────────┐
              ▼                                                      ▼
   beat_config_from(plan)                                   melody_notes(plan)
   → BeatConfig → build_beat (R-0009)                       → Vec<QuantizedNote>
              │                                              → render_notes (R-0007)
              ▼                                                      ▼
          BeatView                                              RiffView
```

### Types (`describe.rs`)

```rust
/// A description turned into sound: what was understood, and what it sounds like.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DescribedSong {
    pub plan: SoundPlan,   // inspectable: what the words became (R-0026)
    pub riff: RiffView,    // the generated melody, rendered
    pub beat: BeatView,    // the generated beat, rendered
}
```

Returning the plan alongside the audio mirrors R-0008's "return what it heard":
the UI (R-0029) shows *what it understood* next to *what it made*, and the user
can edit the plan and re-render.

### Plan → beat (AC1)

```rust
fn beat_config_from(plan: &SoundPlan, bars: u32) -> BeatConfig
```

Mechanical: `VoiceRole::{Kick,Snare,Hat} → DrumKind::{Kick,Snare,HiHat}`, and
`VoicePlan{onsets, steps, rotate, level} → BeatVoiceSpec{..}` unchanged. The
existing `build_beat` (R-0009) then renders it. **Degradation note:** the beat
clock is 4/4 today (R-0035 pending), so a 6/8 plan's steps are laid across the
bar as-is — the pattern is right, the bar length is not yet.

### Plan → melody (AC2, AC3)

```rust
fn melody_notes(plan: &SoundPlan, bars: u32) -> Vec<QuantizedNote>
```

1. **Grid.** `PitchGrid::harmonic(ROOT_HZ, plan.odd_limit)` — `tension` already
   chose the odd-limit (R-0026), so a tenser description literally gets a grid
   containing more complex ratios.
2. **Reachable degrees.** Sort the grid's degrees by `Ratio::complexity()`
   ascending and keep a prefix whose length grows with the plan's density-free
   tension proxy: `window = 1 + round(tension_frac · (len − 1))`, where
   `tension_frac` is recovered from the odd-limit's position in `3..=15`. The
   melody may only use degrees inside the window, so **tension deepens the
   reachable ratio set and never shrinks it** (AC3).
3. **Rhythm.** `Pattern::euclidean(k, steps)` with `steps = plan.steps()` and
   `k = clamp(round(MELODY_MIN + (MELODY_MAX − MELODY_MIN) · density) · steps)`
   — a melody is sparser than a hat lane, so its fractions are lower.
4. **Contour.** Walk the window deterministically: the *i*-th onset takes degree
   `window[(i · STRIDE) % window.len()]` with `STRIDE = 3` (a coprime-ish stride
   gives leaps rather than a scale run), and the octave lifts by one every
   `OCTAVE_EVERY` onsets so the line has shape. Fully deterministic (AC5).
5. **Timing.** `step_secs = bar_secs / steps` from the plan's tempo/meter;
   `onset_secs = step · step_secs`; `duration_secs = step_secs` (one step,
   let-ring is the renderer's job).

Emitted as `QuantizedNote`s so the existing R-0007 renderer consumes them
unchanged — generation reuses the hum→riff back half rather than duplicating it.

### Description → song (AC4)

```rust
pub fn describe_song(prompt: &str, bars: u32) -> DescribedSong
pub fn song_from_plan(plan: &SoundPlan, bars: u32) -> DescribedSong
```

`describe_song` = `parse_intent` → `plan_sound` → `song_from_plan`. Total: no
`Result`, because the plan is already validated (R-0026 AC5) and every stage
below is total or infallible for a valid plan (AC6).

## 3. Code outline

```rust
pub fn song_from_plan(plan: &SoundPlan, bars: u32) -> DescribedSong {
    let bars = bars.max(1);
    let beat_stem = build_beat(&tempo_of(plan), SAMPLE_RATE, &beat_config_from(plan, bars))
        .unwrap_or_else(|_| BeatStem::empty(SAMPLE_RATE)); // plan is pre-validated
    let notes = melody_notes(plan, bars);
    let audio = render_notes(&notes, SAMPLE_RATE, &RenderConfig {
        drive: 1.0 + plan.drive * DRIVE_RANGE, ..RenderConfig::default()
    });
    DescribedSong { plan: plan.clone(), riff: riff_view(audio, notes, bars), beat: beat_view(beat_stem) }
}
```

## 4. Non-goals

- No influence-model biasing (R-0018), no UI (R-0029), no per-instrument prompts
  (R-0032), no 808/FX/6-8 engine work (R-0033/34/35).
- Not a composition engine: the contour is a deliberate v0 to be tuned by ear.

## 5. Open questions

None.

## 6. Acceptance criteria mapping

- AC1 → `beat_config_from` + a test that role/k/n/rotation/level survive and the
  stem is bar-aligned and bounded.
- AC2 → `melody_notes` test: every pitch is a grid degree; onsets land on grid steps.
- AC3 → monotonicity tests: density↑ ⇒ note count never falls; tension↑ ⇒ the set
  of distinct degrees never shrinks; drive reaches `RenderConfig`.
- AC4 → `describe_song("…")` returns a populated song.
- AC5 → equality of two identical calls.
- AC6 → bounds/finiteness sweep + an empty-description test.
- AC7 → four gates + docs.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-04 | Melody emitted as `QuantizedNote`s and rendered by R-0007 | Reuses the hum→riff back half; generation differs only in where the notes come from. |
| 2026-07-04 | Tension recovered from the plan's odd-limit rather than re-passed | `SoundPlan` is the single source of truth for what to play; adding a parallel tension field would let the two disagree. |
| 2026-07-04 | A coprime-ish stride over complexity-ordered degrees for the contour | Deterministic, gives leaps instead of a scale run, and keeps "tense = more complex ratios" literally true. |
| 2026-07-04 | Total functions (no `Result`) | The plan is pre-validated (R-0026 AC5); the honesty rule wants an unrecognized description to still play. |

## Changelog

- 2026-07-04 — created; proposed for architect review.
