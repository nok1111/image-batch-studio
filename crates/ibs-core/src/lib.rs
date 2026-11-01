//! Image Batch Studio engine: input discovery, safe output planning,
//! decoding/encoding and batch execution. Independent of Tauri so it can be
//! tested headless.

pub mod codec;
pub mod engine;
pub mod error;
pub mod format;
pub mod gif;
pub mod intake;
pub mod job;
pub mod naming;
pub mod output;
pub mod resize;
pub mod transform;
pub mod watermark;

pub use engine::{preview, run, JobError, JobEvent, JobSummary, PlannedAction, PlannedOutput};
pub use error::{FileError, FileErrorCode};
pub use format::{ConflictPolicy, ImageFormat};
pub use intake::{scan, InputFile, ScanResult};
pub use job::{FileResult, FileStatus, FileWarning, Job, MetadataPolicy, OutputMode, OutputSpec};
pub use naming::RenameSpec;
pub use resize::{ResizeMode, ResizeSpec};
pub use transform::{CropSpec, TransformSpec};
pub use watermark::{WatermarkKind, WatermarkPosition, WatermarkSpec};
