use crate::error::CommandResult;
use crate::state::AppState;
use ibs_store::{ResizePreset, Settings, WatermarkPreset};
use tauri::State;

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CommandResult<Settings> {
    Ok(state.store.settings()?)
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> CommandResult<Settings> {
    Ok(state.store.save_settings(&settings)?)
}

#[tauri::command]
pub fn reset_settings(state: State<'_, AppState>) -> CommandResult<Settings> {
    Ok(state.store.reset_settings()?)
}

#[tauri::command]
pub fn list_resize_presets(state: State<'_, AppState>) -> CommandResult<Vec<ResizePreset>> {
    Ok(state.store.resize_presets()?)
}

#[tauri::command]
pub fn save_resize_preset(
    state: State<'_, AppState>,
    preset: ResizePreset,
) -> CommandResult<ResizePreset> {
    Ok(state.store.save_resize_preset(&preset)?)
}

#[tauri::command]
pub fn delete_resize_preset(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    Ok(state.store.delete_resize_preset(id)?)
}

#[tauri::command]
pub fn list_watermark_presets(state: State<'_, AppState>) -> CommandResult<Vec<WatermarkPreset>> {
    Ok(state.store.watermark_presets()?)
}

#[tauri::command]
pub fn save_watermark_preset(
    state: State<'_, AppState>,
    preset: WatermarkPreset,
) -> CommandResult<WatermarkPreset> {
    Ok(state.store.save_watermark_preset(&preset)?)
}

#[tauri::command]
pub fn delete_watermark_preset(state: State<'_, AppState>, id: i64) -> CommandResult<()> {
    Ok(state.store.delete_watermark_preset(id)?)
}
