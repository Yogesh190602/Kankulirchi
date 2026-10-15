use tauri::{AppHandle, State};

use super::cache_in_background_in_order;
use crate::error::{AppError, AppResult};
use crate::models::{
    validate_interval, RotationMode, RotationSettings, RotationStatus, SearchRotation, WallpaperInput,
};
use crate::state::AppState;

/// Most search results one request can save for rotation.
const MAX_SEARCH_ROTATION_WALLPAPERS: usize = 500;

#[tauri::command]
pub async fn get_rotation_status(state: State<'_, AppState>) -> AppResult<RotationStatus> {
    state.scheduler.status(&state.db)
}

#[tauri::command]
pub async fn get_rotation_settings(
    state: State<'_, AppState>,
    collection_id: i64,
) -> AppResult<Option<RotationSettings>> {
    state.db.rotation_settings(collection_id)
}

/// Remembers the interval and mode for a collection. If the collection is
/// rotating, the new settings take effect immediately.
#[tauri::command]
pub async fn save_rotation_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    collection_id: i64,
    interval_seconds: u32,
    rotation_mode: RotationMode,
) -> AppResult<RotationSettings> {
    validate_interval(interval_seconds)?;
    let settings = state.db.save_rotation_settings(collection_id, interval_seconds, rotation_mode)?;
    if settings.enabled {
        state.scheduler.settings_changed(&app)?;
    }
    Ok(settings)
}

#[tauri::command]
pub async fn start_rotation(
    app: AppHandle,
    state: State<'_, AppState>,
    collection_id: i64,
    interval_seconds: u32,
    rotation_mode: RotationMode,
) -> AppResult<RotationStatus> {
    state.scheduler.start(&app, collection_id, interval_seconds, rotation_mode).await
}

/// Saves search results to the collection called `name` (created if no
/// collection has that name) and rotates it. The collection is downloaded
/// in the background so rotation keeps working offline.
#[tauri::command]
pub async fn start_search_rotation(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    wallpapers: Vec<WallpaperInput>,
    interval_seconds: u32,
    rotation_mode: RotationMode,
) -> AppResult<SearchRotation> {
    validate_interval(interval_seconds)?;
    if wallpapers.is_empty() {
        return Err(AppError::Validation("Search for wallpapers to rotate first.".into()));
    }
    if wallpapers.len() > MAX_SEARCH_ROTATION_WALLPAPERS {
        return Err(AppError::Validation(format!(
            "Up to {MAX_SEARCH_ROTATION_WALLPAPERS} wallpapers can be rotated at a time."
        )));
    }
    for wallpaper in &wallpapers {
        state.providers.validate_input(wallpaper)?;
    }

    let collection = state.db.save_to_collection(&name, &wallpapers)?;
    // Start first, so the first wallpaper downloads alone and shows sooner.
    let status = state.scheduler.start(&app, collection.id, interval_seconds, rotation_mode).await?;
    cache_in_background_in_order(&app, collection.id, state.db.collection_wallpapers(collection.id)?);
    Ok(SearchRotation { collection, status })
}

#[tauri::command]
pub async fn stop_rotation(app: AppHandle, state: State<'_, AppState>) -> AppResult<RotationStatus> {
    state.scheduler.stop(&app)
}
