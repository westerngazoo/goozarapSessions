# R-0007 — Instrument render v0 (Karplus-Strong guitar + distortion)

- **Status:** Accepted
- **Milestone:** M2
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-06-19
- **Depends on:** R-0006 (`QuantizedNote`)
- **Realized by:** SPEC-0007
- **QA:** `qa` agent run scoped to this requirement

## 1. Statement

The project must render a stream of quantized notes (R-0006) into audio as a
plucked-string instrument: a **Karplus-Strong** voice tuned to each note's
frequency, mixed into one output buffer and run through a **distortion FX
chain** (a selectable soft-clip or hard-clip waveshaper with a drive control).
Each note is plucked at its onset and **rings out its natural decay** past its
grid duration — overlapping tails sum, like a real guitar where strings keep
ringing. The result is a rendered `f32` buffer: the *output* side of
voice-to-instrument. Excitation is deterministic (a fixed-seed pluck), so the
render is reproducible and testable without ears.

It turns "what the user hummed, snapped to the grid" into a guitar sound. It is
not yet wired to the recorder/engine — the end-to-end record→riff pipeline is
R-0008; other instruments (bass, drums, FM) and richer FX are later.

## 2. Rationale

This is where the app finally makes the user's idea audible *as an instrument*.
Karplus-Strong is the classic, cheap, convincing plucked-string model — the
right v0 guitar. Distortion is what makes it read as an electric guitar rather
than a toy pluck, and offering both a warm (soft-clip) and an aggressive
(hard-clip) voicing lets the same riff sound like overdrive or fuzz. Letting
notes ring naturally is what makes a sequence of plucks sound like playing,
not a metronome of blips. Determinism keeps the synthesis exact and testable —
the same quantized notes always render the same buffer.

## 3. Acceptance criteria

- **AC1 — In tune.** A single note in the integer-tuned band (delay length
  `n = round(sr/f) ≳ 48`, i.e. `f ≲ ~1 kHz` at 48 kHz) renders a tone whose
  fundamental period (by autocorrelation of the rendered region) is
  `≈ sample_rate / f` within ~1 % — the string is tuned to the note. (Above that
  band the integer-delay error grows past 1 %; correcting it is a later
  fractional-delay refinement, §4.)
- **AC2 — Plucked decay.** Within a rendered note the amplitude decays: the
  signal energy in a late window is strictly less than in an early window (a
  pluck that rings down, not a sustained drone).
- **AC3 — Onset placement.** Output before the first note's onset sample is
  silence (all ≈ 0); a note produces non-zero output starting at its onset.
- **AC4 — Let-ring mix.** Multiple notes mix into one buffer and each rings its
  natural decay past its grid duration: there is non-zero output in the first
  note's tail region beyond `onset + duration`, and the buffer length spans the
  last note's onset plus its decay tail. Notes sum (a later note entering does
  not silence an earlier one's tail).
- **AC5 — Distortion FX.** Both `SoftClip` and `HardClip` are available and each
  changes the signal versus undistorted; a higher drive increases saturation;
  for input in `[-1, 1]` both keep the output within `[-1, 1]` (soft-clip
  saturates smoothly, hard-clip clamps). The "clean" setting is mode-specific:
  SoftClip at low drive ≈ identity; HardClip is exactly identity at drive = 1.0
  (and near-silent at very low drive, by construction). The final rendered
  buffer never exceeds `[-1, 1]`.
- **AC6 — Deterministic & robust.** The same notes + config render an identical
  buffer (fixed-seed excitation); empty input or a zero sample rate yields an
  empty buffer; a note with a non-finite/non-positive frequency is skipped;
  output contains no NaN/inf; library paths never panic.
- **AC7 — Documented API & gates.** Every public item is documented with a
  runnable example; behaviour is covered by tests; all four toolchain gates are
  green.

## 4. Constraints & non-goals

- Consumes R-0006 `QuantizedNote`s + a sample rate + a render config; pure,
  offline, deterministic. Depends inward (`gooz-synth → gooz-dsp`).
- **One instrument (Karplus-Strong guitar) only**; bass, drums, FM/wavetable,
  and the sampler are later. **Distortion is the only FX**; delay/reverb later.
- **No engine wiring** — turning a recorded take into a played-back riff stem is
  R-0008; this requirement renders a note list to a buffer.
- v0 accepts the small Karplus-Strong tuning error (integer delay length);
  fractional-delay tuning correction is a later refinement.
- No real RNG (excitation is a fixed-seed PRNG for reproducibility); no
  per-note velocity/dynamics from cents offset yet.

## 5. Open questions

None — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-19 | Distortion offers **both** `SoftClip` (tanh) and `HardClip`, selectable, with a drive control | Owner choice — warm overdrive and aggressive fuzz from the same engine without redoing it later. |
| 2026-06-19 | **Let-ring**: each pluck rings its natural Karplus-Strong decay past its grid duration; tails sum | Owner choice — makes a sequence of plucks sound like a guitar being played, not gated blips. |
| 2026-06-19 | Deterministic fixed-seed pluck excitation (no real RNG) | Reproducible renders → golden-buffer tests; same quantized notes always sound the same. |
| 2026-06-19 | Lives in `gooz-synth`, consuming `gooz-dsp`'s `QuantizedNote` | It is the instrument/output stage; `gooz-synth → gooz-dsp` is the correct inward edge. |

## Changelog

- 2026-06-19 — created, accepted for M2.
