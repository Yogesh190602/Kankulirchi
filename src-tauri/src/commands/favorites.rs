use tauri::State;

use super::store_wallpaper;
use crate::error::AppResult;
use crate::models::{Wallpaper, WallpaperInput};
use crate::state::AppState;

#[tauri::command]
pub async fn list_favorites(state: State<'_, AppState>) -> AppResult<Vec<Wallpaper>> {
    state.db.list_favorites()
}

/// Saves the wallpaper if needed and marks it as a favorite.
#[tauri::command]
pub async fn add_favorite(state: State<'_, AppState>, wallpaper: WallpaperInput) -> AppResult<Wallpaper> {
    let stored = store_wallpaper(&state, &wallpaper)?;
    let wallpaper_id = stored.id.expect("stored wallpapers have an id");
    state.db.add_favorite(wallpaper_id)?;
    state.db.get_wallpaper(wallpaper_id)
}

#[tauri::command]
pub async fn remove_favorite(state: State<'_, AppState>, wallpaper_id: i64) -> AppResult<Wallpaper> {
    state.db.remove_favorite(wallpaper_id)?;
    state.db.get_wallpaper(wallpaper_id)
}
