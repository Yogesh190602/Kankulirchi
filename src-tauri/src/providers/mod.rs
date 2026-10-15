//! Wallpaper provider abstraction.
//!
//! Each online source implements [`WallpaperProvider`] in its own module and
//! is registered once in [`ProviderRegistry::with_defaults`]. Nothing else in
//! the application refers to a concrete provider, so adding a source means
//! adding a module and one `register` call; existing providers stay untouched.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures_util::future::join_all;
use reqwest::StatusCode;
use serde::{Deserialize, Deserializer, Serialize};
use url::Url;

use crate::error::{AppError, AppResult};
use crate::models::{Credit, ProviderError, Wallpaper, WallpaperInput};

pub mod pexels;
pub mod safe_search;
pub mod unsplash;
pub mod wallhaven;
pub mod wikimedia;

/// Number of results requested per page from providers that let us choose.
pub const PAGE_SIZE: u32 = 24;

/// Time limit for a single provider API request.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiKeyRequirement {
    None,
    Optional,
    Required,
}

/// Static description of a provider, shown in the provider filter and in
/// Settings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMetadata {
    pub id: &'static str,
    pub name: &'static str,
    pub website: &'static str,
    pub api_key_requirement: ApiKeyRequirement,
    /// Where users can create an API key, if the provider uses one.
    pub api_key_url: Option<&'static str>,
    /// Attribution text the provider asks applications to display.
    pub attribution: Option<&'static str>,
    /// Hosts that credit links (author, source page, license) may point to.
    pub link_hosts: &'static [&'static str],
}

#[derive(Debug, Clone)]
pub struct SearchRequest<'a> {
    pub keyword: &'a str,
    /// 1-based page number.
    pub page: u32,
    pub api_key: Option<&'a str>,
}

/// A wallpaper as returned by a provider, before it is matched against the
/// local database.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderWallpaper {
    pub provider_id: String,
    pub title: String,
    pub image_url: String,
    pub thumbnail_url: String,
    pub width: u32,
    pub height: u32,
    pub credit: Credit,
}

impl ProviderWallpaper {
    pub fn into_wallpaper(self, provider: &str) -> Wallpaper {
        Wallpaper {
            id: None,
            provider: provider.to_owned(),
            provider_id: self.provider_id,
            title: self.title,
            image_url: self.image_url,
            thumbnail_url: self.thumbnail_url,
            local_path: None,
            width: self.width,
            height: self.height,
            created_at: None,
            is_favorite: false,
            credit: self.credit,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProviderPage {
    pub wallpapers: Vec<ProviderWallpaper>,
    pub has_more: bool,
}

#[async_trait]
pub trait WallpaperProvider: Send + Sync {
    fn metadata(&self) -> &ProviderMetadata;

    /// Hosts this provider serves full images and thumbnails from. Downloads
    /// are only allowed from these hosts, over HTTPS.
    fn image_hosts(&self) -> &[&'static str];

    /// Searches for wallpapers. Results must be safe for work: use the
    /// provider's strictest content filter (see [`safe_search`]).
    async fn search(&self, request: &SearchRequest<'_>) -> AppResult<ProviderPage>;

    /// Called when the user starts using one of this provider's wallpapers:
    /// the first time it is applied, or when it is downloaded on request.
    /// Providers whose API terms ask for download tracking report it here.
    async fn report_download(&self, _provider_id: &str, _api_key: Option<&str>) -> AppResult<()> {
        Ok(())
    }

    /// Credits for wallpapers saved before credits were recorded, by
    /// provider id. Only providers whose licenses require attribution need
    /// to look them up; the rest return nothing.
    async fn credits(&self, _provider_ids: &[String]) -> AppResult<HashMap<String, Credit>> {
        Ok(HashMap::new())
    }

    fn id(&self) -> &'static str {
        self.metadata().id
    }
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn WallpaperProvider>>,
}

impl ProviderRegistry {
    /// Registry with every provider that ships with the application.
    pub fn with_defaults(client: reqwest::Client) -> Self {
        let mut registry = Self::default();
        registry.register(Arc::new(wallhaven::WallhavenProvider::new(client.clone())));
        registry.register(Arc::new(unsplash::UnsplashProvider::new(client.clone())));
        registry.register(Arc::new(pexels::PexelsProvider::new(client.clone())));
        registry.register(Arc::new(wikimedia::WikimediaProvider::new(client)));
        registry
    }

    /// Adds a provider. A provider registered with an id that already exists
    /// replaces the earlier one.
    pub fn register(&mut self, provider: Arc<dyn WallpaperProvider>) {
        let id = provider.id();
        match self.providers.iter().position(|existing| existing.id() == id) {
            Some(index) => self.providers[index] = provider,
            None => self.providers.push(provider),
        }
    }

    pub fn get(&self, id: &str) -> Option<&Arc<dyn WallpaperProvider>> {
        self.providers.iter().find(|provider| provider.id() == id)
    }

    pub fn require(&self, id: &str) -> AppResult<&Arc<dyn WallpaperProvider>> {
        self.get(id).ok_or_else(|| AppError::Validation(format!("Unknown wallpaper provider \"{id}\".")))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn WallpaperProvider>> {
        self.providers.iter()
    }

    /// Reports in the background that the user started using `wallpaper`
    /// (see [`WallpaperProvider::report_download`]). Failures are logged.
    pub fn report_download(&self, wallpaper: &Wallpaper, api_key: Option<String>) {
        let Some(provider) = self.get(&wallpaper.provider).cloned() else { return };
        let provider_id = wallpaper.provider_id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = provider.report_download(&provider_id, api_key.as_deref()).await {
                log::warn!("could not report the download of {}:{provider_id}: {error}", provider.id());
            }
        });
    }

    /// Checks wallpaper metadata coming from the frontend before it is stored
    /// or downloaded: the provider must exist and both URLs must point at
    /// that provider's image hosts.
    pub fn validate_input(&self, input: &WallpaperInput) -> AppResult<()> {
        let provider = self.require(&input.provider)?;
        if input.provider_id.trim().is_empty() || input.provider_id.len() > 128 {
            return Err(AppError::Validation("The wallpaper id is invalid.".into()));
        }
        if input.title.chars().count() > 512 {
            return Err(AppError::Validation("The wallpaper title is too long.".into()));
        }
        for url in [&input.image_url, &input.thumbnail_url] {
            if !is_allowed_url(url, provider.image_hosts()) {
                return Err(AppError::Validation(format!(
                    "{} does not serve images from {url}.",
                    provider.metadata().name
                )));
            }
        }
        let credit = &input.credit;
        if [&credit.author, &credit.license].into_iter().flatten().any(|text| text.chars().count() > MAX_CREDIT_TEXT) {
            return Err(AppError::Validation("The wallpaper credit is too long.".into()));
        }
        for url in [&credit.author_url, &credit.source_url, &credit.license_url].into_iter().flatten() {
            if !is_allowed_url(url, provider.metadata().link_hosts) {
                return Err(AppError::Validation(format!(
                    "{} wallpapers do not link to {url}.",
                    provider.metadata().name
                )));
            }
        }
        Ok(())
    }
}

/// Longest author name or license name kept in a credit.
pub(crate) const MAX_CREDIT_TEXT: usize = 200;

/// `raw` if it is an HTTPS link to one of `hosts`. Credits keep only links
/// the app is allowed to open.
pub(crate) fn https_link(raw: &str, hosts: &[&str]) -> Option<String> {
    is_allowed_url(raw, hosts).then(|| raw.to_owned())
}

/// Trims `text` and shortens it to [`MAX_CREDIT_TEXT`] characters. `None` if
/// nothing is left.
pub(crate) fn credit_text(text: &str) -> Option<String> {
    let text: String = text.trim().chars().take(MAX_CREDIT_TEXT).collect();
    (!text.is_empty()).then_some(text)
}

fn is_allowed_url(raw: &str, hosts: &[&str]) -> bool {
    match Url::parse(raw) {
        Ok(url) => {
            url.scheme() == "https"
                && url.host_str().is_some_and(|host| hosts.iter().any(|allowed| host.eq_ignore_ascii_case(allowed)))
        }
        Err(_) => false,
    }
}

/// One provider to query, with the credentials configured for it.
pub struct SearchTarget {
    pub provider: Arc<dyn WallpaperProvider>,
    pub api_key: Option<String>,
}

pub struct AggregatedSearch {
    pub wallpapers: Vec<Wallpaper>,
    pub providers_with_more: Vec<String>,
    pub errors: Vec<ProviderError>,
}

/// Queries all targets concurrently. A failing provider does not fail the
/// whole search; its error is reported next to the other providers' results.
/// Results are interleaved so a mixed search shows every source near the top.
pub async fn search_providers(targets: Vec<SearchTarget>, keyword: &str, page: u32) -> AggregatedSearch {
    let searches = targets.iter().map(|target| async move {
        let request = SearchRequest { keyword, page, api_key: target.api_key.as_deref() };
        (target.provider.id(), target.provider.search(&request).await)
    });
    let results = join_all(searches).await;

    let mut pages = Vec::new();
    let mut providers_with_more = Vec::new();
    let mut errors = Vec::new();
    for (provider, result) in results {
        match result {
            Ok(page) => {
                if page.has_more {
                    providers_with_more.push(provider.to_owned());
                }
                pages.push((provider, page.wallpapers));
            }
            Err(error) => {
                log::warn!("search on {provider} failed: {error}");
                errors.push(ProviderError { provider: provider.to_owned(), message: error.to_string() });
            }
        }
    }

    AggregatedSearch { wallpapers: interleave(pages), providers_with_more, errors }
}

fn interleave(pages: Vec<(&'static str, Vec<ProviderWallpaper>)>) -> Vec<Wallpaper> {
    let total = pages.iter().map(|(_, items)| items.len()).sum();
    let mut iterators: Vec<_> = pages.into_iter().map(|(provider, items)| (provider, items.into_iter())).collect();
    let mut merged = Vec::with_capacity(total);
    while merged.len() < total {
        for (provider, items) in iterators.iter_mut() {
            if let Some(item) = items.next() {
                merged.push(item.into_wallpaper(provider));
            }
        }
    }
    merged
}

/// Maps a non-success HTTP status from a provider API to a message that
/// tells the user what to do.
pub(crate) fn status_error(provider_name: &str, status: StatusCode) -> AppError {
    let message = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            format!("{provider_name} rejected the API key. Check it in Settings.")
        }
        StatusCode::TOO_MANY_REQUESTS => {
            format!("{provider_name} rate limit reached. Wait a minute and search again.")
        }
        status if status.is_server_error() => {
            format!("{provider_name} is unavailable right now (HTTP {}).", status.as_u16())
        }
        status => format!("{provider_name} returned HTTP {}.", status.as_u16()),
    };
    AppError::Provider(message)
}

/// Accepts a JSON number or a numeric string. Some APIs are inconsistent
/// about which one they send for pagination fields.
pub(crate) fn de_flexible_u32<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Number {
        Int(u64),
        Text(String),
    }

    match Number::deserialize(deserializer)? {
        Number::Int(value) => u32::try_from(value).map_err(serde::de::Error::custom),
        Number::Text(value) => value.trim().parse().map_err(serde::de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeProvider {
        metadata: ProviderMetadata,
        result: fn(u32) -> AppResult<ProviderPage>,
    }

    #[async_trait]
    impl WallpaperProvider for FakeProvider {
        fn metadata(&self) -> &ProviderMetadata {
            &self.metadata
        }

        fn image_hosts(&self) -> &[&'static str] {
            &["img.example.com"]
        }

        async fn search(&self, request: &SearchRequest<'_>) -> AppResult<ProviderPage> {
            (self.result)(request.page)
        }
    }

    fn fake(id: &'static str, result: fn(u32) -> AppResult<ProviderPage>) -> Arc<dyn WallpaperProvider> {
        Arc::new(FakeProvider {
            metadata: ProviderMetadata {
                id,
                name: id,
                website: "https://example.com",
                api_key_requirement: ApiKeyRequirement::None,
                api_key_url: None,
                attribution: None,
                link_hosts: &["example.com"],
            },
            result,
        })
    }

    fn item(id: &str) -> ProviderWallpaper {
        ProviderWallpaper {
            provider_id: id.into(),
            title: id.into(),
            image_url: format!("https://img.example.com/{id}.jpg"),
            thumbnail_url: format!("https://img.example.com/{id}_t.jpg"),
            width: 1920,
            height: 1080,
            credit: Credit::default(),
        }
    }

    #[test]
    fn registering_a_new_provider_keeps_existing_ones() {
        let mut registry = ProviderRegistry::default();
        registry.register(fake("a", |_| Ok(ProviderPage::default())));
        registry.register(fake("b", |_| Ok(ProviderPage::default())));
        registry.register(fake("a", |_| Ok(ProviderPage::default())));
        let ids: Vec<_> = registry.iter().map(|p| p.id()).collect();
        assert_eq!(ids, ["a", "b"]);
    }

    #[test]
    fn default_registry_contains_every_bundled_provider() {
        let registry = ProviderRegistry::with_defaults(reqwest::Client::new());
        let ids: Vec<_> = registry.iter().map(|p| p.id()).collect();
        assert_eq!(ids, ["wallhaven", "unsplash", "pexels", "wikimedia"]);
    }

    #[test]
    fn validate_input_rejects_foreign_hosts() {
        let mut registry = ProviderRegistry::default();
        registry.register(fake("a", |_| Ok(ProviderPage::default())));
        let mut input = WallpaperInput {
            provider: "a".into(),
            provider_id: "1".into(),
            title: String::new(),
            image_url: "https://img.example.com/1.jpg".into(),
            thumbnail_url: "https://img.example.com/1_t.jpg".into(),
            width: 0,
            height: 0,
            credit: Credit {
                author: Some("Ada".into()),
                author_url: Some("https://example.com/@ada".into()),
                source_url: Some("https://example.com/photos/1".into()),
                license: None,
                license_url: None,
            },
        };
        assert!(registry.validate_input(&input).is_ok());

        input.credit.source_url = Some("https://evil.example.org/photos/1".into());
        assert!(registry.validate_input(&input).is_err(), "credit links must point to the provider's hosts");
        input.credit.source_url = Some("javascript:alert(1)".into());
        assert!(registry.validate_input(&input).is_err());
        input.credit.source_url = None;
        input.credit.author = Some("x".repeat(MAX_CREDIT_TEXT + 1));
        assert!(registry.validate_input(&input).is_err());
        input.credit.author = None;

        input.image_url = "http://img.example.com/1.jpg".into();
        assert!(registry.validate_input(&input).is_err());

        input.image_url = "https://evil.example.org/1.jpg".into();
        assert!(registry.validate_input(&input).is_err());

        input.image_url = "file:///etc/passwd".into();
        assert!(registry.validate_input(&input).is_err());

        input.image_url = "https://img.example.com/1.jpg".into();
        input.provider = "missing".into();
        assert!(registry.validate_input(&input).is_err());
    }

    #[tokio::test]
    async fn search_interleaves_results_and_reports_failures() {
        let targets = vec![
            SearchTarget {
                provider: fake("a", |_| {
                    Ok(ProviderPage { wallpapers: vec![item("a1"), item("a2"), item("a3")], has_more: true })
                }),
                api_key: None,
            },
            SearchTarget {
                provider: fake("b", |_| Ok(ProviderPage { wallpapers: vec![item("b1")], has_more: false })),
                api_key: None,
            },
            SearchTarget { provider: fake("c", |_| Err(AppError::Provider("boom".into()))), api_key: None },
        ];

        let result = search_providers(targets, "forest", 1).await;
        let ids: Vec<_> = result.wallpapers.iter().map(|w| w.provider_id.as_str()).collect();
        assert_eq!(ids, ["a1", "b1", "a2", "a3"]);
        assert_eq!(result.providers_with_more, ["a"]);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].provider, "c");
    }

    #[test]
    fn flexible_u32_accepts_numbers_and_strings() {
        #[derive(Deserialize)]
        struct Probe {
            #[serde(deserialize_with = "de_flexible_u32")]
            value: u32,
        }
        let a: Probe = serde_json::from_str(r#"{"value":24}"#).unwrap();
        let b: Probe = serde_json::from_str(r#"{"value":"24"}"#).unwrap();
        assert_eq!(a.value, 24);
        assert_eq!(b.value, 24);
    }
}
