use serde::Serialize;
use tauri::AppHandle;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub tauri_version: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    pub logical_cpus: usize,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    let pkg = app.package_info();
    AppInfo {
        name: pkg.name.clone(),
        version: pkg.version.to_string(),
        tauri_version: tauri::VERSION,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        logical_cpus: std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1),
    }
}

/// Opens the configured default output folder in the system file manager.
/// The path comes from persisted settings, never from the frontend, so the
/// webview cannot ask the OS to open arbitrary paths.
#[tauri::command]
pub fn open_output_dir(
    app: AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
) -> crate::error::CommandResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir =
        state
            .store
            .settings()?
            .default_output_dir
            .ok_or_else(|| crate::error::CommandError {
                code: "no_output_dir",
                message: "no default output folder is configured".into(),
            })?;
    if !std::path::Path::new(&dir).is_dir() {
        return Err(crate::error::CommandError {
            code: "output_dir_missing",
            message: format!("folder does not exist: {dir}"),
        });
    }
    app.opener()
        .open_path(dir, None::<&str>)
        .map_err(|e| crate::error::CommandError {
            code: "open_failed",
            message: e.to_string(),
        })
}
