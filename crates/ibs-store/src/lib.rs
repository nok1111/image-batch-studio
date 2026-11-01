//! Local persistence for Image Batch Studio (SQLite).
//!
//! All application state that must survive restarts lives here. The frontend
//! never talks SQL; it goes through typed Tauri commands backed by [`Store`].

mod error;
mod migrations;
mod presets;
mod settings;

pub use error::StoreError;
pub use presets::{ResizePreset, WatermarkPreset, MAX_PRESET_NAME};
pub use settings::{ConflictPolicy, ImageFormat, Language, Settings, Theme};

use rusqlite::{Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        Self::from_connection(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut conn: Connection) -> Result<Self, StoreError> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        migrations::run(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Returns the persisted settings. Unknown or malformed stored values fall
    /// back to defaults instead of failing, so a damaged row never bricks the app.
    pub fn settings(&self) -> Result<Settings, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let raw: Option<String> = conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [settings::SETTINGS_KEY],
                |row| row.get(0),
            )
            .optional()?;
        Ok(raw
            .map(|r| Settings::from_json_lenient(&r))
            .unwrap_or_default())
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<Settings, StoreError> {
        settings.validate()?;
        let json = serde_json::to_string(settings)?;
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (settings::SETTINGS_KEY, json),
        )?;
        Ok(settings.clone())
    }

    /// Generic key/value rows (licensing markers) — not part of `Settings`.
    pub fn kv_get(&self, key: &str) -> Result<Option<String>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()
        .map_err(Into::into)
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value),
        )?;
        Ok(())
    }

    pub fn reset_settings(&self) -> Result<Settings, StoreError> {
        // Read + write under one lock/transaction so a concurrent
        // save_settings can never be overwritten by the reset.
        let mut conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let tx = conn.transaction()?;
        let raw: Option<String> = tx
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [settings::SETTINGS_KEY],
                |row| row.get(0),
            )
            .optional()?;
        // Resetting preferences should not re-trigger the first-run wizard.
        let onboarding_done = raw
            .map(|r| Settings::from_json_lenient(&r).onboarding_done)
            .unwrap_or(false);
        let restored = Settings {
            onboarding_done,
            ..Settings::default()
        };
        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (settings::SETTINGS_KEY, serde_json::to_string(&restored)?),
        )?;
        tx.commit()?;
        Ok(restored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_empty() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(store.settings().unwrap(), Settings::default());
    }

    #[test]
    fn settings_round_trip_and_survive_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("ibs.sqlite3");
        let out = dir.path().join("out");
        let s = Settings {
            language: Language::Es,
            default_quality: 72,
            concurrency: 4,
            default_format: ImageFormat::Webp,
            conflict_policy: ConflictPolicy::Skip,
            default_output_dir: Some(out.to_string_lossy().into_owned()),
            ..Default::default()
        };
        {
            let store = Store::open(&db).unwrap();
            store.save_settings(&s).unwrap();
        }
        let store = Store::open(&db).unwrap();
        assert_eq!(store.settings().unwrap(), s);
    }

    #[test]
    fn invalid_settings_are_rejected_and_not_persisted() {
        let store = Store::open_in_memory().unwrap();
        let mut s = Settings {
            default_quality: 0,
            ..Default::default()
        };
        assert!(store.save_settings(&s).is_err());
        s.default_quality = 101;
        assert!(store.save_settings(&s).is_err());
        s = Settings {
            concurrency: 999,
            ..Default::default()
        };
        assert!(store.save_settings(&s).is_err());
        s = Settings {
            default_output_dir: Some("relative/path".into()),
            ..Default::default()
        };
        assert!(store.save_settings(&s).is_err());
        assert_eq!(store.settings().unwrap(), Settings::default());
    }

    #[test]
    fn corrupted_row_falls_back_to_defaults() {
        let store = Store::open_in_memory().unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO settings (key, value) VALUES ('app', '{not json')",
                [],
            )
            .unwrap();
        assert_eq!(store.settings().unwrap(), Settings::default());
    }

    #[test]
    fn reset_restores_defaults() {
        let store = Store::open_in_memory().unwrap();
        let s = Settings {
            default_quality: 50,
            ..Default::default()
        };
        store.save_settings(&s).unwrap();
        assert_eq!(store.reset_settings().unwrap(), Settings::default());
        assert_eq!(store.settings().unwrap(), Settings::default());
    }

    #[test]
    fn migrations_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("ibs.sqlite3");
        drop(Store::open(&db).unwrap());
        drop(Store::open(&db).unwrap());
        let conn = Connection::open(&db).unwrap();
        let v: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(v, migrations::LATEST as i64);
    }
}
