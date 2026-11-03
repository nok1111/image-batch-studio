mod commands;
mod error;
mod license;
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
                data_dir: data_dir.clone(),
                store,
                running_job: Default::default(),
                last_output_dir: Default::default(),
                viewable: Default::default(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::open_output_dir,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::reset_settings,
            commands::settings::list_resize_presets,
            commands::settings::save_resize_preset,
            commands::settings::delete_resize_preset,
            commands::settings::list_watermark_presets,
            commands::settings::save_watermark_preset,
            commands::settings::delete_watermark_preset,
            commands::jobs::scan_inputs,
            commands::jobs::pick_watermark_image,
            commands::jobs::register_watermark_image,
            commands::jobs::preview_job,
            commands::jobs::sprite_pack_preview,
            commands::jobs::pick_image,
            commands::jobs::dashboard_data,
            commands::jobs::delete_batch,
            commands::jobs::load_image,
            commands::jobs::start_job,
            commands::jobs::cancel_job,
            commands::license::license_status,
            commands::license::license_activate,
            commands::license::license_deactivate,
            commands::jobs::analyze_files,
            commands::jobs::export_presets,
            commands::jobs::import_presets,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Image Batch Studio");
}
