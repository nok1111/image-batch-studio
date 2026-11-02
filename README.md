<p align="center">
  <img src="assets/imagebatch-logo.png" alt="Image Batch Studio" width="160" />
</p>

# Image Batch Studio

Professional bulk image processing for Windows. 100% local — no cloud, no accounts, no AI/token costs.

Status: **Phase 1 (foundation)**. See [docs/CHECKLIST.md](docs/CHECKLIST.md) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Requirements
- Node.js 20+ and npm
- Rust (stable) — https://rustup.rs
- Windows: WebView2 (preinstalled on Windows 11) and the MSVC build tools
- Linux (development only): `libwebkit2gtk-4.1-dev librsvg2-dev libayatana-appindicator3-dev libxdo-dev libssl-dev`

## Commands
```bash
npm install
npm run tauri dev        # run the desktop app
npm run tauri build      # build installers (NSIS/MSI on Windows)
npm run lint && npm run typecheck && npm test   # frontend checks
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

`cargo test` also regenerates the TypeScript types in `src/lib/ipc/bindings/` from Rust (`ts-rs`).

## Layout
```
src/            React app (app/, components/, features/, i18n/, lib/ipc/, stores/)
src-tauri/      Tauri shell: commands, app state, capabilities
crates/         Rust libraries: ibs-store (SQLite); ibs-core (engine, Phase 2)
docs/           Architecture, ADRs, checklist
```
