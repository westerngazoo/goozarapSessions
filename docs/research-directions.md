# Research directions

Forward-looking threads that are **not** committed work. Nothing here is a
requirement or a spec; an idea graduates into the requirement loop
(`CLAUDE.md` §4) only when it is scheduled into a milestone. This file exists so
promising directions are not lost between now and when they're due.

## Geometric-algebra / Clifford ML for influence models (M4)

The headline research thread for **M4 — Influence models** (`gooz-model`). The
per-song "creative brain" is where learned models enter the system, and it is
the natural home for geometric-algebra (GA) / Clifford machine learning.

### Why it actually fits our constraints

- **Tiny data per model.** An influence model trains on *one song's* (or one
  album's) reference material — small data by ML standards. Equivariant
  architectures bake the relevant symmetry into the weights instead of forcing
  the network to learn it from examples, which makes them **sample-efficient**.
  That data-efficiency is a direct match for the "small, local, per-song"
  constraint in `docs/ARCHITECTURE.md` §5 — arguably the strongest single
  argument for GA ML here.
- **On-device, small adapters.** GA layers can be parameter-frugal (the
  geometric product carries structure the network would otherwise spend
  parameters approximating), which suits on-device training in minutes.

### Where it could plug in

1. **Multivector timbre encoder.** Treat a spectrogram as a 2-D field, derive a
   multivector feature per bin via the **monogenic signal** (Riesz transform =
   the Clifford-analytic generalization of the Hilbert/analytic signal), then
   feed those multivectors to Clifford-convolution layers. The monogenic
   signal's local *orientation* is naturally a multivector, so a GA encoder
   consumes it without throwing the structure away. (See the monogenic-signal
   note below — the two threads are the same idea seen from analysis vs. ML.)
2. **Clifford-group-equivariant feature encoder.** For geometric embeddings
   (points in a pitch × time × timbre space), a small `O(n)`-equivariant GA
   encoder regularizes a tiny dataset by construction.
3. **Spatial / multichannel timbre transfer (later).** Quaternion- or
   GA-valued signals processed by rotation-equivariant GA layers — only if a
   spatial-audio milestone ever lands.

### The honest caveat: match the algebra to *audio's* symmetries

Off-the-shelf GA ML targets **Euclidean** `E(n)`/`O(n)` rotations and
reflections — superb for 3-D point clouds, molecules, robotics. Audio's natural
symmetries are different:

- **Transposition** = translation in log-frequency,
- **Time / phase shift** = a cyclic group,
- **Tempo change** = scaling.

None of those is a Euclidean rotation. So the payoff comes from either (a)
applying GA where Euclidean equivariance *is* meaningful (geometric embeddings,
spatial audio, the orientation content of a spectrogram), or (b) constructing a
geometric algebra whose group matches an audio symmetry — which is past the
off-the-shelf frontier. **Do not assume "GA ⇒ automatically better for audio."**
The win is specific, not general.

### Landmark references (verify versions/links at use time)

- Brandstetter, van den Berg, Welling, et al. — *Clifford Neural Layers for PDE
  Modeling* (NeurIPS 2022). Multivector feature maps, Clifford convolutions.
- Ruhe, Brandstetter, Forré — *Clifford Group Equivariant Neural Networks*
  (CGENN, NeurIPS 2023). Clean, general `O(n)`/`E(n)`-equivariant construction.
- Ruhe et al. — *Geometric Clifford Algebra Networks* (GCAN, ICML 2023).
- Brehmer, de Haan, et al. — *Geometric Algebra Transformer* (GATr, NeurIPS
  2023). Transformer over multivectors in projective GA `Cl(3,0,1)`,
  `E(3)`-equivariant; aimed at 3-D problems.
- Roots: Bayro-Corrochano's geometric-algebra neural networks; quaternion
  neural networks as a special case (`ℍ ≅ Cl(0,2)`).

### Libraries

- **Python:** `cliffordlayers` (Microsoft Research, accompanies the PDE paper);
  `clifford` and `kingdon` (general GA, autodiff-friendly); `tfga`
  (TensorFlow); the GATr / CGENN reference implementations.
- **Rust-first reality:** this project's ML targets **candle**
  (`docs/ARCHITECTURE.md` §5), and there is no mature Rust GA-ML library. Expect
  to **hand-roll multivector ops on candle tensors** — a single geometric-product
  layer is tractable to write and test. Track upstream Rust GA crates as they
  mature.

### Keeping M4 "GA-ready" without committing now

- Keep the `gooz-model` feature-extraction → adapter seam clean and tensor-based
  (candle), so a Clifford/multivector layer can slot behind the same interface a
  plain conv adapter uses.
- Prototype on **one** concrete task first (the multivector timbre encoder) and
  measure it against a plain-conv baseline on a single song's data **before**
  generalizing. Equivariance must earn its place on our data, not on a
  benchmark's.

### Status

Research thread, parked against **M4**. Not scheduled. Revisit when M4's
requirements (R-0014–R-0018) are drafted.

## Clifford analysis for DSP: the monogenic signal (M4 / later)

Adjacent to the above, on the *analysis* side. The **monogenic signal**
(Felsberg–Sommer) generalizes the 1-D analytic signal to 2-D via the Riesz
transform — a Clifford-analysis construction — yielding local amplitude, phase,
and **orientation** for an image. Applied to a spectrogram it is a candidate
timbre/texture feature (orientation energy) for the influence models, and it is
the natural front-end for the multivector timbre encoder above.

For plain 1-D audio (pitch tracking, FFT, time-stretch — `gooz-dsp`, M2)
standard complex analysis / the analytic signal is complete; Clifford analysis
adds nothing there. The value is specifically in 2-D (spectrogram) and
spatial/multichannel settings.

### Status

Research thread, parked against **M4** (feeds the GA-ML encoder). Not scheduled.
