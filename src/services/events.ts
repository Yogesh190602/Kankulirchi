// Event names emitted by the backend (src-tauri/src/events.rs).

/** Payload: HistoryEntry. */
export const WALLPAPER_APPLIED = "wallpaper-applied";

/** Payload: RotationStatus. */
export const ROTATION_STATUS = "rotation-status";

/** Payload: Wallpaper. */
export const WALLPAPER_CACHED = "wallpaper-cached";
/** Copies of pictures were removed from a collection, keeping the sharpest of each. */
export const REPEATS_REMOVED = "repeats-removed";
