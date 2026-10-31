use crate::{ConflictPolicy, FileError, ImageFormat};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OutputSpec {
    /// Absolute destination folder; created if missing.
    pub dir: String,
    pub format: ImageFormat,
    /// 1..=100, used by lossy encoders.
    pub quality: u8,
    pub conflict: ConflictPolicy,
}

/// A batch: every input goes through the same processing and output spec.
/// Image operations (resize, crop, …) join this in later phases.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Job {
    pub inputs: Vec<String>,
    pub output: OutputSpec,
}

/// Non-fatal notice attached to a successful result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FileWarning {
    /// Animated input: only the first frame was exported.
    FirstFrameOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FileStatus {
    Done,
    /// Output already existed and the policy is `skip`.
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileResult {
    /// Index into `Job::inputs`.
    pub index: u32,
    pub status: FileStatus,
    pub output: Option<String>,
    #[ts(type = "number | null")]
    pub output_size: Option<u64>,
    pub error: Option<FileError>,
    pub warning: Option<FileWarning>,
}
