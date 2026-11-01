# Development checklist

## Phase 1 — Foundation
- [x] Project initialized (Vite + React + TS, Tailwind v4, Tauri 2, Cargo workspace)
- [x] Design system tokens (spec palette, dark + light) and base UI components
- [x] Navigation: sidebar with the 10 sections, workspace, bottom status bar
- [x] Dashboard: welcome, quick actions, recent/statistics/favorites (empty states), all tools
- [x] Local settings persisted in SQLite, validated in Rust
- [x] Localization EN / ES / PT-BR with key-parity tests
- [x] CI: lint, typecheck, tests, Windows installer build

## Phase 2 — Core engine
- [x] File/folder selection, drag & drop (recursive scan, dedupe, hidden files ignored)
- [x] Processing queue, background workers, configurable concurrency
- [x] Progress events (batched ~10 Hz), cancellation, per-file status and errors
- [x] Output safety (ADR 0004): temp → verify → atomic rename; sources never targets
- [ ] Thumbnails grid (list view only for now)

## Phase 3 — MVP modules
- [x] Converter: 8 output formats incl. AVIF, lossy/lossless WebP, alpha kept where supported, metadata keep/strip (EXIF orientation applied)
- [x] Compressor: presets, quality, same-as-source format, target size (JPEG/WebP), savings from real output sizes
- [x] Before/after comparison: draggable divider over the original and the encoded result, fit / 100 % zoom (Converter, Compressor, Resizer)
- [x] Resizer: fit / fill / exact / width / height / percent, no-upscale, editable size presets (SQLite)
- [x] Bulk Renamer: templates `{name}` `{n:03}` `{date}`, prefix/suffix, find/replace, preview == result, collisions shown before running; copies to output
- [x] Benchmark command + baseline (docs/BENCHMARK.md); Windows 1000×12 MP figure pending

## Phase 4 — Pro
- [x] Watermark  - [x] Metadata Cleaner  - [x] Crop & Rotate  - [x] Analyzer  - [x] Preset import/export  - [x] AVIF input  - [x] Pipeline (chained ops)

## Phase 5 — Commercialization
- [x] Licensing + 3-day trial (ADR 0005)  - [ ] Signed installer  - [x] Onboarding  - [ ] Docs / landing

## Phase 6 — Testing & release
- [ ] Win10 / Win11  - [ ] Large batches  - [ ] Low memory  - [ ] Corrupted files  - [ ] Offline  - [ ] Install / uninstall
