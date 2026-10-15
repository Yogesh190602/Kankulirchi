//! Keeping one copy of each picture in a collection.
//!
//! The same picture often turns up under different ids, such as a re-upload
//! at another size. Whenever a wallpaper is downloaded or added to a
//! collection, its fingerprint (see [`fingerprint`]) is compared with the
//! rest of the collection, and of two copies the sharper one stays. The
//! other is removed from the collection, and its download deleted if nothing
//! else uses it. Every collection is checked once per start as well.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager};

use crate::cache::fingerprint::{self, Candidate};
use crate::cache::{has_local_file, remove_cached_files};
use crate::error::AppResult;
use crate::events;
use crate::models::{RepeatsRemoved, Wallpaper};
use crate::state::AppState;

/// Removes copies of the wallpaper's picture from the collections it is in,
/// keeping the sharpest copy (see [`fingerprint`]). Failures are logged.
pub async fn remove_repeats_of(app: &AppHandle, wallpaper_id: i64) {
    let state = app.state::<AppState>();
    let _checking = state.repeat_check.lock().await;
    let result: AppResult<()> = async {
        let Some(new) = candidate(&state, &state.db.get_wallpaper(wallpaper_id)?).await? else {
            return Ok(());
        };
        for collection_id in state.db.collections_containing(wallpaper_id)? {
            if let Some(repeat) = fingerprint::repeat_of(&new, &state.db.fingerprinted_wallpapers(collection_id)?) {
                remove_repeats(app, collection_id, &[repeat]).await?;
            }
        }
        Ok(())
    }
    .await;
    if let Err(error) = result {
        log::warn!("could not check wallpaper {wallpaper_id} for repeats: {error}");
    }
}

/// Keeps one copy of each picture in a collection, the sharpest, working out
/// fingerprints its downloaded wallpapers do not have yet.
pub async fn remove_repeats_in(app: &AppHandle, collection_id: i64) {
    let state = app.state::<AppState>();
    let _checking = state.repeat_check.lock().await;
    let result: AppResult<()> = async {
        let mut candidates = Vec::new();
        for wallpaper in state.db.collection_wallpapers(collection_id)? {
            if let Some(candidate) = candidate(&state, &wallpaper).await? {
                candidates.push(candidate);
            }
        }
        let repeats = fingerprint::repeats(&candidates);
        if !repeats.is_empty() {
            remove_repeats(app, collection_id, &repeats).await?;
        }
        Ok(())
    }
    .await;
    if let Err(error) = result {
        log::warn!("could not check collection {collection_id} for repeats: {error}");
    }
}

/// Checks every collection for repeats, once per start, so wallpapers saved
/// before fingerprints existed are covered too.
pub async fn remove_repeats_everywhere(app: AppHandle) {
    let collections = match app.state::<AppState>().db.list_collections() {
        Ok(collections) => collections,
        Err(error) => return log::warn!("could not list collections to check for repeats: {error}"),
    };
    for collection in collections {
        remove_repeats_in(&app, collection.id).await;
    }
}

/// The wallpaper with its fingerprint, worked out from its download (and
/// saved) if it has none yet. `None` when there is neither.
async fn candidate(state: &AppState, wallpaper: &Wallpaper) -> AppResult<Option<Candidate>> {
    let Some(wallpaper_id) = wallpaper.id else { return Ok(None) };
    let fingerprint = match state.db.fingerprint(wallpaper_id)? {
        Some(fingerprint) => fingerprint,
        None if has_local_file(wallpaper) => {
            let path = PathBuf::from(wallpaper.local_path.as_deref().unwrap_or_default());
            match fingerprint::of_file(path).await {
                Ok(fingerprint) => {
                    state.db.set_fingerprint(wallpaper_id, &fingerprint)?;
                    fingerprint
                }
                Err(error) => {
                    log::warn!("could not fingerprint {}: {error}", wallpaper.cache_key());
                    return Ok(None);
                }
            }
        }
        None => return Ok(None),
    };
    Ok(Some(Candidate { wallpaper_id, width: wallpaper.width, height: wallpaper.height, fingerprint }))
}

async fn remove_repeats(app: &AppHandle, collection_id: i64, repeats: &[i64]) -> AppResult<()> {
    let state = app.state::<AppState>();
    let (removed, unused) = state.db.remove_repeats(collection_id, repeats)?;
    if removed == 0 {
        return Ok(());
    }
    let freed = remove_cached_files(unused).await?;
    let collection = state.db.get_collection(collection_id)?;
    log::info!(
        "removed {removed} repeated pictures from {}, keeping the sharpest copies ({} bytes freed)",
        collection.name,
        freed.total_bytes
    );
    let payload = RepeatsRemoved { collection_id, collection_name: collection.name, count: removed };
    if let Err(error) = app.emit(events::REPEATS_REMOVED, &payload) {
        log::warn!("could not emit {}: {error}", events::REPEATS_REMOVED);
    }
    Ok(())
}
