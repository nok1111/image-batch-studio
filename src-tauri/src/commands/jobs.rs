use crate::error::{CommandError, CommandResult};
use crate::state::AppState;
use ibs_core::{
    AnalysisResult, AnalyzeEvent, FileResult, FileStatus, Job, JobEvent, PlannedOutput, ScanResult,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

/// Results are forwarded to the UI in batches at most this often (~10 Hz).
const FLUSH_INTERVAL: Duration = Duration::from_millis(100);

/// Expands dropped/selected files and folders off the main thread.
#[tauri::command]
pub async fn scan_inputs(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> CommandResult<ScanResult> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let scanned = tauri::async_runtime::spawn_blocking(move || {
        ibs_core::scan(&paths, &AtomicBool::new(false))
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: e.to_string(),
    })?;
    // Only files the scan accepted as supported inputs may be shown
    // (crop editor, comparison) — not arbitrary paths the caller passes.
    if let Ok(mut viewable) = state.viewable.lock() {
        viewable.extend(scanned.files.iter().map(|f| PathBuf::from(&f.path)));
    }
    Ok(scanned)
}

/// Output names a batch would produce (renamer preview, collision check).
#[tauri::command]
pub async fn preview_job(job: Job) -> CommandResult<Vec<PlannedOutput>> {
    tauri::async_runtime::spawn_blocking(move || ibs_core::preview(&job))
        .await
        .map_err(|e| CommandError {
            code: "internal",
            message: e.to_string(),
        })?
        .map_err(Into::into)
}

/// Raw bytes of an image for the before/after comparison view.
#[tauri::command]
pub async fn load_image(
    state: State<'_, AppState>,
    path: String,
) -> CommandResult<tauri::ipc::Response> {
    let allowed = state
        .viewable
        .lock()
        .map_err(|_| poisoned())?
        .contains(std::path::Path::new(&path));
    if !allowed {
        return Err(CommandError {
            code: "image",
            message: "not part of the latest batch".into(),
        });
    }
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        ibs_core::codec::display_bytes(std::path::Path::new(&path))
    })
    .await
    .map_err(|e| CommandError {
        code: "internal",
        message: e.to_string(),
    })?
    .map_err(|e| CommandError {
        code: "image",
        message: e.message,
    })?;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Lets the user pick a logo file for the Watermark module: opens the dialog,
/// verifies the file decodes as an image, then whitelists it for `load_image`.
#[tauri::command]
pub fn pick_watermark_image(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CommandResult<Option<String>> {
    let picked = app
        .dialog()
        .file()
        .add_filter(
            "Images",
            &[
                "png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff", "avif",
            ],
        )
        .blocking_pick_file();
    let Some(file) = picked else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| CommandError {
        code: "image",
        message: e.to_string(),
    })?;
    ibs_core::codec::decode(&path).map_err(|e| CommandError {
        code: "image",
        message: e.message,
    })?;
    let path = std::fs::canonicalize(&path).unwrap_or(path);
    // Record dialog-picked logos: register_watermark_image trusts only
    // these (never webview-supplied preset contents).
    state.store.add_watermark_image(&path.to_string_lossy())?;
    state
        .viewable
        .lock()
        .map_err(|_| CommandError {
            code: "internal",
            message: "state lock poisoned".into(),
        })?
        .insert(path.clone());
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Re-whitelists a watermark logo for `load_image`. Dialog-picked paths
/// survive app restarts and job starts (which rebuild `viewable`), so
/// the preview registers them on demand. Only paths recorded by
/// `pick_watermark_image` are trusted — webview-supplied preset contents
/// are not — and the file still has to decode as an image.
#[tauri::command]
pub fn register_watermark_image(state: State<'_, AppState>, path: String) -> CommandResult<()> {
    // Stored paths are already canonical (canonicalized at pick time), so
    // compare verbatim: canonicalizing both sides would also accept a
    // picked path that was later retargeted via symlink.
    let path = std::fs::canonicalize(PathBuf::from(&path)).unwrap_or_else(|_| PathBuf::from(&path));
    let allowed = state
        .store
        .watermark_image_paths()?
        .iter()
        .any(|p| std::path::Path::new(p).as_os_str() == path.as_os_str());
    if !allowed {
        return Err(CommandError {
            code: "image",
            message: "path was not picked through the file dialog".into(),
        });
    }
    ibs_core::codec::decode(&path).map_err(|e| CommandError {
        code: "image",
        message: e.message,
    })?;
    state
        .viewable
        .lock()
        .map_err(|_| CommandError {
            code: "internal",
            message: "state lock poisoned".into(),
        })?
        .insert(path);
    Ok(())
}

/// Starts a batch in the background and streams `JobEvent`s on `on_event`.
#[tauri::command]
pub fn start_job(
    app: AppHandle,
    state: State<'_, AppState>,
    job: Job,
    on_event: Channel<JobEvent>,
) -> CommandResult<()> {
    let concurrency = state.store.settings()?.concurrency as usize;
    let output_dir = ibs_core::engine::prepare(&job)?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.running_job.lock().map_err(|_| poisoned())?;
        if slot.is_some() {
            return Err(CommandError {
                code: "busy",
                message: "a batch is already running".into(),
            });
        }
        *slot = Some(cancel.clone());
    }
    if let Ok(mut last) = state.last_output_dir.lock() {
        *last = Some(output_dir);
    }
    if let Ok(mut viewable) = state.viewable.lock() {
        *viewable = job.inputs.iter().map(PathBuf::from).collect();
    }
    let spawned = std::thread::Builder::new()
        .name("ibs-job".into())
        .spawn(move || {
            let (tx, rx) = mpsc::channel::<FileResult>();
            let forwarder = {
                let channel = on_event.clone();
                std::thread::spawn(move || {
                    forward_batched(rx, &channel, |results| JobEvent::Progress { results })
                })
            };
            let outcome = ibs_core::run(&job, concurrency, &cancel, |r| {
                if let (FileStatus::Done, Some(out)) = (&r.status, &r.output) {
                    if let Ok(mut viewable) = app.state::<AppState>().viewable.lock() {
                        viewable.insert(PathBuf::from(out));
                    }
                }
                let _ = tx.send(r);
            });
            drop(tx);
            let _ = forwarder.join();
            let _ = on_event.send(match outcome {
                Ok(summary) => JobEvent::Finished { summary },
                Err(e) => JobEvent::Aborted {
                    message: e.to_string(),
                },
            });
            if let Ok(mut slot) = app.state::<AppState>().running_job.lock() {
                *slot = None;
            }
        });
    if let Err(e) = spawned {
        if let Ok(mut slot) = state.running_job.lock() {
            *slot = None;
        }
        return Err(CommandError {
            code: "internal",
            message: e.to_string(),
        });
    }
    Ok(())
}

fn forward_batched<T, E>(rx: mpsc::Receiver<T>, channel: &Channel<E>, wrap: impl Fn(Vec<T>) -> E)
where
    E: serde::Serialize + Clone + Send + 'static,
{
    let mut buf = Vec::new();
    let mut last = Instant::now();
    loop {
        match rx.recv_timeout(FLUSH_INTERVAL) {
            Ok(r) => buf.push(r),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if !buf.is_empty() && last.elapsed() >= FLUSH_INTERVAL {
            let _ = channel.send(wrap(std::mem::take(&mut buf)));
            last = Instant::now();
        }
    }
    if !buf.is_empty() {
        let _ = channel.send(wrap(buf));
    }
}

/// Inspects queued images without writing anything: streams `AnalyzeEvent`s.
/// Shares the single-job slot with `start_job`, so `cancel_job` stops it too.
#[tauri::command]
pub fn analyze_files(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
    on_event: Channel<AnalyzeEvent>,
) -> CommandResult<()> {
    let concurrency = state.store.settings()?.concurrency as usize;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state.running_job.lock().map_err(|_| poisoned())?;
        if slot.is_some() {
            return Err(CommandError {
                code: "busy",
                message: "a batch is already running".into(),
            });
        }
        *slot = Some(cancel.clone());
    }
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    let spawned = std::thread::Builder::new()
        .name("ibs-analyze".into())
        .spawn(move || {
            let (tx, rx) = mpsc::channel::<AnalysisResult>();
            let forwarder = {
                let channel = on_event.clone();
                std::thread::spawn(move || {
                    forward_batched(rx, &channel, |results| AnalyzeEvent::Progress { results })
                })
            };
            let outcome = ibs_core::analyze(&paths, concurrency, &cancel, |r| {
                let _ = tx.send(r);
            });
            drop(tx);
            let _ = forwarder.join();
            let _ = on_event.send(match outcome {
                Ok(summary) => AnalyzeEvent::Finished { summary },
                Err(e) => AnalyzeEvent::Aborted {
                    message: e.to_string(),
                },
            });
            if let Ok(mut slot) = app.state::<AppState>().running_job.lock() {
                *slot = None;
            }
        });
    if let Err(e) = spawned {
        if let Ok(mut slot) = state.running_job.lock() {
            *slot = None;
        }
        return Err(CommandError {
            code: "internal",
            message: e.to_string(),
        });
    }
    Ok(())
}

const PRESETS_MAX_BYTES: u64 = 4 * 1024 * 1024;

/// Opens the save dialog itself and writes the presets JSON to the path the
/// user picks — the webview never supplies a path. `None` = cancelled.
#[tauri::command]
pub fn export_presets(app: AppHandle, contents: String) -> CommandResult<Option<String>> {
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("Presets JSON", &["json"])
        .set_file_name("image-batch-studio-presets.json")
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| CommandError {
        code: "io",
        message: e.to_string(),
    })?;
    std::fs::write(&path, contents).map_err(|e| CommandError {
        code: "io",
        message: e.to_string(),
    })?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Opens the picker itself and returns the chosen presets JSON — the webview
/// never supplies a path. `None` = cancelled.
#[tauri::command]
pub fn import_presets(app: AppHandle) -> CommandResult<Option<String>> {
    let Some(file) = app
        .dialog()
        .file()
        .add_filter("Presets JSON", &["json"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = file.into_path().map_err(|e| CommandError {
        code: "io",
        message: e.to_string(),
    })?;
    // Enforce the cap before allocating the whole file in memory.
    let len = std::fs::metadata(&path)
        .map_err(|e| CommandError {
            code: "io",
            message: e.to_string(),
        })?
        .len();
    if len > PRESETS_MAX_BYTES {
        return Err(CommandError {
            code: "io",
            message: "file too large to be a presets file".into(),
        });
    }
    let data = std::fs::read(&path).map_err(|e| CommandError {
        code: "io",
        message: e.to_string(),
    })?;
    String::from_utf8(data).map(Some).map_err(|_| CommandError {
        code: "io",
        message: "not a UTF-8 presets file".into(),
    })
}

/// Requests cancellation of the running batch. Returns false if none is running.
#[tauri::command]
pub fn cancel_job(state: State<'_, AppState>) -> CommandResult<bool> {
    let slot = state.running_job.lock().map_err(|_| poisoned())?;
    Ok(slot.as_ref().is_some_and(|c| {
        c.store(true, Ordering::Relaxed);
        true
    }))
}

fn poisoned() -> CommandError {
    CommandError {
        code: "internal",
        message: "state lock poisoned".into(),
    }
}
