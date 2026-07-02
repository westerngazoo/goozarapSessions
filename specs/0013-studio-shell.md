# SPEC-0013 (v0 slice) — Studio shell

- **Status:** Implemented — v0 slice
- **Realizes:** R-0013 (v0 slice — wraps only R-0008)
- **Author:** Claude (owner: Gustavo Delgadillo)
- **Created:** 2026-07-02
- **Depends on:** SPEC-0008 (hum→riff), SPEC-0003 (record/play); SPEC-0005/6/7 transitively
- **Module(s):** `apps/gooz-studio` (lib `view`), `apps/gooz-studio/src-tauri` (shell), `apps/gooz-studio/ui` (frontend)

## 1. Motivation

Realizes the v0 slice of R-0013: a clickable Tauri desktop shell over the finished
hum→riff pipeline. First time the product is usable without a terminal, and it
establishes the Rust↔web command seam all later UI (M3/M5) extends.

## 2. Design

Three layers, dependency pointing inward:

```
apps/gooz-studio/ui/            vanilla HTML/CSS/JS frontend (Web Audio playback)
apps/gooz-studio/src-tauri/     Tauri shell: thin command wrappers (excluded from
                                the workspace merge gate — pulls the webview)
apps/gooz-studio/src/view.rs    device-free DTO layer (RiffView/NoteView), gated
```

### `view.rs` (in the workspace, unit-tested)

Serializable DTOs and the Easy-Mode pipeline entry points, kept out of the Tauri
crate so the workspace gate covers them:

```rust
pub struct NoteView { pub num: u64, pub den: u64, pub octave: i32, pub hz: f64, pub cents: f64 }

#[serde(rename_all = "camelCase")]
pub struct RiffView {
    pub sample_rate: u32, pub bars: u32, pub seconds: f64,
    pub notes: Vec<NoteView>, pub wave: Vec<f32>, pub samples: Vec<f32>,
}

pub fn riff_from_take(samples: &[f32], sample_rate: u32) -> Result<RiffView, DspError>;
pub fn demo_riff() -> RiffView; // synthesized four-tone hum, deviceless
```

Easy-Mode defaults: 220 Hz harmonic grid (9 harmonics), 92 BPM / 4. The stem is
downsampled to a 600-point peak envelope (`wave`) for the canvas.

### `src-tauri` (excluded crate)

Thin commands over the tested backend:

- `demo_riff()` → `gooz_studio::demo_riff()` (no mic)
- `beat(busy)` → `gooz_studio::beat_view(busy)` — Euclidean beat at a density
- `record_start()` → spawns a `cpal` capture thread (cpal streams are `!Send`)
- `record_stop_analyze()` → joins the thread, `gooz_studio::riff_from_take(...)`

### Beat builder in the shell (R-0009 wired in)

`view::beat_view(busy)` maps the sparse↔busy slider (`0..=100`) onto each drum
voice's `E(k, 16)` onset count (kick 2→8, snare 2→4, hat 4→16), calls
`build_beat`, and returns a `BeatView` (lanes + waveform envelope + samples).
The frontend plays it on its own looping Web Audio node, independent of the
riff, and re-renders live as the slider moves. A browser-only Bjorklund + click
synth mirrors the backend so the beat works in preview without Tauri.

The crate carries its own `[workspace]` table and is listed in the root
workspace `exclude`, so `cargo *--workspace*` never builds the webview runtime.

### `ui`

`index.html` + `style.css` (dark-neon) + `main.js`. Uses `withGlobalTauri` to
`invoke` the commands, renders note cards + a waveform canvas, and loop-plays the
returned samples via Web Audio. `fixture.js` lets the page be previewed in a
plain browser without the Tauri runtime.

## 3. Non-goals

Session persistence (R-0010), arrangement (R-0011), export (R-0012), beat-builder
wiring (R-0009 into the shell), functional ratio sliders, Advanced Mode. Full
R-0013 (wrapping M3) follows in sequence.

## 4. Acceptance criteria

Maps to R-0013 (v0) AC1–AC6; qa owns `apps/gooz-studio/tests/shell_record_mock.rs`
(the record→riff flow via the deviceless `VirtualBackend`) plus the `view` unit
tests.

- [x] AC1 — Tauri window renders the dark-neon shell (by eye / preview)
- [x] AC2 — deviceless `demo_riff` command → note cards + waveform
- [x] AC3 — `record_start`/`record_stop_analyze` live path (by hand)
- [x] AC4 — Web Audio loop playback
- [x] AC5 — each note shows `n:d`, Hz, cents
- [x] AC6 — command/view layer unit-tested; docs; four gates green (shell excluded from gate)

## 5. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-02 | Integrate the parallel R-0013 v0 shell on top of merged R-0009 | Keeps the beat builder and the UI shell both on `main`; the shell wraps R-0008 today and gains the beat builder when wired later. |
| 2026-07-02 | DTO/view logic in the workspace lib; only the IPC bridge in `src-tauri` | The reviewed gate covers the music logic; the excluded crate stays a thin, webview-only seam. |
| 2026-07-02 | Wire the beat builder (R-0009) into the shell: `beat_view` + `beat` command + a functional sparse↔busy slider | Makes the second half of Easy Mode clickable; the slider now drives real `E(k, n)` density instead of being a visual stub. |
| 2026-07-02 | Wire the smooth↔tense slider onto the harmonic-series odd-limit (3→15) via `riff_from_take(.., tense)` | Turns the last visual-stub slider into a real control: smoother grids favour simple ratios, tenser grids admit complex ones. |
| 2026-07-02 | Wire **session save + WAV export** (R-0010/11/12) into the shell: `build_song` + `save_session` / `export_master` commands + Save/Export buttons | The Easy Mode loop now produces a *kept* song — a `.json` session and a mixed `.wav` — not just a transient demo. RiffView/BeatView gain `Deserialize` so the frontend can hand them back. |

## Changelog

- 2026-07-02 — created to document the integrated R-0013 v0 slice.
