//! A backup of the desktop settings Kankulirchi changes, so they can be put
//! back.
//!
//! Just before Kankulirchi first changes a desktop setting, it saves every
//! setting it changes now or may change for themes: the background, the lock
//! screen picture, the accent color and the light or dark style. Restoring
//! puts them all back. A picture that is already one of Kankulirchi's
//! downloads is saved as GNOME's default instead, because the one before it
//! is not known and the download can be deleted.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::database::now_timestamp;
use crate::error::{AppError, AppResult};
use crate::gnome;

const BACKGROUND: &str = "org.gnome.desktop.background";
const LOCK_SCREEN: &str = "org.gnome.desktop.screensaver";
const INTERFACE: &str = "org.gnome.desktop.interface";

/// Settings saved in the backup. Ones this GNOME version lacks are skipped.
const SAVED_SETTINGS: &[(&str, &str)] = &[
    (BACKGROUND, "picture-uri"),
    (BACKGROUND, "picture-uri-dark"),
    (BACKGROUND, "picture-options"),
    (BACKGROUND, "primary-color"),
    (BACKGROUND, "secondary-color"),
    (BACKGROUND, "color-shading-type"),
    (LOCK_SCREEN, "picture-uri"),
    (INTERFACE, "accent-color"),
    (INTERFACE, "color-scheme"),
];

/// Settings whose value is a picture.
const PICTURE_SETTINGS: &[(&str, &str)] =
    &[(BACKGROUND, "picture-uri"), (BACKGROUND, "picture-uri-dark"), (LOCK_SCREEN, "picture-uri")];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopBackup {
    pub taken_at: String,
    pub settings: Vec<SavedSetting>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSetting {
    pub schema: String,
    pub key: String,
    /// The value as GVariant text. `None` restores GNOME's default.
    pub value: Option<String>,
}

impl DesktopBackup {
    fn value(&self, schema: &str, key: &str) -> Option<&SavedSetting> {
        self.settings.iter().find(|setting| setting.schema == schema && setting.key == key)
    }

    /// What restoring the backup sets, in words the Settings page shows.
    pub fn summary(&self) -> BackupSummary {
        let text = |schema, key| {
            self.value(schema, key).and_then(|setting| setting.value.as_deref()).and_then(gnome::parse_gvariant_string)
        };
        BackupSummary {
            taken_at: self.taken_at.clone(),
            wallpaper: self.value(BACKGROUND, "picture-uri").map(|setting| picture_name(setting.value.as_deref())),
            accent_color: text(INTERFACE, "accent-color"),
            color_scheme: text(INTERFACE, "color-scheme"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSummary {
    pub taken_at: String,
    /// The background's file name, or `Some(None)` for GNOME's default.
    /// Left out if the backup has no background.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallpaper: Option<Option<String>>,
    /// GNOME's accent color name, such as "blue".
    pub accent_color: Option<String>,
    /// "default", "prefer-dark" or "prefer-light".
    pub color_scheme: Option<String>,
}

/// The backup file, written once.
pub struct BackupStore {
    path: PathBuf,
    /// Keeps two first changes from both taking the backup.
    writing: tokio::sync::Mutex<()>,
}

impl BackupStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path, writing: tokio::sync::Mutex::new(()) }
    }

    pub fn load(&self) -> AppResult<Option<DesktopBackup>> {
        match fs::read_to_string(&self.path) {
            Ok(text) => Ok(Some(serde_json::from_str(&text)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    /// Saves the current desktop settings, unless a backup already exists.
    /// Pictures inside `download_dirs` are saved as GNOME's default.
    pub async fn ensure(&self, download_dirs: &[PathBuf]) -> AppResult<()> {
        let _writing = self.writing.lock().await;
        if self.path.exists() {
            return Ok(());
        }
        let mut settings = Vec::new();
        for &(schema, key) in SAVED_SETTINGS {
            if let Some(value) = gnome::get(schema, key).await {
                let value = if PICTURE_SETTINGS.contains(&(schema, key)) {
                    original_picture(&value, download_dirs)
                } else {
                    Some(value)
                };
                settings.push(SavedSetting { schema: schema.into(), key: key.into(), value });
            }
        }
        if settings.is_empty() {
            return Err(AppError::Wallpaper("GNOME's desktop settings could not be read.".into()));
        }
        write_atomically(
            &self.path,
            &serde_json::to_string_pretty(&DesktopBackup { taken_at: now_timestamp(), settings })?,
        )?;
        log::info!("saved the desktop settings to {} before changing them", self.path.display());
        Ok(())
    }

    /// Puts every saved setting back. The backup is kept, so it can be
    /// restored again later.
    pub async fn restore(&self) -> AppResult<()> {
        let backup = self.load()?.ok_or_else(|| {
            AppError::NotFound("Kankulirchi has not changed your desktop yet, so there is nothing to restore.".into())
        })?;
        let mut failed = Vec::new();
        for setting in &backup.settings {
            let result = match &setting.value {
                Some(value) => gnome::set(&setting.schema, &setting.key, value).await,
                None => gnome::reset(&setting.schema, &setting.key).await,
            };
            if let Err(error) = result {
                log::warn!("could not restore {} {}: {error}", setting.schema, setting.key);
                failed.push(setting.key.as_str());
            }
        }
        if !failed.is_empty() {
            return Err(AppError::Wallpaper(format!("Some settings could not be restored: {}.", failed.join(", "))));
        }
        log::info!("restored the desktop settings saved at {}", backup.taken_at);
        Ok(())
    }
}

/// `value`, or `None` (GNOME's default) if it is a picture inside one of
/// `download_dirs`.
fn original_picture(value: &str, download_dirs: &[PathBuf]) -> Option<String> {
    let downloaded = gnome::parse_gvariant_string(value)
        .and_then(|uri| Url::parse(&uri).ok())
        .and_then(|url| url.to_file_path().ok())
        .is_some_and(|path| download_dirs.iter().any(|dir| path.starts_with(dir)));
    (!downloaded).then(|| value.to_owned())
}

/// The file name of a picture setting, or `None` for GNOME's default.
fn picture_name(value: Option<&str>) -> Option<String> {
    let uri = gnome::parse_gvariant_string(value?)?;
    let url = Url::parse(&uri).ok()?;
    let name = url.path_segments()?.next_back()?;
    let decoded = percent_decode(name);
    (!decoded.is_empty()).then_some(decoded)
}

fn percent_decode(text: &str) -> String {
    url::form_urlencoded::parse(format!("x={}", text.replace('+', "%2B")).as_bytes())
        .next()
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| text.to_owned())
}

/// Writes `contents` to a temporary file next to `path`, then moves it into
/// place, so a crash never leaves half a backup.
fn write_atomically(path: &Path, contents: &str) -> AppResult<()> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, contents)?;
    fs::rename(&temporary, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_from_kankulirchi_downloads_restore_gnome_default() {
        let dirs = [PathBuf::from("/home/me/.cache/com.kankulirchi.desktop/wallpapers"), PathBuf::from("/data/walls")];
        assert_eq!(
            original_picture("'file:///home/me/.cache/com.kankulirchi.desktop/wallpapers/wallhaven-1.jpg'", &dirs),
            None
        );
        assert_eq!(original_picture("'file:///data/walls/x.png'", &dirs), None);
        let own = "'file:///home/me/Pictures/My%20Lake.jpg'";
        assert_eq!(original_picture(own, &dirs).as_deref(), Some(own));
        let gnome = "'file:///usr/share/backgrounds/gnome/adwaita-timed.xml'";
        assert_eq!(original_picture(gnome, &dirs).as_deref(), Some(gnome));
    }

    #[test]
    fn summary_describes_what_restoring_sets() {
        let setting = |schema: &str, key: &str, value: Option<&str>| SavedSetting {
            schema: schema.into(),
            key: key.into(),
            value: value.map(str::to_owned),
        };
        let backup = DesktopBackup {
            taken_at: "2026-10-10T08:00:00.000Z".into(),
            settings: vec![
                setting(BACKGROUND, "picture-uri", Some("'file:///home/me/Pictures/My%20Lake.jpg'")),
                setting(INTERFACE, "accent-color", Some("'teal'")),
                setting(INTERFACE, "color-scheme", Some("'prefer-dark'")),
            ],
        };
        let summary = backup.summary();
        assert_eq!(summary.wallpaper, Some(Some("My Lake.jpg".into())));
        assert_eq!(summary.accent_color.as_deref(), Some("teal"));
        assert_eq!(summary.color_scheme.as_deref(), Some("prefer-dark"));

        let default = DesktopBackup { settings: vec![setting(BACKGROUND, "picture-uri", None)], ..backup };
        assert_eq!(default.summary().wallpaper, Some(None));
        assert_eq!(default.summary().accent_color, None);
    }

    #[test]
    fn backups_are_read_back_and_missing_ones_are_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path().join("desktop-backup.json"));
        assert_eq!(store.load().unwrap(), None);
        let backup = DesktopBackup {
            taken_at: "2026-10-10T08:00:00.000Z".into(),
            settings: vec![SavedSetting {
                schema: INTERFACE.into(),
                key: "accent-color".into(),
                value: Some("'blue'".into()),
            }],
        };
        write_atomically(&store.path, &serde_json::to_string(&backup).unwrap()).unwrap();
        assert_eq!(store.load().unwrap(), Some(backup));
    }
}
