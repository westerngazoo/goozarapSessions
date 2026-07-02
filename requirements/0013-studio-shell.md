# R-0013 (v0 slice) — Studio shell

- **Status:** Accepted — **v0 slice, owner-authorized out of sequence** (built in
  parallel with R-0009; wraps only what exists today, R-0008)
- **Milestone:** M3
- **Owner:** Gustavo Delgadillo (see project-specifics.md)
- **Created:** 2026-07-01
- **Depends on:** R-0008 (hum→riff pipeline) ✅, R-0003 (record/play) ✅. The full
  R-0013 additionally wraps M3 (R-0010/R-0011/R-0012); this v0 slice does not.
- **Realized by:** SPEC-0013 (v0 slice)
- **QA:** `qa` agent run scoped to this requirement (v0 scope)

## 1. Statement

A **Tauri desktop shell** — dark-neon "Bizarrap studio" look — that opens straight
to **"hum something now"** and makes the existing hum→riff pipeline (R-0008)
**clickable**: record a hum (or run a built-in demo hum), see **what it heard**
(the grid-locked notes) and the **riff waveform**, and **loop-play** the riff.
This is the first time the product is usable without a terminal.

It is a **v0 slice**: it wraps only today's pipeline. Session persistence
(R-0010), arrangement (R-0011), export (R-0012), and the beat builder (R-0009)
are **not** wired in yet — they slot into this shell as they land.

## 2. Rationale

The whole premise is "make a sound → hear it back as an instrument." Until there
is a window with a record button, that loop lives in a `cargo run` demo. A thin
Tauri shell over the finished R-0008 pipeline turns the engine into something the
owner can actually click — and it establishes the Rust↔web command seam that all
later UI (M3/M5) will extend. Keeping a **deviceless demo command** alongside the
live-mic path means the shell is testable in CI and previewable without hardware.

## 3. Acceptance criteria

- **AC1 — App shell.** A Tauri window renders the dark-neon studio; first run
  shows the **"hum something now"** prompt and a record control.
- **AC2 — Demo path (deviceless).** A backend command runs `hum_to_riff` on a
  synthesized hum and returns the grid-locked notes + the riff samples + bars;
  the UI renders note cards + the waveform from that result. Runs with no device
  (CI/preview-friendly).
- **AC3 — Live path (device).** `record_start` / `record_stop_analyze` capture a
  take via `gooz-audio` and run the pipeline; works on a real machine (verified by
  hand, not a CI gate).
- **AC4 — Playback.** The UI loop-plays the returned riff (Web Audio).
- **AC5 — "What I heard."** Each note shows its frequency ratio (`n:d`), Hz, and
  cents offset — the "you hummed → these notes" view.
- **AC6 — Tested, documented, gates.** The Rust command layer is unit-tested
  without a window; every public item is documented; the four toolchain gates are
  green. (UI look verified by eye / preview.)

## 4. Constraints & non-goals

- Wraps **only R-0008**. **No** session persistence (R-0010), arrangement
  (R-0011), export (R-0012), beat-builder wiring (R-0009), functional ratio
  sliders (a visual stub is fine in v0), or Advanced Mode.
- Frontend is vanilla HTML/CSS/JS (no framework/build step); backend commands are
  thin wrappers over the existing crates — no new DSP/synth logic here.
- `apps/gooz-studio` becomes the Tauri app; its R-0008 pipeline **lib** stays as
  the library the commands call.

## 5. Open questions

None for v0 — settled in the decision log.

## 6. Decision log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-07-01 | Build a **v0 slice** of R-0013 now, in parallel with R-0009 | Owner choice — different crate (`gooz-studio`) than the team's R-0009 (`gooz-synth`), so no collision; delivers the clickable UI the owner has been after. Full R-0013 (wrapping M3) still follows the normal sequence later. |
| 2026-07-01 | Keep a **deviceless `demo_riff` command** beside the live-mic path | Makes the shell testable in CI and previewable without a mic; the live path is by-ear on a real machine (consistent with R-0003/R-0008 demos). |
| 2026-07-01 | **Vanilla** HTML/CSS/JS frontend; **Web Audio** playback | Smallest thing that looks right and runs with no build toolchain; playback in the browser avoids a second audio path in v0. |
| 2026-07-01 | Backend commands are **thin wrappers** over `gooz-studio` lib + `gooz-audio` | The shell adds a seam, not logic; all music code stays in the reviewed crates. |

## Changelog

- 2026-07-01 — created; accepted as an owner-authorized v0 slice of R-0013.
