//! Read-only inspection for the Analyzer module: dimensions, format, colour
//! mode, transparency, metadata presence and an estimated WebP size. Writes
//! nothing; mirrors `engine::run` for parallelism and cancellation.

use crate::codec::{self, EncodeOptions};
use crate::{FileError, FileErrorCode, FileStatus, ImageFormat, JobError, JobSummary};
use image::ImageReader;
use rayon::prelude::*;
use serde::Serialize;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;
use ts_rs::TS;

/// Facts about one image, gathered without writing anything.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileAnalysis {
    pub width: u32,
    pub height: u32,
    /// Container format detected from the file's bytes (e.g. "jpeg"), which may
    /// differ from its extension.
    pub format: String,
    /// Colour model of the decoded pixels (e.g. "rgba8").
    pub color: String,
    pub has_alpha: bool,
    /// Multi-frame input of which only the first frame is processed.
    pub animated: bool,
    pub has_exif: bool,
    pub has_icc: bool,
    /// Estimated size as lossy WebP at quality 80 — the compressor's default.
    #[ts(type = "number | null")]
    pub est_webp_bytes: Option<u64>,
}

/// Per-file result; `status` mirrors the batch runner's statuses so the queue
/// UI can reuse them.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnalysisResult {
    /// Index into the paths passed to [`analyze`].
    pub index: u32,
    pub path: String,
    #[ts(type = "number")]
    pub bytes: u64,
    pub status: FileStatus,
    pub analysis: Option<FileAnalysis>,
    pub error: Option<FileError>,
}

/// Events streamed to the UI; same shape as `JobEvent`.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(tag = "type", rename_all = "camelCase")]
#[ts(export)]
pub enum AnalyzeEvent {
    Progress { results: Vec<AnalysisResult> },
    Finished { summary: JobSummary },
    Aborted { message: String },
}

fn analyze_one(path: &Path) -> Result<FileAnalysis, FileError> {
    let container = ImageReader::open(path)
        .map_err(FileError::io)?
        .with_guessed_format()
        .map_err(FileError::io)?
        .format()
        .ok_or_else(|| FileError::new(FileErrorCode::Unsupported, "not a recognized image file"))?;
    let decoded = codec::decode(path)?;
    let color = decoded.image.color();
    let est_webp_bytes = codec::encode(
        &decoded.image,
        &EncodeOptions {
            format: ImageFormat::Webp,
            quality: 80,
            lossless: false,
            icc: None,
            exif: None,
        },
    )
    .ok()
    .map(|e| e.bytes.len() as u64);
    Ok(FileAnalysis {
        width: decoded.image.width(),
        height: decoded.image.height(),
        format: format!("{container:?}").to_lowercase(),
        color: format!("{color:?}").to_lowercase(),
        has_alpha: color.has_alpha(),
        animated: decoded.animated,
        has_exif: decoded.exif.is_some(),
        has_icc: decoded.icc.is_some(),
        est_webp_bytes,
    })
}

/// Inspects every path; `on_result` is called once per input from worker
/// threads, in parallel. `concurrency` 0 means one worker per logical CPU.
pub fn analyze<F>(
    paths: &[PathBuf],
    concurrency: usize,
    cancel: &AtomicBool,
    on_result: F,
) -> Result<JobSummary, JobError>
where
    F: Fn(AnalysisResult) + Sync,
{
    let started = Instant::now();
    let threads = match concurrency {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()),
        n => n,
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("ibs-analyzer-{i}"))
        .build()
        .map_err(|e| JobError::Pool(e.to_string()))?;

    let counts: [AtomicU32; 4] = Default::default();
    pool.install(|| {
        paths.par_iter().enumerate().for_each(|(i, input)| {
            let index = i as u32;
            let base = AnalysisResult {
                index,
                path: input.to_string_lossy().into_owned(),
                bytes: std::fs::metadata(input).map(|m| m.len()).unwrap_or(0),
                status: FileStatus::Done,
                analysis: None,
                error: None,
            };
            let result = if cancel.load(Ordering::Relaxed) {
                AnalysisResult {
                    status: FileStatus::Cancelled,
                    ..base
                }
            } else {
                match catch_unwind(AssertUnwindSafe(|| analyze_one(input))) {
                    Ok(Ok(analysis)) => AnalysisResult {
                        analysis: Some(analysis),
                        ..base
                    },
                    Ok(Err(e)) => AnalysisResult {
                        status: FileStatus::Failed,
                        error: Some(e),
                        ..base
                    },
                    Err(_) => AnalysisResult {
                        status: FileStatus::Failed,
                        error: Some(FileError::new(
                            FileErrorCode::Internal,
                            "unexpected error while analyzing",
                        )),
                        ..base
                    },
                }
            };
            let slot = match result.status {
                FileStatus::Done => 0,
                FileStatus::Skipped => 1,
                FileStatus::Failed => 2,
                FileStatus::Cancelled => 3,
            };
            counts[slot].fetch_add(1, Ordering::Relaxed);
            on_result(result);
        });
    });

    let [done, skipped, failed, cancelled] = counts.map(AtomicU32::into_inner);
    Ok(JobSummary {
        total: paths.len() as u32,
        done,
        skipped,
        failed,
        cancelled,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}
