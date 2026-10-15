//! User settings: cache location and per-provider configuration.
//!
//! Settings are stored as JSON in the application config directory, apart
//! from the wallpaper library database. The file can hold API keys, so it is
//! written with owner-only permissions.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    /// Custom wallpaper cache directory. `None` uses the default location.
    pub cache_dir: Option<PathBuf>,
    /// Settings keyed by provider id. Providers without an entry use
    /// [`ProviderSettings::default`].
    pub providers: BTreeMap<String, ProviderSettings>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderSettings {
    pub enabled: bool,
    pub api_key: Option<String>,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self { enabled: true, api_key: None }
    }
}

pub struct SettingsStore {
    path: PathBuf,
    default_cache_dir: PathBuf,
    current: RwLock<AppSettings>,
}

impl SettingsStore {
    /// Loads settings from `path`. A missing file yields defaults; an
    /// unreadable one is set aside so the application can still start.
    pub fn load(path: PathBuf, default_cache_dir: PathBuf) -> Self {
        let current = match fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|error| {
                log::warn!("settings file {} is invalid ({error}); using defaults", path.display());
                let backup = path.with_extension("json.invalid");
                if let Err(error) = fs::rename(&path, &backup) {
                    log::warn!("could not set aside the invalid settings file: {error}");
                }
                AppSettings::default()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => AppSettings::default(),
            Err(error) => {
                log::warn!("could not read settings file {}: {error}", path.display());
                AppSettings::default()
            }
        };
        Self { path, default_cache_dir, current: RwLock::new(current) }
    }

    fn read(&self) -> RwLockReadGuard<'_, AppSettings> {
        self.current.read().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, AppSettings> {
        self.current.write().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn default_cache_dir(&self) -> &Path {
        &self.default_cache_dir
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.read().cache_dir.clone().unwrap_or_else(|| self.default_cache_dir.clone())
    }

    pub fn provider(&self, id: &str) -> ProviderSettings {
        self.read().providers.get(id).cloned().unwrap_or_default()
    }

    /// Applies `change` to a copy of the settings, saves it, then makes it
    /// current. Nothing changes if saving fails.
    pub fn update(&self, change: impl FnOnce(&mut AppSettings)) -> AppResult<AppSettings> {
        let mut current = self.write();
        let mut next = current.clone();
        change(&mut next);
        persist(&self.path, &next)?;
        *current = next.clone();
        Ok(next)
    }
}

fn persist(path: &Path, settings: &AppSettings) -> AppResult<()> {
    let parent =
        path.parent().ok_or_else(|| AppError::Internal("The settings path has no parent directory.".into()))?;
    fs::create_dir_all(parent)?;

    let temporary = path.with_extension("json.tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(settings)?)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    Ok(())
}

/// Normalizes an API key entered by the user: surrounding whitespace is
/// removed and an empty key means "no key".
pub fn normalize_api_key(key: Option<String>) -> Option<String> {
    key.map(|key| key.trim().to_owned()).filter(|key| !key.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::load(dir.path().join("settings.json"), dir.path().join("cache"));
        assert_eq!(store.cache_dir(), dir.path().join("cache"));
        assert_eq!(store.provider("pexels"), ProviderSettings::default());
    }

    #[test]
    fn updates_are_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let store = SettingsStore::load(path.clone(), dir.path().join("cache"));
        store
            .update(|settings| {
                settings.cache_dir = Some(dir.path().join("elsewhere"));
                settings
                    .providers
                    .insert("pexels".into(), ProviderSettings { enabled: true, api_key: Some("key".into()) });
            })
            .unwrap();

        let reloaded = SettingsStore::load(path.clone(), dir.path().join("cache"));
        assert_eq!(reloaded.cache_dir(), dir.path().join("elsewhere"));
        assert_eq!(reloaded.provider("pexels").api_key.as_deref(), Some("key"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }

    #[test]
    fn invalid_file_is_set_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        let store = SettingsStore::load(path.clone(), dir.path().join("cache"));
        assert_eq!(store.cache_dir(), dir.path().join("cache"));
        assert!(path.with_extension("json.invalid").exists());
    }

    #[test]
    fn api_keys_are_trimmed() {
        assert_eq!(normalize_api_key(Some("  abc ".into())).as_deref(), Some("abc"));
        assert_eq!(normalize_api_key(Some("   ".into())), None);
        assert_eq!(normalize_api_key(None), None);
    }
}
