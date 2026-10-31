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
- [ ] Converter  - [ ] Compressor  - [ ] Resizer  - [ ] Bulk Renamer

## Phase 4 — Pro
- [ ] Watermark  - [ ] Metadata Cleaner  - [ ] Crop & Rotate  - [ ] Analyzer  - [ ] Preset import/export  - [ ] AVIF input

## Phase 5 — Commercialization
- [ ] Licensing + 3-day trial (ADR 0005)  - [ ] Signed installer  - [ ] Onboarding  - [ ] Docs / landing

## Phase 6 — Testing & release
- [ ] Win10 / Win11  - [ ] Large batches  - [ ] Low memory  - [ ] Corrupted files  - [ ] Offline  - [ ] Install / uninstall
