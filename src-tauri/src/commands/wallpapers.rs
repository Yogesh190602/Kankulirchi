use tauri::{AppHandle, State};

use super::store_wallpaper;
use crate::cache::has_local_file;
use crate::error::AppResult;
use crate::models::{HistoryEntry, Wallpaper, WallpaperInput, WallpaperStatus};
use crate::state::AppState;
use crate::wallpaper;

const MAX_PAGE_SIZE: u32 = 200;

/// Library state of a wallpaper (stored, favorite, cached, collections),
/// looked up by its provider id so it works for unsaved search results.
#[tauri::command]
pub async fn get_wallpaper_status(
    state: State<'_, AppState>,
    provider: String,
    provider_id: String,
) -> AppResult<WallpaperStatus> {
    state.db.wallpaper_status(&provider, &provider_id)
}

/// Downloads the wallpaper into the local cache. Already cached wallpapers
/// are returned without downloading again.
#[tauri::command]
pub async fn download_wallpaper(state: State<'_, AppState>, wallpaper: WallpaperInput) -> AppResult<Wallpaper> {
    let stored = store_wallpaper(&state, &wallpaper)?;
    let wallpaper_id = stored.id.expect("stored wallpapers have an id");
    let cache_dir = state.settings.cache_dir();
    let cached = state.downloader.ensure_cached(&state.db, &cache_dir, wallpaper_id).await?;
    if !has_local_file(&stored) {
        state.report_download(&cached);
    }
    Ok(cached)
}

/// Sets the wallpaper as the GNOME desktop background right away.
#[tauri::command]
pub async fn apply_wallpaper(
    app: AppHandle,
    state: State<'_, AppState>,
    wallpaper: WallpaperInput,
) -> AppResult<HistoryEntry> {
    let stored = store_wallpaper(&state, &wallpaper)?;
    let wallpaper_id = stored.id.expect("stored wallpapers have an id");
    wallpaper::apply_wallpaper(&app, wallpaper_id).await
}

/// Every applied wallpaper, newest first.
#[tauri::command]
pub async fn list_history(state: State<'_, AppState>, limit: u32, offset: u32) -> AppResult<Vec<HistoryEntry>> {
    state.db.list_history(limit.clamp(1, MAX_PAGE_SIZE), offset)
}

/// Recently applied wallpapers without repeats, newest first.
#[tauri::command]
pub async fn recent_wallpapers(state: State<'_, AppState>, limit: u32) -> AppResult<Vec<HistoryEntry>> {
    state.db.recent_wallpapers(limit.clamp(1, MAX_PAGE_SIZE))
}
