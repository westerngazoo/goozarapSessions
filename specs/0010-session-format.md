# SPEC-0010 — Session format

- **Status:** Implemented
- **Realizes:** R-0010
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-02
- **Depends on:** none at runtime (pure data); conceptually R-0008/R-0009 stems
- **Module(s):** `crates/gooz-session`

## 1. Motivation

Realizes R-0010: a pure, `serde`-derived song model that saves and loads
losslessly — the data foundation for arrangement (R-0011) and export (R-0012).

## 2. Design

```
crates/gooz-session/src/
├── error.rs    SessionError (Io / Serialize / Deserialize)
├── model.rs    Settings, Take, Stem, StemKind, Song
└── lib.rs      crate docs + re-exports
```

Dependencies: `serde` (derive) + `serde_json`. No workspace-crate deps needed
for v0 (the model is self-contained), so `gooz-ratio` is dropped from this
crate's manifest.

### Types (model.rs)

```rust
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub bpm: f64,
    pub beats_per_bar: f64,
    pub root_hz: f64,
    pub odd_limit: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StemKind { Riff, Beat, Other }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Take {
    pub name: String,
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stem {
    pub name: String,
    pub kind: StemKind,
    pub sample_rate: u32,
    pub bars: u32,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Song {
    pub format_version: u32,
    pub name: String,
    pub settings: Settings,
    pub takes: Vec<Take>,
    pub stems: Vec<Stem>,
    pub model_ref: Option<String>,
}
```

`Song::new(name, settings)` seeds `format_version = FORMAT_VERSION`, empty
takes/stems, `model_ref = None`. Builders `with_take` / `with_stem` append.

### Save / load (model.rs)

```rust
impl Song {
    pub fn to_json(&self) -> Result<String, SessionError>;
    pub fn from_json(s: &str) -> Result<Song, SessionError>;
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SessionError>;
    pub fn load(path: impl AsRef<Path>) -> Result<Song, SessionError>;
}
```

`save` = `to_json` (pretty) then write; `load` = read then `from_json`. Struct
field order fixes JSON key order, so serialization is deterministic (AC6).

### Errors (error.rs)

```rust
pub enum SessionError { Io(String), Serialize(String), Deserialize(String) }
```

`Display` + `std::error::Error`; `io::Error` and `serde_json::Error` are mapped
in at the call sites (kept `String`-backed so `SessionError` stays `Clone`/`Eq`
for test assertions, while remaining typed and meaningful).

## 3. Non-goals

Arrangement semantics (R-0011), binary/WAV export + directory layout (R-0012),
influence-model logic (M4 — only `model_ref`), UI (R-0013), format migration.

## 4. Acceptance criteria

Maps to R-0010 AC1–AC7; qa owns `crates/gooz-session/tests/acceptance_r0010.rs`.

- [x] AC1 — `to_json`/`from_json` round-trip equality
- [x] AC2 — `save`/`load` round-trip equality (temp file)
- [x] AC3 — `Stem` carries samples/rate/bars/kind; `Take` carries samples/rate
- [x] AC4 — missing file / corrupt JSON → typed `SessionError`, no panic
- [x] AC5 — empty song round-trips and saves/loads
- [x] AC6 — identical bytes on repeated `to_json`
- [x] AC7 — docs + four gates green

## 5. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | Samples typed `Vec<f32>` embedded in JSON | Lossless and simple for v0; binary/WAV is R-0012. |
| 2026-07-02 | `SessionError` variants hold `String` messages | Keeps the error `Clone`/`PartialEq` for test assertions while staying typed; the underlying `io`/`serde` errors are mapped via `Display` at the boundary. |
| 2026-07-02 | Drop `gooz-ratio` dep; settings are a plain struct | The on-disk model must not depend on a non-serializable engine type; dependencies point out of the session, not in. |

## Changelog

- 2026-07-02 — created; accepted alongside R-0010.
