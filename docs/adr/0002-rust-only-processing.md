# ADR 0002 — All image processing in Rust (no Sharp / Node sidecar)

Status: accepted

**Context.** Sharp would require shipping Node.js as a sidecar process (+40–80 MB, an extra process to manage and a second failure surface).

**Decision.** Processing lives in a pure Rust crate (`crates/ibs-core`, Phase 2) independent of Tauri, so it can be tested with `cargo test` and a CLI without the UI. Only permissively licensed crates (MIT/Apache/BSD). GPL libraries (e.g. libimagequant) are excluded.

**Consequences.** Lossy WebP needs the `webp` (libwebp) crate; AVIF encode uses `ravif`. AVIF *input* (needs dav1d) is deferred to Phase 4.
