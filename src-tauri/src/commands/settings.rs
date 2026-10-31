use crate::error::CommandResult;
use crate::state::AppState;
use ibs_store::Settings;
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
