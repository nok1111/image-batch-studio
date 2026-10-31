use crate::{Store, StoreError};
use ibs_core::ResizeSpec;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const MAX_PRESET_NAME: usize = 60;

/// Editable resize target (social platforms and custom sizes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResizePreset {
    /// `None` when creating a new preset.
    #[ts(type = "number | null")]
    pub id: Option<i64>,
    pub name: String,
    pub spec: ResizeSpec,
}

impl Store {
    /// Presets in display order; rows with a malformed spec are skipped.
    pub fn resize_presets(&self) -> Result<Vec<ResizePreset>, StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let mut stmt =
            conn.prepare("SELECT id, name, spec FROM resize_presets ORDER BY position, id")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, spec) = row?;
            if let Ok(spec) = serde_json::from_str::<ResizeSpec>(&spec) {
                out.push(ResizePreset {
                    id: Some(id),
                    name,
                    spec,
                });
            }
        }
        Ok(out)
    }

    /// Inserts (no id) or updates a preset; returns it with its id.
    pub fn save_resize_preset(&self, preset: &ResizePreset) -> Result<ResizePreset, StoreError> {
        let name = preset.name.trim().to_string();
        if name.is_empty() || name.chars().count() > MAX_PRESET_NAME {
            return Err(StoreError::Invalid {
                field: "name",
                reason: format!("must be 1–{MAX_PRESET_NAME} characters"),
            });
        }
        preset
            .spec
            .validate()
            .map_err(|reason| StoreError::Invalid {
                field: "spec",
                reason,
            })?;
        let spec = serde_json::to_string(&preset.spec)?;
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        let id = match preset.id {
            Some(id) => {
                let n = conn.execute(
                    "UPDATE resize_presets SET name = ?1, spec = ?2 WHERE id = ?3",
                    (&name, &spec, id),
                )?;
                if n == 0 {
                    return Err(StoreError::Invalid {
                        field: "id",
                        reason: "preset not found".into(),
                    });
                }
                id
            }
            None => {
                conn.execute(
                    "INSERT INTO resize_presets (name, spec, position)
                     VALUES (?1, ?2, (SELECT COALESCE(MAX(position), -1) + 1 FROM resize_presets))",
                    (&name, &spec),
                )?;
                conn.last_insert_rowid()
            }
        };
        Ok(ResizePreset {
            id: Some(id),
            name,
            spec: preset.spec,
        })
    }

    pub fn delete_resize_preset(&self, id: i64) -> Result<(), StoreError> {
        let conn = self.conn.lock().map_err(|_| StoreError::Poisoned)?;
        conn.execute("DELETE FROM resize_presets WHERE id = ?1", [id])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ibs_core::ResizeMode;

    fn spec(w: u32) -> ResizeSpec {
        ResizeSpec {
            mode: ResizeMode::Fit,
            width: w,
            height: w,
            percent: 100,
            allow_upscale: false,
        }
    }

    #[test]
    fn seeded_presets_are_valid() {
        let store = Store::open_in_memory().unwrap();
        let presets = store.resize_presets().unwrap();
        assert_eq!(presets.len(), 9);
        assert!(presets.iter().all(|p| p.spec.validate().is_ok()));
        assert_eq!(presets[0].spec.width, 1080);
    }

    #[test]
    fn crud_round_trip() {
        let store = Store::open_in_memory().unwrap();
        let p = store
            .save_resize_preset(&ResizePreset {
                id: None,
                name: "  Mine ".into(),
                spec: spec(500),
            })
            .unwrap();
        assert_eq!(p.name, "Mine");
        let all = store.resize_presets().unwrap();
        assert_eq!(all.last().unwrap(), &p);
        let edited = store
            .save_resize_preset(&ResizePreset {
                spec: spec(640),
                ..p.clone()
            })
            .unwrap();
        assert_eq!(store.resize_presets().unwrap().last().unwrap(), &edited);
        store.delete_resize_preset(p.id.unwrap()).unwrap();
        assert_eq!(store.resize_presets().unwrap().len(), 9);
    }

    #[test]
    fn invalid_presets_are_rejected() {
        let store = Store::open_in_memory().unwrap();
        let bad = [
            ResizePreset {
                id: None,
                name: " ".into(),
                spec: spec(10),
            },
            ResizePreset {
                id: None,
                name: "x".repeat(61),
                spec: spec(10),
            },
            ResizePreset {
                id: None,
                name: "zero".into(),
                spec: spec(0),
            },
            ResizePreset {
                id: Some(9999),
                name: "missing".into(),
                spec: spec(10),
            },
        ];
        for p in bad {
            assert!(store.save_resize_preset(&p).is_err(), "{p:?}");
        }
        assert_eq!(store.resize_presets().unwrap().len(), 9);
    }
}
