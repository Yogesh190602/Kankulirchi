use tauri::{AppHandle, State};

use super::{cache_in_background, store_wallpaper};
use crate::cache::remove_cached_files;
use crate::error::AppResult;
use crate::models::{Collection, CollectionDetail, FreedSpace, Wallpaper, WallpaperInput};
use crate::state::AppState;

fn is_rotating(state: &AppState, collection_id: i64) -> AppResult<bool> {
    Ok(state.db.enabled_rotation()?.is_some_and(|settings| settings.collection_id == collection_id))
}

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> AppResult<Vec<Collection>> {
    state.db.list_collections()
}

#[tauri::command]
pub async fn get_collection(state: State<'_, AppState>, id: i64) -> AppResult<CollectionDetail> {
    Ok(CollectionDetail { collection: state.db.get_collection(id)?, wallpapers: state.db.collection_wallpapers(id)? })
}

#[tauri::command]
pub async fn create_collection(state: State<'_, AppState>, name: String) -> AppResult<Collection> {
    state.db.create_collection(&name)
}

#[tauri::command]
pub async fn rename_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> AppResult<Collection> {
    let collection = state.db.rename_collection(id, &name)?;
    if is_rotating(&state, id)? {
        state.scheduler.settings_changed(&app)?;
    }
    Ok(collection)
}

/// Deletes a collection along with the downloads nothing else uses, and
/// returns the space freed.
#[tauri::command]
pub async fn delete_collection(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<FreedSpace> {
    let was_rotating = is_rotating(&state, id)?;
    let unused = state.db.delete_collection(id)?;
    if was_rotating {
        state.scheduler.settings_changed(&app)?;
    }
    Ok(remove_cached_files(unused).await.unwrap_or_else(|error| {
        log::warn!("could not delete the downloads of collection {id}: {error}");
        FreedSpace::default()
    }))
}

/// Saves the wallpaper if needed, adds it to the collection and starts
/// downloading it in the background.
#[tauri::command]
pub async fn add_to_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    collection_id: i64,
    wallpaper: WallpaperInput,
) -> AppResult<Wallpaper> {
    let stored = store_wallpaper(&state, &wallpaper)?;
    let wallpaper_id = stored.id.expect("stored wallpapers have an id");
    state.db.add_to_collection(collection_id, wallpaper_id)?;
    cache_in_background(&app, &stored);
    Ok(stored)
}

#[tauri::command]
pub async fn remove_from_collection(
    state: State<'_, AppState>,
    collection_id: i64,
    wallpaper_id: i64,
) -> AppResult<()> {
    state.db.remove_from_collection(collection_id, wallpaper_id)
}
