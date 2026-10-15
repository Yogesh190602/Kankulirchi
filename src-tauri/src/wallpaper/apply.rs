//! Applies wallpapers to the GNOME desktop through `gsettings`.

use std::path::Path;

use tauri::{AppHandle, Emitter, Manager};
use url::Url;

use crate::error::{AppError, AppResult};
use crate::events;
use crate::gnome::{self, gvariant_string};
use crate::models::HistoryEntry;
use crate::repeats;
use crate::state::AppState;

const BACKGROUND_SCHEMA: &str = "org.gnome.desktop.background";

/// Downloads the wallpaper if needed, sets it as the GNOME background,
/// records it in the history and notifies the frontend. The first time a
/// wallpaper is applied, its provider is told it is being used.
pub async fn apply_wallpaper(app: &AppHandle, wallpaper_id: i64) -> AppResult<HistoryEntry> {
    let state = app.state::<AppState>();
    let cache_dir = state.settings.cache_dir();
    let wallpaper = state.downloader.ensure_cached(&state.db, &cache_dir, wallpaper_id).await?;
    let local_path = wallpaper
        .local_path
        .as_deref()
        .ok_or_else(|| AppError::Internal("The wallpaper has no local file after downloading.".into()))?;

    // The desktop as it was before Kankulirchi first changed it, so it can
    // be restored. Not being able to save it does not stop the change.
    let download_dirs = [state.settings.default_cache_dir().to_path_buf(), cache_dir.clone()];
    if let Err(error) = state.backup.ensure(&download_dirs).await {
        log::warn!("could not save the desktop settings before changing them: {error}");
    }
    set_gnome_wallpaper(Path::new(local_path)).await?;

    let first_use = !state.db.has_history(wallpaper_id)?;
    let entry = state.db.add_history(wallpaper_id)?;
    if first_use {
        state.report_download(&wallpaper);
    }
    if let Err(error) = app.emit(events::WALLPAPER_APPLIED, &entry) {
        log::warn!("could not emit {}: {error}", events::WALLPAPER_APPLIED);
    }
    // It may have just been downloaded, so look for copies of it.
    let checking = app.clone();
    tauri::async_runtime::spawn(async move { repeats::remove_repeats_of(&checking, wallpaper_id).await });
    log::info!("applied wallpaper {} ({})", wallpaper_id, wallpaper.cache_key());
    Ok(entry)
}

/// Sets `path` as the desktop background, equivalent to
/// `gsettings set org.gnome.desktop.background picture-uri file:///path`.
pub async fn set_gnome_wallpaper(path: &Path) -> AppResult<()> {
    let uri = Url::from_file_path(path)
        .map_err(|_| AppError::Wallpaper(format!("{} is not an absolute file path.", path.display())))?;
    let value = gvariant_string(uri.as_str());

    gnome::set(BACKGROUND_SCHEMA, "picture-uri", &value).await?;

    // GNOME 42 and later show `picture-uri-dark` while the dark style is
    // active; set it too so the change is visible in both styles.
    if gnome::writable(BACKGROUND_SCHEMA, "picture-uri-dark").await {
        gnome::set(BACKGROUND_SCHEMA, "picture-uri-dark", &value).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uris_are_percent_encoded() {
        let uri = Url::from_file_path("/home/me/My Pictures/wall paper.jpg").unwrap();
        assert_eq!(uri.as_str(), "file:///home/me/My%20Pictures/wall%20paper.jpg");
    }
}
