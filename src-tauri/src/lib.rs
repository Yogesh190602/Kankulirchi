//! Kankulirchi: search, collect, cache and rotate GNOME wallpapers.

mod autostart;
mod background;
mod cache;
mod commands;
mod database;
mod error;
mod events;
mod gnome;
mod models;
mod providers;
mod repeats;
mod settings;
mod state;
mod theme;
mod wallpaper;

use tauri::Manager;

use crate::state::AppState;
use crate::wallpaper::Scheduler;

pub fn run() {
    tauri::Builder::default()
        // Must be registered first: launching Kankulirchi again opens the
        // running instance's window instead of starting another scheduler.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if background::wants_background(&args) {
                return;
            }
            if let Err(error) = background::show_main_window(app) {
                log::error!("could not open the window: {error}");
            }
        }))
        .plugin(tauri_plugin_log::Builder::new().level(log::LevelFilter::Info).build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = AppState::initialize(app.handle())?;
            app.asset_protocol_scope().allow_directory(state.settings.cache_dir(), true)?;
            app.manage(state);
            if !background::wants_background(&std::env::args().collect::<Vec<_>>()) {
                background::show_main_window(app.handle())?;
            } else if background::has_background_work(app.handle()) {
                log::info!("started in the background");
            } else {
                log::info!("started in the background, but no rotation is on, so there is nothing to do");
                app.handle().exit(0);
                return Ok(());
            }
            Scheduler::spawn(app.handle().clone());
            tauri::async_runtime::spawn(commands::fill_missing_credits(app.handle().clone()));
            tauri::async_runtime::spawn(repeats::remove_repeats_everywhere(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::get_launch_at_login,
            commands::app::set_launch_at_login,
            commands::app::quit_app,
            commands::search::list_providers,
            commands::search::search_wallpapers,
            commands::collections::list_collections,
            commands::collections::get_collection,
            commands::collections::create_collection,
            commands::collections::rename_collection,
            commands::collections::delete_collection,
            commands::collections::add_to_collection,
            commands::collections::remove_from_collection,
            commands::favorites::list_favorites,
            commands::favorites::add_favorite,
            commands::favorites::remove_favorite,
            commands::wallpapers::get_wallpaper_status,
            commands::wallpapers::download_wallpaper,
            commands::wallpapers::apply_wallpaper,
            commands::wallpapers::list_history,
            commands::wallpapers::recent_wallpapers,
            commands::rotation::get_rotation_status,
            commands::rotation::get_rotation_settings,
            commands::rotation::save_rotation_settings,
            commands::rotation::start_rotation,
            commands::rotation::start_search_rotation,
            commands::rotation::stop_rotation,
            commands::settings::get_settings,
            commands::settings::update_provider_settings,
            commands::settings::set_cache_dir,
            commands::settings::get_cache_stats,
            commands::theme::wallpaper_palette,
            commands::theme::get_desktop_backup,
            commands::theme::restore_desktop,
        ])
        .build(tauri::generate_context!())
        .expect("failed to start Kankulirchi")
        .run(|app, event| {
            // `code` is None when the last window was closed rather than
            // the app asked to quit.
            if let tauri::RunEvent::ExitRequested { code: None, api, .. } = event {
                if background::has_background_work(app) {
                    log::info!("window closed; rotation continues in the background");
                    api.prevent_exit();
                }
            }
        });
}
