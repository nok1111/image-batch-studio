mod commands;
mod error;
mod state;

use state::AppState;
use tauri::Manager;

const DB_FILE: &str = "ibs.sqlite3";

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = ibs_store::Store::open(&data_dir.join(DB_FILE))?;
            app.manage(AppState {
                store,
                running_job: Default::default(),
                last_output_dir: Default::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::open_output_dir,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::reset_settings,
            commands::jobs::scan_inputs,
            commands::jobs::start_job,
            commands::jobs::cancel_job,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Image Batch Studio");
}
