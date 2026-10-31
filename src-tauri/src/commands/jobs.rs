use crate::error::{CommandError, CommandResult};
use crate::state::AppState;
use ibs_core::{FileResult, Job, JobEvent, PlannedOutput, ScanResult};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

/// Results are forwarded to the UI in batches at most this often (~10 Hz).
const FLUSH_INTERVAL: Duration = Duration::from_millis(100);

/// Expands dropped/selected files and folders off the main thread.
#[tauri::command]
pub async fn scan_inputs(paths: Vec<String>) -> CommandResult<ScanResult> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    tauri::async_runtime::spawn_blocking(move || ibs_core::scan(&paths, &AtomicBool::new(false)))
        .await
        .map_err(|e| CommandError {
            code: "internal",
            message: e.to_string(),
        })
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
    let spawned = std::thread::Builder::new()
        .name("ibs-job".into())
        .spawn(move || {
            let (tx, rx) = mpsc::channel::<FileResult>();
            let forwarder = {
                let channel = on_event.clone();
                std::thread::spawn(move || forward_batched(rx, &channel))
            };
            let outcome = ibs_core::run(&job, concurrency, &cancel, |r| {
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

fn forward_batched(rx: mpsc::Receiver<FileResult>, channel: &Channel<JobEvent>) {
    let mut buf = Vec::new();
    let mut last = Instant::now();
    loop {
        match rx.recv_timeout(FLUSH_INTERVAL) {
            Ok(r) => buf.push(r),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if !buf.is_empty() && last.elapsed() >= FLUSH_INTERVAL {
            let _ = channel.send(JobEvent::Progress {
                results: std::mem::take(&mut buf),
            });
            last = Instant::now();
        }
    }
    if !buf.is_empty() {
        let _ = channel.send(JobEvent::Progress { results: buf });
    }
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
