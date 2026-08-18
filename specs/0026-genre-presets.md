# SPEC-0026 — Genre & style preset library

- **Status:** Proposed — architect review pending
- **Realizes:** R-0026
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-04
- **Depends on:** SPEC-0025 (`MusicalIntent`), R-0002 (`Pattern`/`E(k,n)`), R-0009
- **Module(s):** `crates/gooz-model` (`preset.rs`), adapter in `apps/gooz-studio`

## 1. Motivation

Realize R-0026: turn a `MusicalIntent` into a concrete, ratio-native
**`SoundPlan`** via a data-driven genre preset table, so a description becomes
engine parameters deterministically and without a model.

## 2. Design

### Where it lives — and why

`MusicalIntent` lives in `gooz-model`; `BeatVoiceSpec`/`BeatConfig` live in
`apps/gooz-studio` (the app layer, which depends on `gooz-model`, not the
reverse). So the preset mapping **cannot** emit `BeatVoiceSpec` from
`gooz-model` without inverting the dependency arrow.

Resolution: `gooz-model` emits an **engine-agnostic `SoundPlan`** built only from
primitives and ratio concepts; `gooz-studio` adapts it to its own beat config.
The genre knowledge stays next to the intent; the app keeps its own types.

```
gooz-model:  MusicalIntent ──plan_sound()──▶ SoundPlan   (pure, no synth/app types)
gooz-studio:                                 SoundPlan ──▶ BeatConfig + grid + drive
```

### Types (`preset.rs`)

```rust
/// Which kit role a lane plays. Mirrors the kit without importing synth types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoiceRole { Kick, Snare, Hat }

/// One drum lane, ratio-native: E(onsets, steps) rotated, at a level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoicePlan {
    pub role: VoiceRole,
    pub onsets: u32,   // k
    pub steps: u32,    // n
    pub rotate: i64,
    pub level: f32,
}

/// A description made concrete: what to play, in engine terms.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundPlan {
    pub tempo_bpm: f64,
    pub meter: Meter,
    pub odd_limit: u64,        // harmonic-series grid size, from `tension`
    pub drive: f32,            // passes through from the intent
    pub voices: Vec<VoicePlan>,
    pub preset: String,        // which preset was chosen ("trap", "free", …)
}
```

`SoundPlan` is serializable and inspectable for the same reason the intent is:
the UI can show *"esto voy a tocar"* and the user can adjust before rendering.

### The preset table (data)

```rust
struct Preset {
    name: &'static str,
    tags: &'static [&'static str],   // intent genre tags that select it
    steps_per_beat: u32,             // grid resolution → steps = beats · this
    // per role: (min_k, max_k) as a fraction of steps, rotation in steps, level
    kick:  Lane, snare: Lane, hat: Lane,
}
struct Lane { min: f32, max: f32, rotate_beats: f32, level: f32 }
```

Initial library (tuned by ear later, per R-0026 §4):

| preset | tags | steps/beat | kick | snare | hat |
|--------|------|-----------|------|-------|-----|
| `trap` | trap, drill | 4 | sparse | backbeat (½-time) | busy, rolls |
| `corrido` | corrido, tumbado, bélico, regional | 2 | sparse | **on beat 3** | busy triplet-feel |
| `metal` | metal, black metal, punk, rock | 4 | driving | backbeat | steady |
| `free` | — (fallback) | 4 | today's Easy Mode defaults | | |

Selection: the first preset whose `tags` intersect `intent.genre` (table order =
priority); otherwise `free` (**AC2**). Deterministic — no scanning ambiguity.

### Mapping (`plan_sound`)

```rust
pub fn plan_sound(intent: &MusicalIntent) -> SoundPlan;
```

1. `preset = select(intent.genre)`.
2. `steps = intent.meter.beats · preset.steps_per_beat` — divisible by the beat
   count, so accents land on real beats in 6/8 as well as 4/4 (**AC4**).
3. Per lane: `k = round(lerp(min, max, intent.density) · steps)`, clamped to
   `1..=steps`; `rotate = round(rotate_beats · steps_per_beat)` (**AC1, AC3**).
4. `odd_limit = odd_limit_for(intent.tension)` — the same 3→15 odd-harmonic walk
   the shell's smooth↔tense slider already uses, moved here so one rule serves
   both (**AC3**).
5. `drive = intent.drive`; `tempo_bpm`/`meter` copy from the (already normalized)
   intent.
6. Every lane is emitted with `0 < k ≤ n`, `level ∈ [0,1]` (**AC5**).

Pure, total, no `Result`: a normalized intent can always be planned, and
`MusicalIntent::normalized` (SPEC-0025) guarantees the input is sane.

### App adapter (`gooz-studio`)

`fn beat_config_from(plan: &SoundPlan, bars: u32) -> BeatConfig` maps
`VoiceRole → DrumKind` and `VoicePlan → BeatVoiceSpec`. Thin and mechanical —
the app's only new knowledge is its own type mapping.

## 3. Code outline

```rust
pub fn plan_sound(intent: &MusicalIntent) -> SoundPlan {
    let preset = select_preset(&intent.genre);
    let steps = (intent.meter.beats * preset.steps_per_beat).max(1);
    let lane = |l: &Lane, role| VoicePlan {
        role,
        onsets: ((lerp(l.min, l.max, intent.density) * steps as f32).round() as u32)
            .clamp(1, steps),
        steps,
        rotate: (l.rotate_beats * preset.steps_per_beat as f32).round() as i64,
        level: l.level,
    };
    SoundPlan {
        tempo_bpm: intent.tempo_bpm,
        meter: intent.meter,
        odd_limit: odd_limit_for(intent.tension),
        drive: intent.drive,
        voices: vec![
            lane(&preset.kick, VoiceRole::Kick),
            lane(&preset.snare, VoiceRole::Snare),
            lane(&preset.hat, VoiceRole::Hat),
        ],
        preset: preset.name.to_string(),
    }
}
```

## 4. Non-goals

- No audio (R-0027), no parsing (R-0025), no per-instrument prompts (R-0032).
- No influence-model biasing of preset choice (R-0018).
- Does **not** implement 808/FX/6-8 engine support (R-0033/34/35) — the plan may
  ask for a meter the beat builder cannot fully honour yet; that is expected.

## 5. Open questions

None — the preset numbers are explicitly "tune by ear later" (R-0026 §4).

## 6. Acceptance criteria mapping

- AC1 → `plan_sound` + `SoundPlan`; determinism test.
- AC2 → `select_preset` table + fallback-to-`free` test (unknown/empty genre).
- AC3 → monotonicity tests: density↑ ⇒ total onsets never fall; tension↑ ⇒
  odd_limit never falls; drive passes through.
- AC4 → 6/8 test: `steps % meter.beats == 0`; corrido snare rotation lands on beat 3.
- AC5 → invariant test over a sweep of intents: `0 < k ≤ n`, levels in `[0,1]`.
- AC6 → north-star test: parsed intent → plan is 135 BPM, 6/8, busy hats, high
  odd_limit, high drive, preset ∈ {trap, corrido}.
- AC7 → four gates + docs.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-04 | Emit an **engine-agnostic `SoundPlan`**; the app adapts it | `BeatVoiceSpec` lives in the app layer; emitting it from `gooz-model` would invert the dependency arrow. Keeps genre knowledge with the intent. |
| 2026-07-04 | `steps = meter.beats · steps_per_beat` | Makes the grid meter-aware by construction, so accents land on real beats in 6/8 (AC4). |
| 2026-07-04 | `odd_limit_for(tension)` moves into the preset layer | One rule for smooth↔tense serves both the shell slider and the prompt path (the shell keeps its own copy until R-0027 wires the plan in). |
| 2026-07-04 | `plan_sound` is **total** (no `Result`) | `MusicalIntent::normalized` already guarantees sane input; a planner that cannot fail is simpler for every caller. |

## Changelog

- 2026-07-04 — created; proposed for architect review.
