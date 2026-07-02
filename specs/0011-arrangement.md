# SPEC-0011 — Arrangement

- **Status:** Implemented
- **Realizes:** R-0011
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-02
- **Depends on:** SPEC-0010 (session format)
- **Module(s):** `crates/gooz-session`

## 1. Motivation

Realizes R-0011: a pure, validated arrangement on the `Song` model — sections,
an optional loop region, and per-stem placements (mute + level) — that
round-trips with the session and feeds the mixdown (R-0012).

## 2. Design

New `arrangement` module in `gooz-session`:

```
crates/gooz-session/src/
├── error.rs        + SessionError::InvalidArrangement(String)
├── arrangement.rs  Section, LoopRegion, StemPlacement, Arrangement
├── model.rs        Song gains `arrangement: Arrangement` (#[serde(default)])
└── lib.rs          re-exports the new types
```

### Types (arrangement.rs)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Section { pub name: String, pub start_bar: u32, pub length_bars: u32 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoopRegion { pub start_bar: u32, pub length_bars: u32 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StemPlacement {
    pub stem: usize,     // index into Song::stems
    pub start_bar: u32,
    pub muted: bool,
    pub level: f32,      // linear gain in [0, 1]
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Arrangement {
    pub sections: Vec<Section>,
    pub loop_region: Option<LoopRegion>,
    pub placements: Vec<StemPlacement>,
}
```

### Behaviour

- `Section::end_bar` / `LoopRegion::end_bar` = `start_bar + length_bars`.
- `Arrangement::total_bars()` = max end across sections, placements' loop ends,
  and the loop region; `0` when empty. A placement's end needs the stem's bar
  count, so `total_bars(stems: &[Stem])` takes the song's stems.
- `Arrangement::validate(stem_count)`:
  - every `Section.length_bars ≥ 1`,
  - `loop_region.length_bars ≥ 1` when `Some`,
  - every `placement.stem < stem_count`,
  - every `placement.level` finite and in `[0, 1]`,
  - else `SessionError::InvalidArrangement(reason)`.
- Builders on `Song`: `with_section`, `with_loop`, `with_placement` (append /
  set), mirroring R-0010's `with_take`/`with_stem`.
- `Song::validate()` = `arrangement.validate(self.stems.len())`.

### Serialization

`Song.arrangement` is `#[serde(default)]`, so pre-arrangement R-0010 sessions
load with an empty arrangement (AC5). Struct field order keeps output
deterministic.

## 3. Non-goals

Mixdown/audio (R-0012), sub-bar placement, fades/automation, dB/pan/FX, UI.

## 4. Acceptance criteria

Maps to R-0011 AC1–AC7; qa owns `crates/gooz-session/tests/acceptance_r0011.rs`.

- [x] AC1 — sections are named bar spans, `length_bars ≥ 1`
- [x] AC2 — optional loop region
- [x] AC3 — placement: stem index + start_bar + muted + level
- [x] AC4 — `validate` rejects zero spans / bad stem index / out-of-range level
- [x] AC5 — round-trip with arrangement; older sessions default to empty
- [x] AC6 — `total_bars` reports the furthest end (0 when empty)
- [x] AC7 — docs + four gates green

## 5. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | `total_bars` takes `&[Stem]` | A placement's end depends on its stem's bar length; the arrangement stays index-based and needs the stems to resolve lengths. |
| 2026-07-02 | Validation reasons are `String` in `SessionError::InvalidArrangement` | Consistent with SPEC-0010's error style; keeps the error `Clone`/`Eq` for tests while naming what failed. |

## Changelog

- 2026-07-02 — created; accepted alongside R-0011.
