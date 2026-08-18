# SPEC-0025 — Describe → MusicalIntent

- **Status:** Accepted (architect review: REQUEST CHANGES → findings applied)
- **Realizes:** R-0025
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-04
- **Depends on:** SPEC-0014 (model registry — where weights live), R-0016 (candle
  in `gooz-model`); [`docs/ai-music-direction.md`](../docs/ai-music-direction.md)
- **Module(s):** `crates/gooz-model`

## 1. Motivation

Realize R-0025: parse a natural-language prompt into an inspectable
`MusicalIntent`. Per the owner, the parser is a small local **`candle` LM**; per
the constitution (§5 TDD, deterministic gates) and the honesty rule, the LM sits
on a **mandatory deterministic fallback**. The intent feeds R-0026 (presets) and
R-0027 (generation).

## 2. Design

### Module layout

```
crates/gooz-model/src/
├── intent.rs   MusicalIntent, Meter, SectionIntent — serde, Default, validation
├── parse.rs    Parser trait + DefaultParser (deterministic keyword/number scan)
└── llm.rs      LmParser (candle) — compiled only under the `llm` feature
```

### Cargo features — keep the gates fast, the LM opt-in

```toml
[features]
# The candle instruct-model path. Heavy (pulls the transformer stack and a
# tokenizer) and needs model weights, so it is OFF by default: the four toolchain
# gates build only the deterministic parser and never download a model. Compile
# the LM path with `--features llm` (by-hand, per R-0025 AC3).
llm = ["dep:candle-transformers", "dep:tokenizers"]
```

The deterministic `DefaultParser` is **always** compiled. This is the same
"heavy ML is opt-in" stance the workspace already takes with the excluded Tauri
crate: CI stays green and fast; the model path is exercised by hand.

### Types (`intent.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MusicalIntent {
    pub tempo_bpm: f64,           // default 92.0 (Easy Mode tempo)
    pub meter: Meter,             // default 4/4
    pub tension: f32,             // 0..=1, smooth↔tense, default 0.30
    pub density: f32,             // 0..=1, sparse↔busy, default 0.55
    pub drive: f32,               // 0..=1, clean↔distorted, default 0.40
    pub genre: Vec<String>,       // tags, default []
    pub mood: Vec<String>,        // tags, default []
    pub structure: Vec<SectionIntent>, // default []
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meter { pub beats: u32, pub unit: u32 } // 6/8 = {6, 8}; default {4, 4}
```

- **Manual `impl Default`** (not `#[derive(Default)]`) for both `MusicalIntent`
  (the neutral values above — a derived `Default` would give `tempo_bpm = 0.0`,
  empty vecs) **and** `Meter` (`{4, 4}` — a derived `Default` would give the
  invalid `{0, 0}`). This is the correctness crux: container `#[serde(default)]`
  fills any missing JSON field from `MusicalIntent::default()`, so a partial LM
  JSON (e.g. `{"tempoBpm": 135}`) stays neutral where it is silent (**AC1**;
  robust LM parse). A malformed/partial `meter` object is caught by `normalized()`
  (below), never panics.
- `MusicalIntent::normalized(self) -> MusicalIntent`: clamps sliders to `[0,1]`,
  lowercases + dedups tags, and validates the meter (`beats > 0`, `unit ∈
  {1,2,4,8,16}`; else 4/4). **Always returns a valid intent** — the parser ends
  with this, so no path can emit garbage (**AC6**).
- `serde(default)` makes deserialization tolerant: a partial JSON from the LM
  fills only the fields it names, the rest stay neutral (**AC5**, robust LM parse).

### Parser seam (`parse.rs`)

```rust
pub trait Parser {
    /// Never fails: an unparseable prompt yields the neutral default intent.
    fn parse(&self, prompt: &str) -> MusicalIntent;
}

/// Deterministic keyword + number scan. Pure, allocation-simple, unit-tested.
pub struct DefaultParser;
```

`DefaultParser` extraction rules (all case-insensitive, order-independent):

| Field | Rule |
|-------|------|
| `tempo_bpm` | first **plausible** (40–250) number adjacent to `bpm`/`tempo` (`"135 BPM"`). An implausible neighbour is skipped, not clamped — `"un 808, bpm 135"` yields 135, and `"9000 bpm"` leaves the default |
| `meter` | first `N/M` with `M ∈ {2,4,8,16}` (`"6/8"`); else 4/4 |
| `tension` | high-cues {`tenso`,`tensión`,`menor`,`minor`,`segundas menores`,`dark`,`oscuro`,`disonante`,`black metal`} raise it; low-cues {`suave`,`smooth`,`mayor`,`major`,`warm`,`consonante`} lower it |
| `density` | high {`saturado`,`busy`,`rápido`,`tresillos`,`rolls`,`denso`}; low {`sparse`,`lento`,`minimal`} |
| `drive` | high {`distorsión`,`distortion`,`overdrive`,`crush`,`satura`,`crujir`,`fuzz`,`drive`}; low {`limpio`,`clean`} |
| `genre` | fixed vocab present in prompt: {`trap`,`corrido`,`tumbado`,`bélico`,`metal`,`black metal`,`drill`,`lofi`,`house`,`reggaeton`,…} |
| `mood` | {`dark`,`spooky`,`oscuro`,`uplifting`,`warm`,`aggressive`,`chill`,…} |

Returns `MusicalIntent::normalized()`. Deterministic ⇒ fully unit-testable (**AC2,
AC4**).

`LmParser` (feature `llm`) holds a loaded quantized instruct model + tokenizer.
`parse` builds an instruction ("*Extract musical parameters as JSON with keys
tempoBpm, meter, tension, density, drive, genre, mood from this description:
<prompt>*"), decodes greedily/low-temp, extracts the first JSON object, and
`serde`-deserializes it into `MusicalIntent` (tolerant via `serde(default)`),
then `.normalized()`. On **any** failure (load / decode / no-JSON / invalid) it
delegates to `DefaultParser` — so the LM path is never worse than deterministic
(**AC3, AC6**). Weights load from the session model dir (R-0014) or the `hf-hub`
cache; **no network at all — weights are read from disk; nothing leaves the
device**.

### Model choice (resolves the requirement's open question — architect to confirm)

Proposal: **Qwen2.5-0.5B-Instruct, GGUF Q4** (~400 MB) — strong instruction /
JSON-following for its size, and `candle-transformers` has quantized-Qwen
support. Lighter alternative: **SmolLM2-360M-Instruct**. Downloaded on first use
into the R-0014 model dir; never bundled into the binary. If a generative LM
proves too heavy on-device, the fallback plan is an embedding + slot-classifier
behind the same `Parser` trait (no API change) — the deterministic parser already
ships the feature meanwhile.

## 3. Code outline

```rust
// parse.rs
impl Parser for DefaultParser {
    fn parse(&self, prompt: &str) -> MusicalIntent {
        let p = prompt.to_lowercase();
        let mut intent = MusicalIntent::default();
        if let Some(bpm) = scan_tempo(&p) { intent.tempo_bpm = bpm; }
        if let Some(m) = scan_meter(&p) { intent.meter = m; }
        intent.tension = cue_scale(&p, TENSION_HIGH, TENSION_LOW, intent.tension);
        intent.density = cue_scale(&p, DENSITY_HIGH, DENSITY_LOW, intent.density);
        intent.drive   = cue_scale(&p, DRIVE_HIGH,   DRIVE_LOW,   intent.drive);
        intent.genre   = tags_present(&p, GENRE_VOCAB);
        intent.mood    = tags_present(&p, MOOD_VOCAB);
        intent.normalized()
    }
}

/// Always-available convenience (deterministic).
pub fn parse_intent(prompt: &str) -> MusicalIntent { DefaultParser.parse(prompt) }
```

## 4. Non-goals

- No genre→engine-params mapping (**R-0026**), no generation (**R-0027**), no
  per-instrument prompts (**R-0032**).
- The LM path is **not** a CI gate; no model is bundled; no cloud.

## 5. Open questions

- Final model + quantization — this spec proposes Qwen2.5-0.5B-Instruct GGUF;
  architect/owner confirm against the on-device budget.

## 6. Acceptance criteria mapping

- AC1 `MusicalIntent` + defaults → `intent.rs` types + `Default` + serde tests.
- AC2 `Parser` trait + deterministic fallback → `DefaultParser` + unit tests
  (fixtures → slots; empty → default; determinism).
- AC3 candle LM parser → `llm.rs` behind `llm` feature; seam-fixture / by-hand.
- AC4 rich-prompt extraction → a test feeding the owner's north-star prompt asserts
  `≈135` bpm, `6/8`, high tension/density/drive, `genre ⊇ {trap, corrido, metal}`.
- AC5 inspectable/editable → public fields + `normalized`; tolerant serde.
- AC6 local/graceful → fallback-on-any-failure; `normalized` can't emit garbage.
- AC7 tests/docs/gates → deterministic path + validation in the gate; LM by-hand.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-04 | LM path behind a `llm` cargo feature; deterministic parser always compiled | Keeps the four gates fast + model-free; heavy ML is opt-in (same stance as the excluded Tauri crate). |
| 2026-07-04 | `LmParser` falls back to `DefaultParser` on **any** failure | The LM is never worse than deterministic; guarantees AC6 with no user-facing errors. |
| 2026-07-04 | `MusicalIntent` ends every parse via `.normalized()` | One choke point guarantees a valid, clamped, meter-checked intent regardless of source. |
| 2026-07-04 | Propose Qwen2.5-0.5B-Instruct GGUF (SmolLM2-360M alt) | Small, on-device, strong JSON-following, candle-transformers support. Architect confirms. |
| 2026-07-04 | Land the **deterministic parser first**; the `llm` candle path follows in its own PR | AC2 makes the deterministic parser mandatory and independently valuable, and the `Parser` trait is the seam the LM plugs into with no API change — so this is the TDD-shaped order, not an under-delivery. R-0025 is **not** closed until AC3 lands. |
| 2026-07-04 | Cues and genre tags match **whole words**, not substrings | Architect finding: `str::contains` fired "hats" inside "whats" and "crush" inside a plugin name, moving sliders on un-actionable text (AC4 forbids exactly that). Token matching also removes the singular/plural double-counting. |
| 2026-07-04 | A tempo candidate must be **plausible** (40–250 BPM) to win | Architect finding: `"un 808, bpm 135"` yielded 808 (clamped to 250). "808" is this domain's most common number. |
| 2026-07-04 | A compound genre tag and its base are **both** reported ("black metal" ⇒ also "metal") | Architect finding: dropping the base contradicted AC4 and would break coarse consumers — R-0026's preset lookup keys on "metal". Subsumption is the consumer's call, not the parser's. |
| 2026-08-18 | The LM path's **text handling** (instruction building, JSON extraction) is compiled and gate-tested **unconditionally**; only model loading and decoding sit behind `llm` | Text wrangling is where the bugs hide — brace balance inside strings, escapes, truncated replies — so it must be covered by the gates even though inference is by-hand. A refinement of §2's module layout. |
| 2026-08-18 | Weights are read **from disk paths only**; `hf-hub` dropped from the feature | The implementation never downloads, so shipping a hub client (and its TLS/HTTP stack) would be a dead dependency and an unearned network surface. Fetching weights, if ever wanted, belongs to the model registry (R-0014). |

## Changelog

- 2026-07-04 — created; proposed for architect review.
