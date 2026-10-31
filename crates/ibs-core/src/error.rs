use serde::Serialize;
use ts_rs::TS;

/// Stable per-file error codes; the UI localizes by code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FileErrorCode {
    /// Not an image, or an image format this build cannot read.
    Unsupported,
    /// Corrupted or truncated image data.
    Decode,
    /// Dimensions or memory needs exceed the safety limits.
    TooLarge,
    Io,
    Encode,
    /// The written file did not decode back to the expected image.
    Verify,
    /// The output would have replaced one of the batch's source files.
    WouldOverwriteSource,
    /// The output format cannot be produced from this image or in this build.
    OutputUnsupported,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[error("{message}")]
#[ts(export)]
pub struct FileError {
    pub code: FileErrorCode,
    /// English detail for logs/tooltips.
    pub message: String,
}

impl FileError {
    pub fn new(code: FileErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn io(e: std::io::Error) -> Self {
        Self::new(FileErrorCode::Io, e.to_string())
    }
}
