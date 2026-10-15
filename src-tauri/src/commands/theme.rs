use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::cache::has_local_file;
use crate::error::{AppError, AppResult};
use crate::models::{RotationStatus, WallpaperInput};
use crate::state::AppState;
use crate::theme::backup::BackupSummary;
use crate::theme::palette::{self, Palette};

/// The wallpaper's colors, worked out from its downloaded file if it has
/// one, otherwise from its thumbnail. Saved wallpapers keep them.
#[tauri::command]
pub async fn wallpaper_palette(state: State<'_, AppState>, wallpaper: WallpaperInput) -> AppResult<Palette> {
    state.providers.validate_input(&wallpaper)?;
    let stored = state.db.find_wallpaper(&wallpaper.provider, &wallpaper.provider_id)?;
    let id = stored.as_ref().and_then(|stored| stored.id);
    if let Some(palette) = id.map(|id| state.db.palette(id)).transpose()?.flatten() {
        return Ok(palette);
    }

    let from_file = match stored.filter(has_local_file).and_then(|stored| stored.local_path) {
        Some(path) => palette::from_file(PathBuf::from(path)).await.ok(),
        None => None,
    };
    let palette = match from_file {
        Some(palette) => palette,
        None => palette::from_bytes(state.downloader.fetch_thumbnail(&wallpaper.thumbnail_url).await?).await?,
    };
    if let Some(id) = id {
        state.db.set_palette(id, &palette)?;
    }
    Ok(palette)
}

/// What restoring the desktop would set, or `None` before Kankulirchi has
/// changed the desktop.
#[tauri::command]
pub async fn get_desktop_backup(state: State<'_, AppState>) -> AppResult<Option<BackupSummary>> {
    Ok(state.backup.load()?.map(|backup| backup.summary()))
}

/// Stops rotation, then puts back the desktop settings saved before
/// Kankulirchi first changed them.
#[tauri::command]
pub async fn restore_desktop(app: AppHandle, state: State<'_, AppState>) -> AppResult<RotationStatus> {
    if state.backup.load()?.is_none() {
        return Err(AppError::NotFound(
            "Kankulirchi has not changed your desktop yet, so there is nothing to restore.".into(),
        ));
    }
    let status = state.scheduler.stop_after_current_change(&app).await?;
    state.backup.restore().await?;
    Ok(status)
}
