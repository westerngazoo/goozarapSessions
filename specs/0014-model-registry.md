# SPEC-0014 — Model registry

- **Status:** Implemented
- **Realizes:** R-0014
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-02
- **Depends on:** SPEC-0010 (session format — conceptually; the registry roots
  under a session dir but takes a plain path, so no crate dependency)
- **Module(s):** `crates/gooz-model`

## 1. Motivation

Realizes R-0014: a pure, per-song model registry — the filesystem/metadata seam
every M4 influence-model feature builds on, with no ML dependency yet.

## 2. Design

```
crates/gooz-model/src/
├── error.rs     ModelError (Io / Serialize / Deserialize / NotFound / AlreadyExists / InvalidName)
├── registry.rs  ModelKind, ModelManifest, ModelHandle, ModelRegistry
└── lib.rs       crate docs + re-exports
```

Dependencies become `serde` + `serde_json` only (the registry is pure); the
prior `gooz-ratio`/`gooz-dsp` deps are dropped until a later requirement needs
them.

### Types (registry.rs)

```rust
pub const MODEL_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind { Timbre, Beat, Lyric }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelManifest {
    pub format_version: u32,
    pub id: String,
    pub name: String,
    pub kind: ModelKind,
    pub created_unix: u64,   // seconds since epoch (0 if the clock is unavailable)
    pub files: Vec<String>,  // artifact file names owned by this model
}

pub struct ModelHandle { /* id + dir; accessors id(), dir() */ }

pub struct ModelRegistry { root: PathBuf }
```

### Behaviour

- `ModelRegistry::open(root)` — `create_dir_all(root)`, store root.
- `id_from_name(name)` — lowercase, non-alphanumeric → `-`, collapse repeats,
  trim; empty → `ModelError::InvalidName`.
- `create(name, kind) -> ModelHandle`:
  - `id = id_from_name(name)`; if `root/id` exists → `AlreadyExists`;
  - `create_dir_all(root/id)`; write `manifest.json` with `created_unix` = now
    (`0` if the clock read fails); return the handle.
- `list() -> Vec<ModelManifest>` — read each subdir's `manifest.json`
  (skipping non-model dirs), sorted by id for determinism.
- `get(id) -> ModelManifest` — read `root/id/manifest.json` or `NotFound`.
- `dir(id) -> PathBuf` — `root/id` (no existence guarantee; pair with `get`).
- `manifest_add_file(id, file)` — load, push if absent, rewrite the manifest
  (how R-0015/R-0016 register the artifacts they drop into the dir).
- `remove(id)` — delete the model dir (`NotFound` if absent).

`ModelHandle` exposes `id()` and `dir()` so a caller can immediately write files
into the model directory, then record them via `manifest_add_file`.

### Errors (error.rs)

```rust
pub enum ModelError {
    Io(String), Serialize(String), Deserialize(String),
    NotFound(String), AlreadyExists(String), InvalidName(String),
}
```

`Display` + `std::error::Error`; `String`-backed for `Clone`/`Eq` in tests,
consistent with `SessionError` (SPEC-0010).

## 3. Non-goals

Feature extraction (R-0015), training/candle (R-0016), inference/timbre transfer
(R-0017), beat biasing (R-0018), Whisper, uuids, migrations.

## 4. Acceptance criteria

Maps to R-0014 AC1–AC7; qa owns `crates/gooz-model/tests/acceptance_r0014.rs`.

- [x] AC1 — `open` roots and creates the registry dir
- [x] AC2 — `create` makes a model dir + manifest, returns id + dir
- [x] AC3 — manifest is JSON with version/id/name/kind/created/files; round-trips
- [x] AC4 — `list` / `get` / `dir`
- [x] AC5 — `manifest_add_file` persists across reopen
- [x] AC6 — duplicate / missing / bad name → typed `ModelError`, no panic
- [x] AC7 — docs + four gates green

## 5. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | `list()` sorts by id | Deterministic ordering for callers and tests, independent of `read_dir` order. |
| 2026-07-02 | `created_unix` falls back to `0` if the clock read fails | Keeps `create` infallible on the clock; a manifest is still valid without a real timestamp. |
| 2026-07-02 | `manifest_add_file` is idempotent (no duplicate entries) | Re-registering the same artifact is a no-op, so training re-runs don't bloat the manifest. |

## Changelog

- 2026-07-02 — created; accepted alongside R-0014.
