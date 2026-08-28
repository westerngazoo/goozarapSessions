# AGENTS.md

## Cursor Cloud specific instructions

This is a Rust Cargo workspace (edition 2024, requires Rust >= 1.85; the VM ships
current stable). Standard build/test/lint/format commands live in
`project-specifics.md` and `README.md` — use those; they are not duplicated here.

Non-obvious environment notes:

- **Audio requires an ALSA default device.** The audio crate (`gooz-audio`) uses
  `cpal`, which links against ALSA (`libasound2-dev` is a system dependency in
  the VM image). Cloud VMs have no sound card, so a `type null` ALSA default is
  configured at `/etc/asound.conf`. Without it, the `gooz-studio` binaries print
  `audio device unavailable` and exit cleanly (they never panic). With it, they
  run end to end against a silent sink.
- **The two app binaries are by-ear demos, not headless-friendly:**
  - `cargo run -p gooz-studio` (hum→riff) records 4 s from the default *input*.
    On the null device that is silence, so it detects 0 notes and exits — this
    is expected in the cloud, not a failure.
  - `cargo run -p gooz-studio --bin beat` synthesizes and plays a Euclidean drum
    loop; it runs fully against the null device.
- **To exercise the signature hum→riff pipeline without a microphone**, drive
  the library directly (`gooz_studio::hum_to_riff` / `build_beat`) with
  synthetic samples — see the doc example on `hum_to_riff` in
  `apps/gooz-studio/src/pipeline.rs`. The R-0008 acceptance tests do exactly
  this, so `cargo test --workspace` fully covers the core DSP/synth path.
- **Known pre-existing test failure:** `ac6_snap_tie_breaks_to_the_lower_pitch`
  in `crates/gooz-ratio/tests/acceptance_r0001.rs` fails on `main` (a fix is in
  flight on a separate branch). All other tests pass. Do not treat this single
  failure as an environment-setup problem.
