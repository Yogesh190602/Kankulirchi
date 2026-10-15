//! Running without a window.
//!
//! While a rotation is on, closing the window closes only the window: the
//! process keeps running so the scheduler can keep changing the wallpaper,
//! and launching Kankulirchi again opens a new window (see the single-instance
//! plugin in `lib.rs`). The window is destroyed rather than hidden, so its web
//! view's memory is released while it is closed. With no rotation on there
//! is nothing to do in the background, so closing the window quits.

use tauri::{AppHandle, Manager, WebviewWindowBuilder};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

/// Starts Kankulirchi without opening its window, as the login entry does.
pub const BACKGROUND_ARG: &str = "--background";

const MAIN_WINDOW: &str = "main";

pub fn wants_background(args: &[String]) -> bool {
    args.iter().any(|arg| arg == BACKGROUND_ARG)
}

/// Whether there is a reason to keep running without a window: a rotation
/// is on.
pub fn has_background_work(app: &AppHandle) -> bool {
    let Some(state) = app.try_state::<AppState>() else {
        return false;
    };
    match state.db.enabled_rotation() {
        Ok(rotation) => rotation.is_some(),
        Err(error) => {
            log::warn!("could not read the rotation settings: {error}");
            false
        }
    }
}

/// Brings the main window to the front, creating it if it is closed.
pub fn show_main_window(app: &AppHandle) -> AppResult<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    // tauri.conf.json marks the window `"create": false`, so it is created
    // here, where a background start can skip it.
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == MAIN_WINDOW)
        .ok_or_else(|| AppError::Internal("tauri.conf.json has no main window".into()))?;
    WebviewWindowBuilder::from_config(app, config)?.build()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_flag_is_recognized_anywhere_in_the_arguments() {
        let args = |list: &[&str]| list.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
        assert!(wants_background(&args(&["kankulirchi", "--background"])));
        assert!(!wants_background(&args(&["kankulirchi"])));
        assert!(!wants_background(&args(&["kankulirchi", "--background-color"])));
    }
}
