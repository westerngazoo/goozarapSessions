# R-0009 — Beat builder

- **Status:** Accepted
- **Milestone:** M2
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-02
- **Depends on:** R-0002 (Euclidean `E(k,n)` + bar grids), R-0003/R-0004 (playback for the demo)
- **Realized by:** SPEC-0009
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must turn Euclidean drum templates into a **loopable beat stem**: three
synthesized kit voices (kick, snare, hi-hat), each driven by its own `E(k, n)`
pattern from `gooz-ratio`, mixed into bar-aligned audio at a caller-supplied tempo
and sample rate. Each voice is controlled by a **sparse↔busy** pair — the onset
count `k` and the step count `n` — matching the product's ratio-first rhythm
language. A by-ear demo plays a default trap-flavoured groove in a loop. This
completes M2's Easy Mode foundation alongside the hum→riff pipeline (R-0008).

## 2. Rationale

Easy Mode is voice-driven, but a session needs a beat underneath the riff. The beat
builder is the rhythmic counterpart to hum→riff: instead of tracking pitch, the user
(or a default template) picks how dense each drum voice is via `E(k, n)`. The math
already exists in R-0002; this requirement renders it as sound and makes it
loopable. Influence-model biasing (R-0018) and UI sliders (R-0013) come later; v0
is programmatic config plus a device demo, mirroring R-0008.

## 3. Acceptance criteria

- **AC1 — Three-voice render.** Given kick, snare, and hi-hat patterns built from
  valid `E(k, n)` values, the renderer returns a non-empty, bounded stem for
  `bars ≥ 1` at a valid tempo and sample rate.
- **AC2 — Loopable (bar-aligned) stem.** The stem length is exactly
  `bars · bar_samples` (`bar_samples` from the tempo and sample rate); `bars ≥ 1`
  for a non-empty beat.
- **AC3 — Pattern placement.** Energy peaks in the rendered stem align with the
  Euclidean onset positions for each voice (within one step's sample tolerance per
  bar).
- **AC4 — k controls density.** Increasing a voice's onset count `k` (holding `n`
  fixed) increases the number of detected hits for that voice in one bar.
- **AC5 — Typed errors / input guard.** Invalid `E(k, n)` construction
  (`k > n`, `n == 0`) is reported as a typed [`BeatError`]; `bars == 0` or
  `sample_rate == 0` yields an empty stem without panic.
- **AC6 — Deterministic.** The same voices + tempo + bars + sample rate produce
  identical samples.
- **AC7 — Bounded, clean audio.** Stem samples are within `[-1, 1]` and contain
  no NaN/inf.
- **AC8 — Demo, docs, gates.** A `cargo run -p gooz-studio --bin beat` demo plays
  a default looped beat on a real machine (by ear; not a CI gate). Every public
  item is documented; the pure renderer is covered by tests; all four toolchain
  gates are green.

## 4. Constraints & non-goals

- Offline, deterministic synthesis in `gooz-synth`; integration + demo in
  `gooz-studio`. Depends inward (`gooz-synth → gooz-ratio`; studio → synth +
  audio).
- **808-style synthesized kit** (sine kick, noise snare, short noise hat); sampler
  and FM drums are later.
- **No influence-model biasing** (R-0018), **no session persistence** (R-0010), **no
  UI sliders** (R-0013). Caller supplies tempo and patterns (or uses defaults).
- Patterns may use **different `n` per voice**; each voice maps its steps across
  one full bar independently.
- No live/step-sequencer editing; no arbitrary non-Euclidean patterns.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | Three voices: kick, snare, hi-hat; each with its own `E(k,n)` | Matches ARCHITECTURE §4.2 and the sparse↔busy product language. |
| 2026-07-02 | Default trap template: kick `E(4,16)`, snare `E(2,16)` rotated +4, hat `E(7,16)` | Backbeat snare on steps 4 & 12 of a 16th grid; busy hats; four-on-the-floor kick. |
| 2026-07-02 | Synthesis in `gooz-synth`; `build_beat` + demo in `gooz-studio` | Same split as R-0007 (render) / R-0008 (integrate + demo). |
| 2026-07-02 | Separate demo binary `beat` (`cargo run -p gooz-studio --bin beat`) | Keeps the hum→riff demo (`main`) untouched; each M2 loop has its own by-ear entry point. |

## Changelog

- 2026-07-02 — created, accepted for M2.
