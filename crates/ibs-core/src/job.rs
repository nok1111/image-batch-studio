use crate::transform::TransformSpec;
use crate::{ConflictPolicy, FileError, ImageFormat, RenameSpec, ResizeSpec};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which encoding each output gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", content = "format", rename_all = "camelCase")]
#[ts(export)]
pub enum OutputMode {
    /// Re-encode every file to this format.
    Convert(ImageFormat),
    /// Re-encode in the format given by the input's extension (Compressor, Resizer).
    SameAsSource,
    /// Copy the original bytes unchanged (Renamer). Not combinable with resize.
    CopyOriginal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MetadataPolicy {
    /// No EXIF/ICC in outputs. EXIF orientation is applied to the pixels.
    #[default]
    Strip,
    /// Copy EXIF and ICC profile where the output format can carry them.
    Keep,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OutputSpec {
    /// Absolute destination folder; created if missing.
    pub dir: String,
    pub mode: OutputMode,
    /// 1..=100, used by lossy encoders (JPEG, lossy WebP, AVIF).
    pub quality: u8,
    /// WebP only: lossless instead of lossy.
    pub lossless: bool,
    pub metadata: MetadataPolicy,
    /// Lossy JPEG/WebP only: highest quality ≤ `quality` whose output fits.
    #[ts(type = "number | null")]
    pub target_size: Option<u64>,
    pub conflict: ConflictPolicy,
}

/// A batch: every input goes through the same processing and output spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Job {
    pub inputs: Vec<String>,
    pub output: OutputSpec,
    /// Crop / rotate / flip applied before resize.
    pub transform: Option<TransformSpec>,
    pub resize: Option<ResizeSpec>,
    /// Output file names; without it outputs keep the input's name.
    pub rename: Option<RenameSpec>,
}

/// Non-fatal notice attached to a successful result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FileWarning {
    /// Animated input: only the first frame was exported.
    FirstFrameOnly,
    /// Metadata was requested but the output format cannot carry (all of) it.
    MetadataDropped,
    /// Even the lowest quality is above the target size.
    TargetNotReached,
    /// Target size ignored: the output format is not lossy.
    TargetSizeUnsupported,
    /// Re-encoded file is larger than the original.
    LargerThanSource,
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
    pub warnings: Vec<FileWarning>,
}
