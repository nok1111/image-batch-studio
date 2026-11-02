use crate::error::{CommandError, CommandResult};
use crate::license::{License, LicenseStatus};
use crate::state::AppState;
use tauri::State;

fn lic(state: &AppState) -> License<'_> {
    License::load(&state.store, &state.data_dir)
}

/// Current licensing state (trial days left / licensed / expired).
#[tauri::command]
pub fn license_status(state: State<'_, AppState>) -> CommandResult<LicenseStatus> {
    let license = lic(&state);
    // Opportunistic server revalidation; failure keeps the grace window.
    license.refresh();
    Ok(license.status())
}

/// Activate a license key (Lemon Squeezy key or an `IBS1.*` signed key).
#[tauri::command]
pub fn license_activate(state: State<'_, AppState>, key: String) -> CommandResult<LicenseStatus> {
    lic(&state).activate(&key).map_err(|e| CommandError {
        code: "license",
        message: e.to_string(),
    })
}

/// Deactivate the current license on this machine.
#[tauri::command]
pub fn license_deactivate(state: State<'_, AppState>) -> CommandResult<LicenseStatus> {
    lic(&state).deactivate().map_err(|e| CommandError {
        code: "license",
        message: e.to_string(),
    })
}
