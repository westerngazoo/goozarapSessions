# R-0010 — Session format

- **Status:** Accepted
- **Milestone:** M3
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-02
- **Depends on:** R-0008 (riff stems), R-0009 (beat stems), R-0004 (tempo settings)
- **Realized by:** SPEC-0010
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must have a **savable, loadable song session**: a single data model
that holds a song's **settings** (tempo, grid), its **takes** (recorded audio),
and its **stems** (rendered loopable parts — riffs and beats), plus a reference
to its future influence model. The session serializes losslessly to disk and
loads back to an identical model. This is what turns the Easy Mode loop from a
transient demo into a *song you can keep*.

It is the data foundation of M3. Arrangement (sections, loop regions — R-0011)
and mixdown/WAV export (R-0012) build on this model but are separate
requirements; this one defines the model and its save/load.

## 2. Rationale

Everything the studio produces today (a riff stem, a beat stem) vanishes when
the process exits. Before arrangement, export, or a UI session browser can
exist, there must be one authoritative, serializable description of a song.
Keeping it a plain, `serde`-derived model — no hidden state, no engine
dependency — means it is trivially testable (round-trip equality) and portable
(the same file opens on any machine). A `model_ref` field reserves the seam for
M4's per-song influence model without pulling any ML into this crate.

## 3. Acceptance criteria

- **AC1 — Lossless round-trip.** A `Song` (settings + takes + stems) serializes
  to text and deserializes back to an equal `Song` (`assert_eq!`).
- **AC2 — Save & load.** `Song::save(path)` writes a session file and
  `Song::load(path)` reads it back to an equal `Song`.
- **AC3 — Stems carry audio + kind.** A `Stem` holds its samples, sample rate,
  bar count, and a `StemKind` (riff / beat / other); a `Take` holds its captured
  samples and sample rate.
- **AC4 — Typed errors, no panics.** Loading a missing or corrupt file returns a
  typed `SessionError`; the crate never panics on library paths.
- **AC5 — Empty song.** A song with no takes and no stems round-trips and
  saves/loads.
- **AC6 — Deterministic serialization.** The same `Song` serializes to identical
  bytes on repeated calls.
- **AC7 — Docs & gates.** Every public item is documented; the model is covered
  by tests; all four toolchain gates are green.

## 4. Constraints & non-goals

- Pure data + serialization; depends only on `serde`/`serde_json` (no engine,
  no synth, no ML). `gooz-session` gains no audio-processing responsibility.
- **v0 embeds audio samples in the session file** (JSON). Efficient binary /
  per-stem WAV layout is **R-0012** (mixdown & export); a project *directory*
  layout is deferred with it.
- **No arrangement semantics** (sections, loop regions, mute/level) — that is
  **R-0011**. This model may carry an arrangement placeholder but defines no
  arrangement behaviour.
- No influence-model logic — only an opaque `model_ref` string reserved for M4.
- No UI (R-0013), no migration/versioning strategy beyond a `format_version`
  stamp.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | `Song` = `settings` + `takes` + `stems` + `model_ref`, all `serde`-derived | The minimal authoritative description of a song; derive keeps it pure and round-trip-testable. |
| 2026-07-02 | Serialize to **JSON** (`serde_json`), samples embedded, single file for v0 | Smallest thing that saves/loads and is human-inspectable; binary/WAV stems are R-0012. |
| 2026-07-02 | Session settings are a **self-contained struct** (bpm, beats/bar, root Hz, odd-limit), not `gooz_ratio::Tempo` | Keeps the on-disk format independent of a non-serializable engine type; the studio converts at the edges. |
| 2026-07-02 | A `format_version` field is stamped into every session | Cheap forward-compatibility hook before any real migration need. |

## Changelog

- 2026-07-02 — created, accepted for M3.
