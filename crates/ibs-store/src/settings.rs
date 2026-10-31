use crate::StoreError;
use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

pub(crate) const SETTINGS_KEY: &str = "app";
pub const MAX_CONCURRENCY: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[ts(export)]
pub enum Language {
    /// Follow the operating system language, falling back to English.
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en")]
    En,
    #[serde(rename = "es")]
    Es,
    #[serde(rename = "pt-BR")]
    PtBr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ImageFormat {
    Png,
    #[default]
    Jpeg,
    Webp,
    Avif,
    Bmp,
    Tiff,
    Gif,
    Ico,
}

/// What to do when an output file already exists. Source files are never
/// overwritten regardless of this policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum ConflictPolicy {
    #[default]
    Rename,
    Skip,
    Overwrite,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(default, rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    pub theme: Theme,
    pub language: Language,
    pub default_output_dir: Option<String>,
    pub default_format: ImageFormat,
    /// 1..=100
    pub default_quality: u8,
    /// 0 = automatic (number of logical CPUs).
    pub concurrency: u32,
    pub conflict_policy: ConflictPolicy,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            language: Language::default(),
            default_output_dir: None,
            default_format: ImageFormat::default(),
            default_quality: 85,
            concurrency: 0,
            conflict_policy: ConflictPolicy::default(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), StoreError> {
        if !(1..=100).contains(&self.default_quality) {
            return Err(StoreError::Invalid {
                field: "defaultQuality",
                reason: "must be between 1 and 100".into(),
            });
        }
        if self.concurrency > MAX_CONCURRENCY {
            return Err(StoreError::Invalid {
                field: "concurrency",
                reason: format!("must be between 0 (auto) and {MAX_CONCURRENCY}"),
            });
        }
        if let Some(dir) = &self.default_output_dir {
            if dir.trim().is_empty() || !Path::new(dir).is_absolute() {
                return Err(StoreError::Invalid {
                    field: "defaultOutputDir",
                    reason: "must be an absolute path".into(),
                });
            }
        }
        Ok(())
    }

    /// Parses stored JSON field by field: unknown/invalid fields take their
    /// default value, and an invalid result as a whole yields defaults.
    pub(crate) fn from_json_lenient(raw: &str) -> Self {
        let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(raw)
        else {
            return Self::default();
        };
        let mut merged = serde_json::to_value(Self::default()).expect("defaults serialize");
        if let serde_json::Value::Object(base) = &mut merged {
            for (k, v) in map {
                if base.contains_key(&k) {
                    let mut probe = base.clone();
                    probe.insert(k.clone(), v.clone());
                    if serde_json::from_value::<Self>(serde_json::Value::Object(probe)).is_ok() {
                        base.insert(k, v);
                    }
                }
            }
        }
        serde_json::from_value::<Self>(merged)
            .ok()
            .filter(|s| s.validate().is_ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lenient_parse_keeps_valid_fields_and_drops_invalid_ones() {
        let s = Settings::from_json_lenient(
            r#"{"language":"es","theme":"neon","defaultQuality":70,"unknown":1}"#,
        );
        assert_eq!(s.language, Language::Es);
        assert_eq!(s.theme, Theme::Dark);
        assert_eq!(s.default_quality, 70);
    }

    #[test]
    fn lenient_parse_rejects_out_of_range_values() {
        let s = Settings::from_json_lenient(r#"{"defaultQuality":0}"#);
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn serializes_language_codes() {
        assert_eq!(serde_json::to_string(&Language::PtBr).unwrap(), "\"pt-BR\"");
    }
}
