//! Wallpaper rotation scheduler.
//!
//! One background task rotates the wallpapers of the enabled collection. It
//! keeps no schedule of its own: the next change is always derived from the
//! newest history entry plus the interval, so rotation resumes correctly
//! after a restart and a wallpaper applied by hand restarts the countdown.
//!
//! The task wakes at least every [`MAX_SLEEP`] and compares against the wall
//! clock, so time spent suspended counts towards the interval.

use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use super::apply::apply_wallpaper;
use crate::cache::has_local_file;
use crate::database::Database;
use crate::error::{AppError, AppResult};
use crate::events;
use crate::models::{validate_interval, HistoryEntry, RotationMode, RotationSettings, RotationStatus, Wallpaper};
use crate::state::AppState;

/// Longest the task sleeps before checking the clock again.
const MAX_SLEEP: Duration = Duration::from_secs(30);

/// Sleep while no rotation is enabled. Starting a rotation wakes the task.
const IDLE_SLEEP: Duration = Duration::from_secs(3600);

/// Delay before retrying after a failed rotation: the interval, kept
/// between these bounds so a short interval does not hammer a provider that
/// is failing.
const MIN_RETRY_DELAY: chrono::Duration = chrono::Duration::seconds(30);
const MAX_RETRY_DELAY: chrono::Duration = chrono::Duration::minutes(5);

/// Wallpapers tried per rotation before giving up until the retry.
const MAX_ATTEMPTS: usize = 3;

#[derive(Default)]
struct Runtime {
    last_error: Option<String>,
    last_failure_at: Option<DateTime<Utc>>,
}

pub struct Scheduler {
    wake: Notify,
    /// Serializes rotations so the background task and a manual start never
    /// change the wallpaper twice in a row.
    rotation: tokio::sync::Mutex<()>,
    runtime: Mutex<Runtime>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self { wake: Notify::new(), rotation: tokio::sync::Mutex::new(()), runtime: Mutex::new(Runtime::default()) }
    }

    /// Starts the background task. Call once, after `AppState` is managed.
    pub fn spawn(app: AppHandle) {
        tauri::async_runtime::spawn(async move {
            loop {
                let state = app.state::<AppState>();
                let wait = state.scheduler.tick(&app).await;
                tokio::select! {
                    _ = state.scheduler.wake.notified() => {}
                    _ = tokio::time::sleep(wait) => {}
                }
            }
        });
    }

    /// Makes the background task re-read the rotation settings now.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    fn runtime(&self) -> std::sync::MutexGuard<'_, Runtime> {
        self.runtime.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn reset_runtime(&self) {
        *self.runtime() = Runtime::default();
    }

    fn record(&self, result: &AppResult<HistoryEntry>) {
        let mut runtime = self.runtime();
        match result {
            Ok(_) => *runtime = Runtime::default(),
            Err(error) => {
                log::warn!("wallpaper rotation failed: {error}");
                runtime.last_error = Some(error.to_string());
                runtime.last_failure_at = Some(Utc::now());
            }
        }
    }

    /// Starts rotating a collection, replacing any running rotation. The
    /// first wallpaper is applied right away unless this collection was
    /// already rotating. A failed first change is reported in the returned
    /// status and retried by the background task.
    pub async fn start(
        &self,
        app: &AppHandle,
        collection_id: i64,
        interval_seconds: u32,
        mode: RotationMode,
    ) -> AppResult<RotationStatus> {
        validate_interval(interval_seconds)?;
        let state = app.state::<AppState>();
        {
            let _rotation = self.rotation.lock().await;
            let collection = state.db.get_collection(collection_id)?;
            if collection.wallpaper_count == 0 {
                return Err(AppError::Validation(format!(
                    "Add wallpapers to {} before starting rotation.",
                    collection.name
                )));
            }
            let already_running =
                state.db.enabled_rotation()?.is_some_and(|settings| settings.collection_id == collection_id);
            let settings = state.db.save_rotation_settings(collection_id, interval_seconds, mode)?;
            state.db.enable_rotation(collection_id)?;
            if !already_running {
                self.reset_runtime();
                let result = self.rotate(app, &settings).await;
                self.record(&result);
            }
        }
        self.wake();
        self.publish(app)
    }

    pub fn stop(&self, app: &AppHandle) -> AppResult<RotationStatus> {
        let state = app.state::<AppState>();
        state.db.disable_rotation()?;
        self.reset_runtime();
        self.wake();
        self.publish(app)
    }

    /// [`Self::stop`], once a change in progress has finished, so no change
    /// lands after this returns.
    pub async fn stop_after_current_change(&self, app: &AppHandle) -> AppResult<RotationStatus> {
        let _rotation = self.rotation.lock().await;
        self.stop(app)
    }

    /// Call after rotation settings or the rotating collection changed.
    pub fn settings_changed(&self, app: &AppHandle) -> AppResult<RotationStatus> {
        self.wake();
        self.publish(app)
    }

    pub fn status(&self, db: &Database) -> AppResult<RotationStatus> {
        let last_applied_at = db.latest_history()?.map(|entry| entry.applied_at);
        let Some(settings) = db.enabled_rotation()? else {
            return Ok(RotationStatus { last_applied_at, ..RotationStatus::default() });
        };
        let collection_name = db.get_collection(settings.collection_id)?.name;
        let next_run_at = self.next_due(db, &settings)?;
        Ok(RotationStatus {
            active: true,
            collection_id: Some(settings.collection_id),
            collection_name: Some(collection_name),
            interval_seconds: Some(settings.interval_seconds),
            rotation_mode: Some(settings.rotation_mode),
            last_applied_at,
            next_run_at: Some(next_run_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
            last_error: self.runtime().last_error.clone(),
        })
    }

    fn publish(&self, app: &AppHandle) -> AppResult<RotationStatus> {
        let state = app.state::<AppState>();
        let status = self.status(&state.db)?;
        if let Err(error) = app.emit(events::ROTATION_STATUS, &status) {
            log::warn!("could not emit {}: {error}", events::ROTATION_STATUS);
        }
        Ok(status)
    }

    /// When the enabled rotation should change the wallpaper next.
    fn next_due(&self, db: &Database, settings: &RotationSettings) -> AppResult<DateTime<Utc>> {
        let interval = chrono::Duration::seconds(i64::from(settings.interval_seconds));
        let last_applied = db
            .latest_history()?
            .and_then(|entry| DateTime::parse_from_rfc3339(&entry.applied_at).ok())
            .map(|time| time.with_timezone(&Utc));
        let mut due = last_applied.map_or_else(Utc::now, |time| time + interval);

        if let Some(failed_at) = self.runtime().last_failure_at {
            if last_applied.is_none_or(|applied| failed_at > applied) {
                due = failed_at + retry_delay(interval);
            }
        }
        Ok(due)
    }

    /// One pass of the background task. Returns how long to sleep.
    async fn tick(&self, app: &AppHandle) -> Duration {
        let _rotation = self.rotation.lock().await;
        let state = app.state::<AppState>();

        let settings = match state.db.enabled_rotation() {
            Ok(Some(settings)) => settings,
            Ok(None) => return IDLE_SLEEP,
            Err(error) => {
                log::error!("could not read rotation settings: {error}");
                return MAX_SLEEP;
            }
        };

        let due = match self.next_due(&state.db, &settings) {
            Ok(due) => due,
            Err(error) => {
                log::error!("could not compute the next rotation: {error}");
                return MAX_SLEEP;
            }
        };

        if due > Utc::now() {
            return sleep_until(due);
        }

        let result = self.rotate(app, &settings).await;
        self.record(&result);
        if let Err(error) = self.publish(app) {
            log::warn!("could not publish rotation status: {error}");
        }
        match self.next_due(&state.db, &settings) {
            Ok(due) => sleep_until(due),
            Err(_) => MAX_SLEEP,
        }
    }

    async fn rotate(&self, app: &AppHandle, settings: &RotationSettings) -> AppResult<HistoryEntry> {
        let state = app.state::<AppState>();
        let wallpapers = state.db.collection_wallpapers(settings.collection_id)?;
        if wallpapers.is_empty() {
            return Err(AppError::Validation("The rotating collection is empty. Add wallpapers to it.".into()));
        }
        let last_in_collection = state.db.last_applied_in_collection(settings.collection_id)?;
        let current = state.db.latest_history()?.and_then(|entry| entry.wallpaper.id);
        let candidates =
            order_candidates(wallpapers, settings.rotation_mode, last_in_collection, current, fastrand::shuffle);

        let mut last_error = None;
        for wallpaper in candidates.into_iter().take(MAX_ATTEMPTS) {
            let Some(id) = wallpaper.id else { continue };
            match apply_wallpaper(app, id).await {
                Ok(entry) => return Ok(entry),
                // GNOME refused the change; another wallpaper will not help.
                Err(error @ AppError::Wallpaper(_)) => return Err(error),
                Err(error) => {
                    log::warn!("could not rotate to wallpaper {id}: {error}");
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| AppError::Internal("No wallpaper could be applied.".into())))
    }
}

fn retry_delay(interval: chrono::Duration) -> chrono::Duration {
    interval.clamp(MIN_RETRY_DELAY, MAX_RETRY_DELAY)
}

fn sleep_until(due: DateTime<Utc>) -> Duration {
    (due - Utc::now()).to_std().unwrap_or(Duration::ZERO).clamp(Duration::from_secs(1), MAX_SLEEP)
}

/// Orders a collection's wallpapers by preference for the next rotation.
///
/// The first entry is the wallpaper the mode calls for: the one after
/// `last_in_collection` for sequential rotation, or a random one other than
/// the `current` desktop wallpaper. The rest are fallbacks if it cannot be
/// applied, with already-downloaded wallpapers first so rotation keeps
/// working offline.
fn order_candidates(
    mut wallpapers: Vec<Wallpaper>,
    mode: RotationMode,
    last_in_collection: Option<i64>,
    current: Option<i64>,
    shuffle: impl FnOnce(&mut [Wallpaper]),
) -> Vec<Wallpaper> {
    if wallpapers.is_empty() {
        return wallpapers;
    }
    match mode {
        RotationMode::Sequential => {
            let start = last_in_collection
                .and_then(|id| wallpapers.iter().position(|w| w.id == Some(id)))
                .map_or(0, |index| index + 1);
            let len = wallpapers.len();
            wallpapers.rotate_left(start % len);
        }
        RotationMode::Random => {
            shuffle(&mut wallpapers);
            if wallpapers.len() > 1 {
                if let Some(index) = wallpapers.iter().position(|w| w.id.is_some() && w.id == current) {
                    let repeat = wallpapers.remove(index);
                    wallpapers.push(repeat);
                }
            }
        }
    }
    wallpapers[1..].sort_by_key(|wallpaper| !has_local_file(wallpaper));
    wallpapers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wallpaper(id: i64) -> Wallpaper {
        Wallpaper {
            id: Some(id),
            provider: "wallhaven".into(),
            provider_id: id.to_string(),
            title: String::new(),
            image_url: String::new(),
            thumbnail_url: String::new(),
            local_path: None,
            width: 0,
            height: 0,
            created_at: None,
            is_favorite: false,
            credit: Default::default(),
        }
    }

    fn ids(wallpapers: &[Wallpaper]) -> Vec<i64> {
        wallpapers.iter().map(|w| w.id.unwrap()).collect()
    }

    fn no_shuffle(_: &mut [Wallpaper]) {}

    #[test]
    fn sequential_starts_at_the_beginning() {
        let list = vec![wallpaper(1), wallpaper(2), wallpaper(3)];
        let ordered = order_candidates(list, RotationMode::Sequential, None, None, no_shuffle);
        assert_eq!(ids(&ordered)[0], 1);
    }

    #[test]
    fn sequential_continues_after_the_last_applied() {
        let list = vec![wallpaper(1), wallpaper(2), wallpaper(3)];
        let ordered = order_candidates(list, RotationMode::Sequential, Some(2), None, no_shuffle);
        assert_eq!(ids(&ordered)[0], 3);
    }

    #[test]
    fn sequential_wraps_around() {
        let list = vec![wallpaper(1), wallpaper(2), wallpaper(3)];
        let ordered = order_candidates(list, RotationMode::Sequential, Some(3), None, no_shuffle);
        assert_eq!(ids(&ordered), [1, 2, 3]);
    }

    #[test]
    fn sequential_restarts_when_last_was_removed() {
        let list = vec![wallpaper(1), wallpaper(2)];
        let ordered = order_candidates(list, RotationMode::Sequential, Some(9), None, no_shuffle);
        assert_eq!(ids(&ordered)[0], 1);
    }

    #[test]
    fn random_avoids_repeating_the_current_wallpaper() {
        let list = vec![wallpaper(1), wallpaper(2), wallpaper(3)];
        let ordered = order_candidates(list, RotationMode::Random, None, Some(1), no_shuffle);
        assert_eq!(*ids(&ordered).last().unwrap(), 1);
        assert_ne!(ids(&ordered)[0], 1);
    }

    #[test]
    fn random_with_one_wallpaper_still_applies_it() {
        let ordered = order_candidates(vec![wallpaper(1)], RotationMode::Random, None, Some(1), no_shuffle);
        assert_eq!(ids(&ordered), [1]);
    }

    #[test]
    fn retries_wait_between_thirty_seconds_and_five_minutes() {
        let seconds = chrono::Duration::seconds;
        assert_eq!(retry_delay(seconds(5)), seconds(30));
        assert_eq!(retry_delay(seconds(60)), seconds(60));
        assert_eq!(retry_delay(seconds(3600)), seconds(300));
    }

    #[test]
    fn fallbacks_prefer_downloaded_wallpapers() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("3.jpg");
        std::fs::write(&file, b"x").unwrap();
        let mut cached = wallpaper(3);
        cached.local_path = Some(file.to_str().unwrap().into());

        let list = vec![wallpaper(1), wallpaper(2), cached];
        let ordered = order_candidates(list, RotationMode::Sequential, None, None, no_shuffle);
        assert_eq!(ids(&ordered), [1, 3, 2]);
    }
}
