# ADR 0003 — SQLite owned by the Rust side

Status: accepted

**Decision.** `crates/ibs-store` owns the database (`rusqlite`, bundled SQLite, file `ibs.sqlite3` in the OS app-data dir). Schema migrations are an append-only list tracked with `PRAGMA user_version`. The frontend only uses typed commands (`get_settings`, `save_settings`, …); types are generated from Rust with `ts-rs` into `src/lib/ipc/bindings` (`cargo test` regenerates them).

Settings are validated in Rust; a corrupted/unknown stored value falls back to its default instead of failing startup.
