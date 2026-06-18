# SPEC-0005 — Pitch tracking & onset detection (note transcription)

- **Status:** Accepted
- **Realizes:** R-0005
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-06-18
- **Depends on:** none (new code in `gooz-dsp`; consumes raw samples)
- **Module(s):** `crates/gooz-dsp`

## 1. Motivation

Realizes R-0005: bring `gooz-dsp` to life — transcribe a recorded monophonic
take into note events via YIN pitch tracking and spectral-flux onset detection.
The first analysis stage of the Easy Mode hum→riff loop.

## 2. Design

`gooz-dsp` modules; one new dependency (`rustfft` for the onset STFT). Operates
on `&[f32]` + sample rate — no `gooz-audio`, no device.

```
crates/gooz-dsp/Cargo.toml   + rustfft = "6"   (drop the unused gooz-ratio dep; R-0006 re-adds it)
crates/gooz-dsp/src/
├── lib.rs        crate docs + re-exports
├── error.rs      DspError — typed analysis error
├── yin.rs        YIN pitch detection (per frame) + pitch_track over a signal
├── onset.rs      spectral-flux onset detection (rustfft)
└── transcribe.rs Config, NoteEvent, Transcription, analyze() — ties it together
```

### `DspError` (error.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DspError {
    EmptySignal,        // no samples to analyze
    InvalidSampleRate,  // zero sample rate
    WindowTooLarge,     // analysis window longer than the signal
    NonFiniteSample,    // input contains a NaN or infinite sample
}
```

Fieldless (matches the other crates' error style); `Display` + `std::error::Error`.
No panic on library paths.

### Types (transcribe.rs)

```rust
pub struct PitchFrame { pub time_secs: f64, pub f0_hz: Option<f32>, pub confidence: f32 }
pub struct PitchTrack { pub frames: Vec<PitchFrame> }      // f0_hz == None ⇒ unvoiced
pub struct Onset { pub time_secs: f64, pub strength: f32 }
pub struct NoteEvent { pub onset_secs: f64, pub pitch_hz: f32, pub duration_secs: f64 }
pub struct Transcription { pub pitch_track: PitchTrack, pub onsets: Vec<Onset>, pub notes: Vec<NoteEvent> }

pub struct Config {
    pub window: usize,        // YIN analysis frame length (default 2048)
    pub hop: usize,           // frames advance (default 256)
    pub f_min: f32,           // default 80.0
    pub f_max: f32,           // default 1000.0
    pub yin_threshold: f32,   // absolute threshold (default 0.15)
    pub fft_size: usize,      // onset STFT size (default 1024)
    pub onset_sensitivity: f32, // peak-pick margin (× local stddev) (default 0.3)
    pub onset_window_frames: usize, // ± frames for the adaptive threshold (default 8)
}
impl Default for Config { /* the values above */ }
```

`notes` is the headline output (R-0005's chosen result); `pitch_track` and
`onsets` are the intermediate analyses, exposed because they are computed anyway
and aid tests/visualization.

### YIN (yin.rs) — AC1, AC2

`fn detect_pitch(frame: &[f32], sample_rate: u32, cfg: &Config) -> (Option<f32>, f32)`
returns `(f0, confidence)`; per de Cheveigné & Kawahara:

1. integration window `w = frame.len() / 2`; difference function
   `d(τ) = Σ_{j=0}^{w-1} (x[j] − x[j+τ])²` for `τ` in `0..=w`.
2. cumulative mean normalized difference: `d'(0) = 1`,
   `d'(τ) = d(τ) / ((1/τ) Σ_{k=1}^{τ} d(k))`.
3. search `τ` over a **generous fixed range** `[sr/2000, sr/50]` (clamped to
   `[2, w]`) — wide enough to find the true fundamental even when it is *above*
   `f_max`. Pick the **first** local minimum with `d'(τ) < yin_threshold`, else
   the global minimum over the range. (Searching only `[sr/f_max, sr/f_min]`
   would be wrong: a tone above `f_max` would alias to its in-range subharmonic
   — e.g. 900 Hz → ~453 Hz — and be falsely reported. Finding the real
   fundamental and rejecting it in step 5 is correct.)
4. parabolic interpolation around the chosen `τ` for a sub-sample period,
   clamped so `τ_refined ≥ τ_min` (it can never be 0, so `f0` is finite).
5. `f0 = sr / τ_refined`; `confidence = (1 − d'(τ)).clamp(0.0, 1.0)` (`d'` can
   exceed 1). **Voiced** iff `d'(τ) < yin_threshold` and `f0` within
   `[f_min, f_max]`; otherwise the frame's `f0_hz` is `None`.

`fn pitch_track(signal: &[f32], sample_rate: u32, cfg: &Config) -> Result<PitchTrack, DspError>`
slides `window` by `hop`, stamping each frame at its centre time.

### Onsets (onset.rs) — AC3

`fn detect_onsets(signal: &[f32], sample_rate: u32, cfg: &Config) -> Result<Vec<Onset>, DspError>`:

1. Hann-windowed STFT (`fft_size`, hop `cfg.hop`) via a cached `rustfft` plan.
   Magnitudes `|X_m[k]|` are raw (un-normalized) — the peak-pick threshold is
   relative, so normalization is irrelevant.
2. spectral flux `SF[m] = Σ_k max(0, |X_m[k]| − |X_{m-1}[k]|)`, with an
   **implicit all-zero frame before the first** (`X_{-1} = 0`), so `SF[0] = Σ_k
   |X_0[k]|` = frame 0's energy. This makes an attack that begins at sample 0
   (energy already present, no rising edge) register as an onset at `t ≈ 0` —
   satisfying "a steady tone produces exactly one onset (its start)".
3. threshold: `SF[m]` must exceed `mean + onset_sensitivity · stddev` taken over
   the **whole** flux array (a global adaptive threshold). A local window tracks
   the sustain level too closely and fires on steady-state flux ripple; against
   the global distribution an attack spike towers over the signal while sustain
   ripple does not.
4. release rejection: also require the frame's **energy to be rising** —
   per-frame energy is `Σ_k |X_m[k]|²` (sum of squares, per Parseval; *not*
   summed magnitudes, which a broadband edge transient can inflate), and the
   onset frame must have `energy[m] > energy[m-1]`. This drops the broadband
   transient at a note *release* (tone→silence), which produces positive flux
   but falling energy.
5. peak-pick: among frames passing 3–4, `m` is an onset if `SF[m]` is the
   maximum over `±onset_window_frames` frames (truncated at the edges), with a
   minimum inter-onset gap (≈30 ms) so one attack is one onset. Onset time =
   `m · hop / sr`, strength = `SF[m]`.

### Assembly (transcribe.rs) — AC4, AC5

`fn analyze(signal: &[f32], sample_rate: u32, cfg: &Config) -> Result<Transcription, DspError>`:

1. validate: empty → `EmptySignal`; `sample_rate == 0` → `InvalidSampleRate`;
   any non-finite sample → `NonFiniteSample`; `window > signal.len()` →
   `WindowTooLarge`. (Non-finite is rejected up front so no downstream sum,
   sort, or `median` can be poisoned by a NaN.)
2. `pitch_track` + `detect_onsets`.
3. note boundaries: the onset times, **plus a synthetic boundary at the first
   voiced frame's time when the first voiced frame precedes the first onset by
   more than one hop** (so a leading note isn't dropped if its attack escaped
   the onset detector), plus the signal end as the final boundary. For each
   consecutive pair `[start, end)`, gather the voiced `f0` of frames whose
   centre falls in `[start, end)`; **emit a note only if there is ≥ 1 voiced
   frame and the duration is strictly positive**, with
   `pitch_hz = median(voiced f0)`, `onset_secs = start`, and
   `duration_secs = end − start` where for the final segment `end` is the time
   of its last voiced frame (clamped so `end > start`). Result is sorted by
   onset and non-overlapping by construction.

## 3. Code outline

```rust
// yin.rs — cumulative mean normalized difference + threshold pick
fn cmnd(diff: &[f32]) -> Vec<f32> {
    let mut out = vec![1.0; diff.len()];
    let mut running = 0.0f32;
    for tau in 1..diff.len() {
        running += diff[tau];
        out[tau] = if running > 0.0 { diff[tau] * tau as f32 / running } else { 1.0 };
    }
    out
}

// transcribe.rs — representative pitch
fn median(mut xs: Vec<f32>) -> Option<f32> {
    if xs.is_empty() { return None; }
    xs.sort_by(|a, b| a.total_cmp(b));
    Some(xs[xs.len() / 2])
}
```

## 4. Non-goals

- Streaming/real-time analysis — later requirement.
- Polyphony, pYIN/HMM smoothing, SWIPE — v0 is plain YIN, monophonic.
- Grid quantization (R-0006), instrument rendering (R-0007), the full pipeline
  (R-0008).

## 5. Open questions

None — settled in the decision log.

## 6. Acceptance criteria

Maps to R-0005 AC1–AC7; qa owns `tests/acceptance_r0005.rs`, golden synthetic
signals only (no microphone).

- [ ] AC1 — YIN within ±1 % on synth tones (220/330/440 Hz).
- [ ] AC2 — silence/noise unvoiced; tone voiced; unvoiced never contributes pitch.
- [ ] AC3 — K bursts → K onsets within ≈±20 ms; steady tone → 1 onset.
- [ ] AC4 — two-tone signal → 2 ordered, non-overlapping notes, right pitch/onset.
- [ ] AC5 — `[f_min,f_max]` range honoured; notes only from voiced segments.
- [ ] AC6 — typed `DspError` for empty / zero-rate / non-finite / window-too-large;
      no panic; non-finite input rejected before any sum/sort.
- [ ] AC7 — doc examples on public items; golden tests; four gates green.

## 7. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-06-18 | `rustfft` for the onset STFT; drop the scaffold's unused `gooz-ratio` dep (R-0006 re-adds it) | v0 onset needs an FFT; R-0005 needs no ratio math (note pitches are Hz; ratios come at the snap step). |
| 2026-06-18 | `analyze()` returns one `Transcription` (notes + pitch_track + onsets) | Single entry point; the headline note events plus the intermediate analyses in one value. |
| 2026-06-18 | Parabolic interpolation on the YIN period | Sub-sample period → cents-accurate f0 from a modest window; standard YIN refinement. |
| 2026-06-18 | Minimum inter-onset gap in the peak-picker | Prevents one attack registering as several onsets (AC3's "not a stream of spurious ones"). |
| 2026-06-18 | Architect review (REQUEST CHANGES) applied: implicit zero frame before the first so a sample-0 attack onsets at t≈0; synthetic boundary at the first voiced frame so a leading note isn't dropped; emit a note only when ≥1 voiced frame AND duration > 0; reject non-finite input with `DspError::NonFiniteSample` before any sum/sort; clamp `τ_refined ≥ τ_min` and `confidence` to [0,1]; pin the adaptive-threshold window (`onset_window_frames`, edge-truncated) and raw magnitudes | Findings 1–6 of the SPEC-0005 review. Findings 1–2 were blocking (sample-0 attack / dropped leading segment broke AC3/AC4). |
| 2026-06-18 | Golden test corpus may use a short silent lead-in before the first tone | Gives the first attack a genuine rising edge; the implicit-zero-frame rule also covers the sample-0 case, so the suite is robust either way. |
| 2026-06-18 | YIN searches a generous fixed `τ` range `[sr/2000, sr/50]`, then gates the detected `f0` by `[f_min, f_max]` (rather than restricting the search to `[sr/f_max, sr/f_min]`) | Discovered at implementation: restricting the search aliases an above-`f_max` tone to its in-range subharmonic (900 Hz → ~453 Hz), which would wrongly produce a note and break AC5. Finding the true fundamental and rejecting out-of-range f0 is correct. |
| 2026-06-18 | Onset threshold is **global** (mean+sensitivity·stddev over the whole flux), not a local centred window | Discovered at implementation: a local window tracks the sustain level and fires repeatedly on steady-state flux ripple (one tone → many onsets). The global distribution cleanly separates attack spikes from ripple. |
| 2026-06-18 | Onset requires **rising energy** (`Σ|X|²`), rejecting note releases | Discovered at implementation: a tone→silence release makes a broadband edge transient with positive flux, producing a false onset. Energy (sum of squares) genuinely falls at a release, so the rising-energy gate removes it while keeping attacks. |

## Changelog

- 2026-06-18 — created; accepted alongside R-0005.
