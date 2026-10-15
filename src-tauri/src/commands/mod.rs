//! Tauri commands: the only interface between the frontend and the backend.
//! Commands validate input and delegate to the backend modules.

pub mod app;
pub mod collections;
pub mod favorites;
pub mod rotation;
pub mod search;
pub mod settings;
pub mod theme;
pub mod wallpapers;

use futures_util::StreamExt;
use tauri::{AppHandle, Emitter, Manager};

use crate::cache::{has_local_file, BACKGROUND_DOWNLOADS};
use crate::error::AppResult;
use crate::events;
use crate::models::{Wallpaper, WallpaperInput};
use crate::repeats::{remove_repeats_in, remove_repeats_of};
use crate::state::AppState;

/// Looks up the credits of wallpapers saved before credits were recorded,
/// once per start. Failures (such as being offline) are logged and the
/// lookup runs again at the next start.
pub(crate) async fn fill_missing_credits(app: AppHandle) {
    let state = app.state::<AppState>();
    for provider in state.providers.iter() {
        let missing = match state.db.wallpapers_missing_credit(provider.id()) {
            Ok(missing) if !missing.is_empty() => missing,
            Ok(_) => continue,
            Err(error) => {
                log::warn!("could not list wallpapers without credits: {error}");
                return;
            }
        };
        let provider_ids: Vec<String> = missing.iter().map(|(_, provider_id)| provider_id.clone()).collect();
        let credits = match provider.credits(&provider_ids).await {
            Ok(credits) => credits,
            Err(error) => {
                log::warn!("could not look up credits on {}: {error}", provider.id());
                continue;
            }
        };
        let mut filled = 0;
        for (id, provider_id) in &missing {
            if let Some(credit) = credits.get(provider_id) {
                match state.db.add_credit(*id, credit) {
                    Ok(()) => filled += 1,
                    Err(error) => log::warn!("could not save the credit of wallpaper {id}: {error}"),
                }
            }
        }
        if filled > 0 {
            log::info!("added credits to {filled} {} wallpapers saved earlier", provider.id());
        }
    }
}

/// Validates wallpaper metadata from the frontend and stores it, returning
/// the saved wallpaper.
pub(crate) fn store_wallpaper(state: &AppState, input: &WallpaperInput) -> AppResult<Wallpaper> {
    state.providers.validate_input(input)?;
    state.db.upsert_wallpaper(input)
}

/// Downloads a wallpaper without blocking the caller, so collections are
/// available offline when rotation needs them. Failures are logged; the
/// download is retried when the wallpaper is applied.
/// Once downloaded, the wallpaper is checked for repeats in its collections.
pub(crate) fn cache_in_background(app: &AppHandle, wallpaper: &Wallpaper) {
    let Some(wallpaper_id) = wallpaper.id else { return };
    let app = app.clone();
    if has_local_file(wallpaper) {
        tauri::async_runtime::spawn(async move { remove_repeats_of(&app, wallpaper_id).await });
        return;
    }
    tauri::async_runtime::spawn(async move { prefetch(&app, wallpaper_id).await });
}

/// [`cache_in_background`] for several wallpapers, started in the order
/// given, so a rotation finds the wallpapers it needs first downloaded first.
/// Then the whole collection is checked for repeats, which covers the
/// wallpapers that were downloaded already.
pub(crate) fn cache_in_background_in_order(app: &AppHandle, collection_id: i64, wallpapers: Vec<Wallpaper>) {
    let ids: Vec<i64> = wallpapers.iter().filter(|wallpaper| !has_local_file(wallpaper)).filter_map(|w| w.id).collect();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        futures_util::stream::iter(ids).for_each_concurrent(BACKGROUND_DOWNLOADS, |id| prefetch(&app, id)).await;
        remove_repeats_in(&app, collection_id).await;
    });
}

async fn prefetch(app: &AppHandle, wallpaper_id: i64) {
    let state = app.state::<AppState>();
    let cache_dir = state.settings.cache_dir();
    match state.downloader.prefetch(&state.db, &cache_dir, wallpaper_id).await {
        Ok(cached) => {
            if let Err(error) = app.emit(events::WALLPAPER_CACHED, &cached) {
                log::warn!("could not emit {}: {error}", events::WALLPAPER_CACHED);
            }
            remove_repeats_of(app, wallpaper_id).await;
        }
        Err(error) => log::warn!("background download of wallpaper {wallpaper_id} failed: {error}"),
    }
}
