//! Names of events emitted to the frontend.

/// Payload: `HistoryEntry`. Emitted whenever the desktop wallpaper changes,
/// whether applied by hand or by the scheduler.
pub const WALLPAPER_APPLIED: &str = "wallpaper-applied";

/// Payload: `RotationStatus`. Emitted when rotation starts, stops, changes
/// settings, rotates or fails.
pub const ROTATION_STATUS: &str = "rotation-status";

/// Payload: `Wallpaper`. Emitted when a background download finishes.
pub const WALLPAPER_CACHED: &str = "wallpaper-cached";

/// Payload: `RepeatsRemoved`. Emitted when copies of a picture were removed
/// from a collection.
pub const REPEATS_REMOVED: &str = "repeats-removed";
