# goozarapSessions — Easy Mode desktop shell (Tauri, R-0013 v0)

The clickable dark-neon shell over the R-0008 hum→riff pipeline. This crate is
**excluded from the workspace merge gate** (it pulls the webview runtime); build
it on a desktop.

## Run

```sh
cd apps/gooz-studio/src-tauri
cargo run            # first build downloads the Tauri runtime — a few minutes
```

No Tauri CLI is required (the frontend is static files in `../ui`). For
hot-reload dev, `cargo tauri dev` also works if you have `cargo-tauri` installed.

Requires the system WebView (WKWebView on macOS — already present with Xcode).

## What it wraps

- `demo_riff` → `gooz_studio::demo_riff()` (synthetic hum; no mic).
- `record_start` / `record_stop_analyze` → `gooz_audio` capture →
  `gooz_studio::riff_from_take()`.

All music logic lives in the reviewed `gooz-*` crates; this crate is only the
Rust↔web bridge. Frontend: `../ui/{index.html,style.css,main.js}`.
