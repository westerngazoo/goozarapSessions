# R-0025 — Describe → MusicalIntent

- **Status:** Accepted
- **Milestone:** M7
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-04
- **Depends on:** M4 — `gooz-model` (R-0014 registry ✅, R-0016 candle integration ✅);
  design in [`docs/ai-music-direction.md`](../docs/ai-music-direction.md)
- **Realized by:** SPEC-0025
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must turn a **natural-language description** of a song — e.g.
*"corrido tumbado en 6/8 a 135, 808 distorsionado, requinto con segundas menores,
reverb larga"* — into an **inspectable, editable `MusicalIntent`**: a small,
serializable struct of musical parameters (tempo, meter, tension, density, drive,
genre, mood, structure, timbre words) that later stages use to steer the existing
ratio/beat/synth engine (R-0026 presets, R-0027 generation). This is the front
door of M7 "describe → music."

Per the owner's decision, the v0 parser is a **small local LM via `candle`** doing
constrained slot-filling. Per the Engineering Constitution (§5 TDD, deterministic
gates; ARCHITECTURE §5 "honesty rule"), the LM sits **on top of a deterministic
fallback**: with no model, no prompt, or an unparseable prompt, the parse still
returns a **valid, all-defaults `MusicalIntent`**. The intent is always
inspectable and editable — never a black box — and everything runs **on-device**;
the prompt never leaves the machine.

It is **not** generation (R-0027), **not** per-instrument prompts (R-0032), and it
does **not** clone plugins or artists named in a prompt — it extracts the musical
*intent*, ignoring the un-actionable.

## 2. Rationale

"Describe what you want and the app makes it" is the whole point of M7, and the
owner's north-star prompt (a corrido-tumbado × black-metal fusion) is the driving
test case. The `MusicalIntent` is the **seam**: a human-readable contract between
the language layer and the deterministic engine, so the model only ever *biases*
the math (never replaces it) and the UI can show/tweak exactly what it understood.
Making the parser LM-primary but fallback-deterministic keeps the core testable
and honest while giving free-text understanding of rich prompts.

## 3. Acceptance criteria

- **AC1 — `MusicalIntent` type.** A serializable struct where **every field has a
  neutral default** (an empty prompt ⇒ a valid, neutral intent). Fields (v0):
  `tempo_bpm`, `meter` (beats + unit, e.g. 6/8), `tension` (`0..=1`, smooth↔tense),
  `density` (`0..=1`, sparse↔busy), `drive` (`0..=1`, clean↔distorted),
  `genre` (tags), `mood` (tags), `structure` (optional section list). Round-trips
  through serde.
- **AC2 — Parser seam + deterministic fallback.** A `Parser` trait
  (`parse(&self, prompt: &str) -> MusicalIntent`) with a **deterministic**
  implementation (rules/keywords/number extraction) that is fully unit-tested:
  fixture prompts → expected intent slots (tolerant match); empty/garbage prompt →
  defaults; identical input → identical output.
- **AC3 — Local LM parser (candle).** A `candle`-backed parser loads a small
  quantized instruct model (from the session's model dir, R-0014) and produces a
  `MusicalIntent` via constrained/greedy slot-filling with validation. Runs
  **on-device**, no network at inference. Tested at the **API seam with a tiny
  fixture** (per ARCHITECTURE §7); the full-model path is **by-hand**, not a CI
  gate (like R-0008's by-ear demo).
- **AC4 — Extracts the actionable params from a rich prompt.** The owner's
  reference prompt yields (via either parser) approximately: `tempo≈135`,
  `meter=6/8`, `tension` high (minor-second cue), `density` high (saturated-hats
  cue), `drive` high (distortion cue), `genre ⊇ {trap, tumbado, black metal, metal}` — the styles the prompt
  actually names (it says *tumbado*, never *corrido*), with each compound tag
  reported alongside its base so coarse consumers match.
  Un-actionable content (specific plugins, artist names, DAW steps) is ignored
  without error.
- **AC5 — Inspectable & editable.** The returned intent is fully readable and every
  field is independently overridable by the caller/UI before it drives generation.
- **AC6 — Local, private, graceful.** On-device only; no model or no prompt ⇒
  neutral defaults, never a panic or an error surfaced to the user.
- **AC7 — Tests, docs, gates.** The deterministic path + `MusicalIntent`
  validation are covered by tests and the four toolchain gates are green. The LM
  path is exercised by a seam-level fixture test (or by-hand). Every public item is
  documented.

## 4. Constraints & non-goals

- Lives in `crates/gooz-model` (the ML/model crate; depends inward on `gooz-dsp`/
  `gooz-ratio`). No new engine or synth logic here.
- **LM-primary, fallback-deterministic** — the deterministic path is mandatory
  (TDD + honesty rule), not optional. The LM never becomes a hard CI gate.
- **No generation** (R-0027), **no genre→params mapping** (that is R-0026 presets),
  **no per-instrument prompts** (R-0032), **no cloud**, no plugin/artist cloning.
- The specific model (size, arch, quantization, source) is **deferred to
  SPEC-0025** — it must be small enough to run on a laptop and loadable by
  `candle`/`candle-transformers`.

### Reference prompt (the north-star test case)

The owner's prompt this requirement is measured against, reduced to its musical
sentences (the full original also named DAW plugins and artists, which the
parser must ignore):

> Pon el tempo a 135 BPM. La batería trap + tumbado en un compás de 6/8, snare
> seco en el tercer tiempo, y satura los hi-hats para que hagan tresillos
> rápidos. El bajo: un 808 largo con distorsión hasta que cruje. La guitarra:
> black metal + requinto, notas consecutivas muy juntas (segundas menores para
> dar tensión), con reverb de 4 segundos.

## 5. Open questions

- **Which model?** (SPEC-0025.) Candidate: a ≤1–3B quantized instruct model that
  `candle-transformers` supports (e.g. a small Qwen/Llama/Phi-class GGUF), or an
  embedding + slot classifier if a generative LM proves too heavy on-device.
- **Bundle vs. download** the weights (SPEC-0025), and where in the model dir
  (R-0014) they live.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-04 | v0 parser is a **small local `candle` LM** doing slot-filling | Owner choice — free-text understanding of rich prompts (the corrido×metal north-star). |
| 2026-07-04 | LM sits on a **mandatory deterministic fallback**; the LM path is **by-hand / seam-fixture**, not a CI gate | Constitution §5 (TDD, deterministic gates) + ARCHITECTURE §5 (honesty rule: works with no model). Keeps the core testable and the app graceful. |
| 2026-07-04 | `MusicalIntent` is small, serde-serializable, all-defaults, and **editable** | It is the human-readable seam between language and engine; the model biases, never replaces (and the UI can tweak it). |
| 2026-07-04 | Specific model deferred to **SPEC-0025** | Needs an architect + owner call on size/arch/quantization vs. on-device budget. |

## Changelog

- 2026-07-04 — created; draft for owner review (Discuss step).
- 2026-07-04 — accepted by owner ("así es dale"); proceeding to SPEC-0025.
