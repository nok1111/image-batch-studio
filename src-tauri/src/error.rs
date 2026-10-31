use serde::Serialize;

/// Error returned to the frontend: a stable machine-readable `code` plus a
/// human-readable `message` (English; the UI localizes by `code`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<ibs_store::StoreError> for CommandError {
    fn from(e: ibs_store::StoreError) -> Self {
        let code = match e {
            ibs_store::StoreError::Invalid { .. } => "invalid_setting",
            _ => "storage",
        };
        Self {
            code,
            message: e.to_string(),
        }
    }
}

pub type CommandResult<T> = Result<T, CommandError>;
