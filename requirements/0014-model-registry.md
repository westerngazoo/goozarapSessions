# R-0014 — Model registry

- **Status:** Accepted
- **Milestone:** M4
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-02
- **Depends on:** R-0010 (session format — models live inside a session)
- **Realized by:** SPEC-0014
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must have a **per-song/per-album model registry**: a place, *inside a
session's directory*, that holds the song's influence-model artifacts and knows
what they are. The registry creates and lists model directories, each with an
inspectable **manifest** (id, name, kind, creation time, files), and resolves a
model id to its directory so later stages (feature extraction R-0015, training
R-0016, inference R-0017/R-0018) can read and write their files there. A session
references its active model by id via `Song.model_ref`.

This is the **foundation of M4** — the filesystem/metadata seam every influence-
model feature builds on. It contains **no ML**: no candle, no training, no
inference — only the registry and its manifests.

## 2. Rationale

M4's premise is "a small, local model *per song*, living inside the session"
(`docs/ARCHITECTURE.md` §3). Before any feature extraction or training can land,
there must be one authoritative, inspectable way to create, find, and describe
those model directories — otherwise every later requirement invents its own
layout. Keeping the registry pure (filesystem + serde manifest, no ML deps)
makes it trivially testable and keeps `gooz-model`'s heavy candle dependency out
until training (R-0016) actually needs it. The manifest being human-readable
honours the project's "inspectable, no black box" stance.

## 3. Acceptance criteria

- **AC1 — Open a registry.** `ModelRegistry::open(root)` roots a registry at a
  directory (creating it if absent) — in practice `<session>/models/`.
- **AC2 — Create a model.** `create(name, kind)` makes a model subdirectory with
  a written `manifest.json` (id, name, kind, created time, empty file list) and
  returns a handle exposing the model's id and directory.
- **AC3 — Inspectable manifest.** The manifest is JSON with a `format_version`,
  the model's `id`, `name`, `kind`, creation time, and its tracked files; it
  round-trips losslessly.
- **AC4 — List & resolve.** `list()` returns every model's manifest; `get(id)`
  returns one; `dir(id)` resolves a model id to its directory path.
- **AC5 — Track files.** A model can record the files it owns (e.g. a weights or
  feature file) in its manifest, persisted across reopen.
- **AC6 — Typed errors, no panic.** A missing model, a duplicate id, or an I/O /
  parse failure returns a typed `ModelError`; the crate never panics on library
  paths.
- **AC7 — Docs & gates.** Every public item is documented; the registry is
  covered by tests; all four toolchain gates are green.

## 4. Constraints & non-goals

- Pure registry + manifest: depends only on `serde`/`serde_json` and `std::fs`.
  **No candle / ML dependency** enters `gooz-model` in this requirement.
- **No feature extraction** (R-0015), **no training** (R-0016), **no inference /
  timbre transfer** (R-0017), **no beat biasing** (R-0018), **no Whisper**.
- The registry stores *artifacts and metadata*; it does not interpret model
  contents. Weight/feature file formats are defined by the requirements that
  produce them.
- Model ids are session-local and derived from the name (sanitized); global
  uniqueness / uuids are out of scope.
- No migration strategy beyond a `format_version` stamp.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | The registry is **pure** (serde + `std::fs`), no candle | Keeps the heavy ML dependency out until training (R-0016) needs it; the registry stays fast and trivially testable. |
| 2026-07-02 | One **directory per model** with a `manifest.json` inside | Artifacts (weights, features) are files; a per-model dir + manifest is the natural, inspectable layout inside the session. |
| 2026-07-02 | Model **id = sanitized name**, duplicate → error | Session-local uniqueness without a uuid dependency; deterministic and human-readable. |
| 2026-07-02 | `ModelKind` = `Timbre` / `Beat` / `Lyric` (extensible) | The three influence-model families M4/M5 will train; an enum keeps callers honest and the manifest typed. |

## Changelog

- 2026-07-02 — created, accepted for M4.
