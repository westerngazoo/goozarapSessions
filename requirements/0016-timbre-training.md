# R-0016 — On-device training (DDSP timbre decoder)

- **Status:** Accepted
- **Milestone:** M4
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-03
- **Depends on:** R-0014 (registry), R-0015 (feature extraction), R-0005 (analysis)
- **Realized by:** SPEC-0016
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must **train a small timbre model on-device**, locally, from a
reference recording — the first learned adapter in the app and the first use of
`candle`. Given a reference (or its features), the app extracts a **harmonic
timbre target** (the relative strength of a note's harmonics) and trains a small
neural **timbre decoder** to reproduce it, reporting **training progress** (loss
per epoch) and saving the trained weights into the song's model directory
(R-0014). The model reloads and reproduces its learned timbre.

This is the **training half of M4**. It produces the artifact that timbre
transfer (R-0017) will render a hum through. It runs on the CPU, in seconds, on a
laptop — never on the audio thread.

## 2. Rationale

An influence model has to be *trained*, and the project's premise is that this
happens **locally and small** — no cloud, minutes not hours (`docs/ARCHITECTURE.md`
§5). A DDSP-style timbre decoder is the natural first target: its output is a
harmonic amplitude distribution, which is exactly what our ratio-native synth can
consume later. Doing it now proves the whole on-device training loop — candle,
optimizer, progress, save/load into the registry — on a tiny, fast, deterministic
model, so heavier adapters (LoRA, R-0024) reuse the same seam. Keeping the model
small and seeded keeps training reproducible and testable.

## 3. Acceptance criteria

- **AC1 — Extract a timbre target.** From a reference recording, the app derives
  a normalized harmonic-amplitude vector (the magnitudes at `f0, 2·f0, …` for the
  reference's dominant pitch), summing to 1; a signal with no detectable pitch
  yields a neutral (uniform) target.
- **AC2 — Train on-device.** `train_timbre(target, cfg, progress)` trains a small
  candle decoder for `cfg.epochs` epochs with learning rate `cfg.lr`, entirely on
  the CPU.
- **AC3 — Progress reporting.** Training reports per-epoch progress (epoch + loss)
  via a callback and returns the loss history; the loss **decreases** over
  training (the model learns).
- **AC4 — Learns the timbre.** After training, the decoder's output harmonic
  distribution approximates the target (error below a tolerance on a fixture).
- **AC5 — Save & reload.** The trained weights are saved into the model's
  directory (registered in the manifest) and reload to a model that reproduces the
  same harmonics.
- **AC6 — Deterministic.** With a fixed `cfg.seed`, training is reproducible —
  the same target + config yields the same learned harmonics.
- **AC7 — Typed errors, no panic.** Tensor/IO failures surface as a typed
  `ModelError`; library paths never panic.
- **AC8 — Docs & gates.** Every public item is documented; covered by tests; all
  four toolchain gates are green (candle builds in the workspace).

## 4. Constraints & non-goals

- **candle** (CPU) is the framework; the model is tiny (a small MLP → softmax over
  a handful of harmonics). No GPU, no `ort` in this requirement.
- **Not timbre transfer** (R-0017): this trains and stores the decoder; rendering
  a hum through it is the next requirement.
- **Not a full DDSP synthesizer**: v0 learns a static harmonic distribution (the
  decoder's core output), not time-varying control curves or a noise branch.
- No training UI here — progress is exposed as data (callback + history); wiring it
  into the shell is later (R-0013 extension).
- Training never runs on the audio thread; reference audio stays local.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-03 | The timbre target is a **harmonic-amplitude distribution** (Goertzel at `k·f0`), normalized | It is the DDSP decoder's natural output and is directly consumable by the ratio synth for transfer (R-0017). |
| 2026-07-03 | v0 decoder is a **small MLP → softmax** over `n_harmonics`, trained with AdamW | Smallest real learned model that proves the candle training loop; softmax guarantees a valid distribution. |
| 2026-07-03 | Training is **seeded and CPU-only** for determinism | Reproducible tests and a laptop-friendly, no-GPU guarantee. |
| 2026-07-03 | Weights saved as **safetensors** in the model dir, recorded in the manifest | Standard, portable, and consistent with the R-0014 registry layout. |

## Changelog

- 2026-07-03 — created, accepted for M4.
