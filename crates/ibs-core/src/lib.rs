//! Image Batch Studio engine: input discovery, safe output planning,
//! decoding/encoding and batch execution. Independent of Tauri so it can be
//! tested headless.

pub mod codec;
pub mod engine;
pub mod error;
pub mod format;
pub mod intake;
pub mod job;
pub mod output;

pub use engine::{run, JobError, JobEvent, JobSummary};
pub use error::{FileError, FileErrorCode};
pub use format::{ConflictPolicy, ImageFormat};
pub use intake::{scan, InputFile, ScanResult};
pub use job::{FileResult, FileStatus, Job, OutputSpec};
