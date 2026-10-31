use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid setting `{field}`: {reason}")]
    Invalid { field: &'static str, reason: String },
    #[error("database newer than this app (schema {found}, supported {supported})")]
    UnsupportedSchema { found: i64, supported: i64 },
    #[error("store lock poisoned")]
    Poisoned,
}
