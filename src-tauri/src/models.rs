use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Shortest rotation interval, in seconds.
pub const MIN_INTERVAL_SECONDS: u32 = 5;

/// Longest rotation interval, in seconds: one day.
pub const MAX_INTERVAL_SECONDS: u32 = 86_400;

/// Who made a wallpaper and where it comes from: the attribution providers
/// and licenses ask for. Every link points to a host the provider declares.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Credit {
    pub author: Option<String>,
    /// The author's profile on the provider's website.
    pub author_url: Option<String>,
    /// The wallpaper's page on the provider's website.
    pub source_url: Option<String>,
    pub license: Option<String>,
    pub license_url: Option<String>,
}

/// A wallpaper as the frontend sees it. Search results that have never been
/// saved have `id == None`; anything stored in SQLite has an id.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Wallpaper {
    pub id: Option<i64>,
    pub provider: String,
    pub provider_id: String,
    pub title: String,
    pub image_url: String,
    pub thumbnail_url: String,
    pub local_path: Option<String>,
    pub width: u32,
    pub height: u32,
    pub created_at: Option<String>,
    pub is_favorite: bool,
    pub credit: Credit,
}

impl Wallpaper {
    pub fn cache_key(&self) -> String {
        format!("{}:{}", self.provider, self.provider_id)
    }
}

/// Wallpaper metadata sent by the frontend, typically a search result the
/// user wants to favorite, collect, download or apply. Unknown fields (such
/// as `id` or `isFavorite`) are ignored, so a full `Wallpaper` can be passed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperInput {
    pub provider: String,
    pub provider_id: String,
    #[serde(default)]
    pub title: String,
    pub image_url: String,
    pub thumbnail_url: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub credit: Credit,
}

/// Database state of a single wallpaper, used by the detail view.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperStatus {
    pub wallpaper: Option<Wallpaper>,
    pub collection_ids: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub wallpaper_count: i64,
    pub preview_thumbnails: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionDetail {
    pub collection: Collection,
    pub wallpapers: Vec<Wallpaper>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub applied_at: String,
    pub wallpaper: Wallpaper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RotationMode {
    Sequential,
    Random,
}

impl RotationMode {
    pub fn as_str(self) -> &'static str {
        match self {
            RotationMode::Sequential => "sequential",
            RotationMode::Random => "random",
        }
    }
}

impl fmt::Display for RotationMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RotationMode {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "sequential" => Ok(RotationMode::Sequential),
            "random" => Ok(RotationMode::Random),
            other => Err(AppError::Validation(format!("Unknown rotation mode \"{other}\"."))),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RotationSettings {
    pub id: i64,
    pub collection_id: i64,
    pub interval_seconds: u32,
    pub rotation_mode: RotationMode,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RotationStatus {
    pub active: bool,
    pub collection_id: Option<i64>,
    pub collection_name: Option<String>,
    pub interval_seconds: Option<u32>,
    pub rotation_mode: Option<RotationMode>,
    pub last_applied_at: Option<String>,
    pub next_run_at: Option<String>,
    pub last_error: Option<String>,
}

/// Result of rotating search results: the collection they were saved to and
/// the new rotation status.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRotation {
    pub collection: Collection,
    pub status: RotationStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderError {
    pub provider: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub wallpapers: Vec<Wallpaper>,
    pub page: u32,
    /// Providers that have at least one more page for this query. The
    /// frontend requests the next page from these providers only.
    pub providers_with_more: Vec<String>,
    pub errors: Vec<ProviderError>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheStats {
    pub path: String,
    pub total_bytes: u64,
    pub file_count: u64,
}

/// Copies of pictures removed from a collection, which kept the sharpest
/// copy of each.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepeatsRemoved {
    pub collection_id: i64,
    pub collection_name: String,
    pub count: usize,
}

/// Downloads deleted to free up space.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreedSpace {
    pub file_count: u64,
    pub total_bytes: u64,
}

/// Rotation intervals can be any whole number of seconds from
/// [`MIN_INTERVAL_SECONDS`] to [`MAX_INTERVAL_SECONDS`].
pub fn validate_interval(interval_seconds: u32) -> Result<(), AppError> {
    if (MIN_INTERVAL_SECONDS..=MAX_INTERVAL_SECONDS).contains(&interval_seconds) {
        Ok(())
    } else {
        Err(AppError::Validation(format!(
            "Choose an interval from {MIN_INTERVAL_SECONDS} seconds to one day ({MAX_INTERVAL_SECONDS} seconds)."
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_intervals_from_five_seconds_to_a_day() {
        for seconds in [5, 45, 60, 90, 300, 86_400] {
            assert!(validate_interval(seconds).is_ok(), "{seconds} seconds");
        }
        for seconds in [0, 4, 86_401] {
            assert!(validate_interval(seconds).is_err(), "{seconds} seconds");
        }
    }

    #[test]
    fn rotation_mode_round_trips() {
        for mode in [RotationMode::Sequential, RotationMode::Random] {
            assert_eq!(mode.as_str().parse::<RotationMode>().unwrap(), mode);
        }
        assert!("shuffle".parse::<RotationMode>().is_err());
    }

    #[test]
    fn wallpaper_input_ignores_unknown_fields() {
        let input: WallpaperInput = serde_json::from_str(
            r#"{"id":null,"provider":"wallhaven","providerId":"abc","title":"t",
                "imageUrl":"https://w.wallhaven.cc/full/ab/wallhaven-abc.jpg",
                "thumbnailUrl":"https://th.wallhaven.cc/lg/ab/abc.jpg",
                "width":1920,"height":1080,"isFavorite":false,"localPath":null}"#,
        )
        .unwrap();
        assert_eq!(input.provider_id, "abc");
        assert_eq!(input.width, 1920);
    }
}
