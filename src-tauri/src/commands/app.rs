use tauri::{AppHandle, Manager};

use crate::autostart;
use crate::error::AppResult;

/// Whether Kankulirchi starts in the background when the user logs in.
#[tauri::command]
pub async fn get_launch_at_login(app: AppHandle) -> AppResult<bool> {
    Ok(autostart::is_enabled(&app.path().config_dir()?))
}

#[tauri::command]
pub async fn set_launch_at_login(app: AppHandle, enabled: bool) -> AppResult<bool> {
    let config_dir = app.path().config_dir()?;
    if enabled {
        autostart::enable(&config_dir, &autostart::current_executable()?)?;
    } else {
        autostart::disable(&config_dir)?;
    }
    Ok(autostart::is_enabled(&config_dir))
}

/// Quits completely, even while a rotation is on.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    log::info!("quit from the window");
    app.exit(0);
}
