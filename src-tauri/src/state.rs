use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::cache::Downloader;
use crate::database::Database;
use crate::error::AppResult;
use crate::models::Wallpaper;
use crate::providers::ProviderRegistry;
use crate::settings::SettingsStore;
use crate::theme::BackupStore;
use crate::wallpaper::Scheduler;

/// Shared backend services, managed by Tauri and available to every command.
pub struct AppState {
    pub db: Database,
    pub providers: ProviderRegistry,
    pub settings: SettingsStore,
    pub downloader: Downloader,
    pub scheduler: Scheduler,
    pub backup: BackupStore,
    /// Lets one check for repeated pictures run at a time.
    pub repeat_check: tokio::sync::Mutex<()>,
}

impl AppState {
    pub fn initialize(app: &AppHandle) -> AppResult<Self> {
        let paths = app.path();
        let database_path = paths.app_data_dir()?.join("library.db");
        let backup_path = paths.app_data_dir()?.join("desktop-backup.json");
        let settings_path = paths.app_config_dir()?.join("settings.json");
        let default_cache_dir = paths.app_cache_dir()?.join("wallpapers");

        let settings = SettingsStore::load(settings_path, default_cache_dir);
        let db = Database::open(&database_path)?;
        let client = http_client()?;

        log::info!("library database: {}", database_path.display());
        log::info!("wallpaper cache: {}", settings.cache_dir().display());

        Ok(Self {
            db,
            providers: ProviderRegistry::with_defaults(client.clone()),
            settings,
            downloader: Downloader::new(client),
            scheduler: Scheduler::new(),
            backup: BackupStore::new(backup_path),
            repeat_check: tokio::sync::Mutex::new(()),
        })
    }

    /// Tells the wallpaper's provider that the user started using it, with
    /// the API key configured for that provider.
    pub fn report_download(&self, wallpaper: &Wallpaper) {
        let api_key = self.settings.provider(&wallpaper.provider).api_key;
        self.providers.report_download(wallpaper, api_key);
    }
}

fn http_client() -> AppResult<reqwest::Client> {
    Ok(reqwest::Client::builder()
        // Name, version and where to find the project, as Wikimedia's
        // User-Agent policy asks of every app.
        .user_agent(concat!("Kankulirchi/", env!("CARGO_PKG_VERSION"), " (", env!("CARGO_PKG_REPOSITORY"), ")"))
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(60))
        .build()?)
}
