# R-0011 — Arrangement

- **Status:** Accepted
- **Milestone:** M3
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-02
- **Depends on:** R-0010 (session format)
- **Realized by:** SPEC-0011
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

A song must have an **arrangement**: how its stems are laid out in time. The
arrangement names **sections** as bar spans (intro, verse, hook…), an optional
**loop region** (a bar span to repeat), and per-stem **placements** — where each
stem starts, whether it is **muted**, and its **level**. The arrangement is part
of the saved session (R-0010) and round-trips losslessly. This turns a bag of
stems into a song laid out on a timeline.

It defines arrangement *structure and validation only*; turning the arrangement
into a rendered master is mixdown/export (**R-0012**).

## 2. Rationale

R-0010 gives a song its stems and takes, but nothing says *when* each stem plays
or how loud. Sections and a loop region are how a non-musician thinks about
structure ("this is the hook, loop these 4 bars"); mute and level are the
minimum mixing controls. Keeping this as pure, validated data on the `Song`
model means the future timeline UI (R-0013) and the mixdown (R-0012) both read
one authoritative arrangement, and it stays trivially round-trip-testable.

## 3. Acceptance criteria

- **AC1 — Sections as bar spans.** An arrangement holds ordered `Section`s, each
  a name + `start_bar` + `length_bars` (`length_bars ≥ 1`).
- **AC2 — Loop region.** An arrangement carries an optional loop region
  (`start_bar` + `length_bars ≥ 1`); `None` means "no loop".
- **AC3 — Stem placement.** Each placement references a stem, a `start_bar`, a
  `muted` flag, and a `level` in `[0, 1]`.
- **AC4 — Validation.** `validate` rejects a zero-length section or loop, a
  placement referencing a non-existent stem, or a level outside `[0, 1]`, with a
  typed `SessionError`; a valid arrangement passes.
- **AC5 — Round-trip.** A song with an arrangement serializes and loads back
  equal (extends R-0010's round-trip); a session saved before arrangement
  existed still loads (arrangement defaults to empty).
- **AC6 — Total length.** The arrangement reports its `total_bars` (the furthest
  section/placement/loop end, `0` when empty).
- **AC7 — Docs & gates.** Every public item is documented; the model is covered
  by tests; all four toolchain gates are green.

## 4. Constraints & non-goals

- Pure data + validation on `gooz-session`; no audio rendering (that is R-0012),
  no engine, no UI.
- **No mixdown/export** — the arrangement describes intent; producing audio from
  it is R-0012.
- Placements are **bar-aligned**; sub-bar offsets, fades/automation, and
  per-section stem changes are out of scope for v0.
- Levels are linear gain in `[0, 1]`; no dB, pan, or FX sends yet.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | Arrangement is a field on `Song` (`#[serde(default)]`) | One authoritative timeline in the session; the default keeps older R-0010 files loadable. |
| 2026-07-02 | Placements reference stems **by index** into `Song::stems` | Compact and unambiguous within a session; names can repeat, indices cannot. |
| 2026-07-02 | Mute + linear level `[0, 1]` only | The minimum mixing controls a non-musician needs; dB/pan/FX are later. |
| 2026-07-02 | Validation is an explicit `validate()` call, not enforced in setters | Keeps the model plain-data and `serde`-round-trippable; callers validate before mixdown/export. |

## Changelog

- 2026-07-02 — created, accepted for M3.
