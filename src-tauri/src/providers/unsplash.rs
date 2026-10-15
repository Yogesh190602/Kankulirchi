//! Unsplash provider: <https://unsplash.com/documentation>
//!
//! Unsplash requires a free access key, configured in Settings. Results are
//! landscape photos with the strict content filter. Following the API
//! guidelines, images are loaded from the URLs the API returns (sized through
//! its CDN parameters), the photographer is credited in the title, and using
//! a photo is reported to the download endpoint.

use async_trait::async_trait;
use reqwest::header::{HeaderMap, AUTHORIZATION};
use reqwest::StatusCode;
use serde::Deserialize;
use url::Url;

use super::{
    credit_text, status_error, ApiKeyRequirement, ProviderMetadata, ProviderPage, ProviderWallpaper, SearchRequest,
    WallpaperProvider, PAGE_SIZE, REQUEST_TIMEOUT,
};
use crate::error::{AppError, AppResult};
use crate::models::Credit;

const API_URL: &str = "https://api.unsplash.com/";
const SEARCH_URL: &str = "https://api.unsplash.com/search/photos";
const API_VERSION: (&str, &str) = ("Accept-Version", "v1");

/// CDN parameters for the downloaded wallpaper: at most 4K wide. Unsplash
/// scales larger originals down and never scales smaller ones up.
const IMAGE_PARAMS: &[(&str, &str)] = &[("w", "3840"), ("q", "85"), ("fm", "jpg"), ("fit", "max")];
const THUMBNAIL_PARAMS: &[(&str, &str)] = &[("w", "640"), ("q", "75"), ("fm", "jpg"), ("fit", "max")];

const METADATA: ProviderMetadata = ProviderMetadata {
    id: "unsplash",
    name: "Unsplash",
    website: "https://unsplash.com/?utm_source=kankulirchi&utm_medium=referral",
    api_key_requirement: ApiKeyRequirement::Required,
    api_key_url: Some("https://unsplash.com/oauth/applications"),
    attribution: Some("Photos provided by Unsplash"),
    link_hosts: &["unsplash.com"],
};

/// Free photos only. Unsplash+ photos are served from another host and are
/// left out of the results.
const IMAGE_HOSTS: &[&str] = &["images.unsplash.com"];

pub struct UnsplashProvider {
    client: reqwest::Client,
}

impl UnsplashProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    total_pages: u32,
    results: Vec<Photo>,
}

#[derive(Debug, Deserialize)]
struct Photo {
    id: String,
    width: u32,
    height: u32,
    #[serde(default)]
    alt_description: Option<String>,
    urls: Urls,
    #[serde(default)]
    user: Option<User>,
    #[serde(default)]
    links: Option<Links>,
}

#[derive(Debug, Deserialize)]
struct Urls {
    raw: String,
}

#[derive(Debug, Deserialize)]
struct User {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    links: Option<Links>,
}

#[derive(Debug, Deserialize)]
struct Links {
    /// The page on unsplash.com: the photo's, or the photographer's profile.
    html: String,
}

/// Unsplash asks for links back to it to carry these referral parameters.
const REFERRAL_PARAMS: &[(&str, &str)] = &[("utm_source", "kankulirchi"), ("utm_medium", "referral")];

fn photo_title(photo: &Photo) -> String {
    match photo.alt_description.as_deref().map(str::trim) {
        Some(alt) if !alt.is_empty() => alt.to_owned(),
        _ => format!("Unsplash photo {}", photo.id),
    }
}

/// The photographer, linked to their profile, and the photo's page, as the
/// Unsplash guidelines ask.
fn photo_credit(photo: &Photo) -> Credit {
    let user = photo.user.as_ref();
    Credit {
        author: user.and_then(|user| user.name.as_deref()).and_then(credit_text),
        author_url: user.and_then(|user| user.links.as_ref()).and_then(|links| referral_link(&links.html)),
        source_url: photo.links.as_ref().and_then(|links| referral_link(&links.html)),
        ..Credit::default()
    }
}

/// An unsplash.com link with the referral parameters. `None` for any other
/// address.
fn referral_link(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw).ok()?;
    if url.scheme() != "https" || !url.host_str().is_some_and(|host| METADATA.link_hosts.contains(&host)) {
        return None;
    }
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !key.starts_with("utm_"))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.query_pairs_mut().clear().extend_pairs(kept).extend_pairs(REFERRAL_PARAMS);
    Some(url.into())
}

/// `raw` with CDN sizing parameters, replacing any already present. `None`
/// if the URL is not a free Unsplash image.
fn sized_url(raw: &str, params: &[(&str, &str)]) -> Option<String> {
    let mut url = Url::parse(raw).ok()?;
    if url.scheme() != "https" || !url.host_str().is_some_and(|host| IMAGE_HOSTS.contains(&host)) {
        return None;
    }
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| !params.iter().any(|(name, _)| key == name))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.query_pairs_mut().clear().extend_pairs(kept).extend_pairs(params);
    Some(url.into())
}

fn parse_page(body: SearchResponse, page: u32) -> ProviderPage {
    let wallpapers = body
        .results
        .into_iter()
        .filter_map(|photo| {
            Some(ProviderWallpaper {
                title: photo_title(&photo),
                credit: photo_credit(&photo),
                image_url: sized_url(&photo.urls.raw, IMAGE_PARAMS)?,
                thumbnail_url: sized_url(&photo.urls.raw, THUMBNAIL_PARAMS)?,
                provider_id: photo.id,
                width: photo.width,
                height: photo.height,
            })
        })
        .collect();
    ProviderPage { wallpapers, has_more: page < body.total_pages }
}

/// Unsplash answers 403 rather than 429 once the hourly request limit is
/// used up.
fn response_error(status: StatusCode, headers: &HeaderMap) -> AppError {
    let limit_reached = headers
        .get("x-ratelimit-remaining")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim() == "0");
    if limit_reached {
        return AppError::Provider("Unsplash's hourly request limit is used up. Try again later.".into());
    }
    status_error(METADATA.name, status)
}

fn authorization(api_key: &str) -> String {
    format!("Client-ID {api_key}")
}

#[async_trait]
impl WallpaperProvider for UnsplashProvider {
    fn metadata(&self) -> &ProviderMetadata {
        &METADATA
    }

    fn image_hosts(&self) -> &[&'static str] {
        IMAGE_HOSTS
    }

    async fn search(&self, request: &SearchRequest<'_>) -> AppResult<ProviderPage> {
        let api_key = request
            .api_key
            .ok_or_else(|| AppError::Provider("Add an Unsplash access key in Settings to search Unsplash.".into()))?;
        let page = request.page.to_string();
        let per_page = PAGE_SIZE.to_string();
        let query = [
            ("query", request.keyword),
            ("page", page.as_str()),
            ("per_page", per_page.as_str()),
            ("orientation", "landscape"),
            ("content_filter", "high"),
        ];

        let response = self
            .client
            .get(SEARCH_URL)
            .header(AUTHORIZATION, authorization(api_key))
            .header(API_VERSION.0, API_VERSION.1)
            .timeout(REQUEST_TIMEOUT)
            .query(&query)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(response_error(status, response.headers()));
        }
        Ok(parse_page(response.json().await?, request.page))
    }

    async fn report_download(&self, provider_id: &str, api_key: Option<&str>) -> AppResult<()> {
        let Some(api_key) = api_key else { return Ok(()) };
        let mut url = Url::parse(API_URL).map_err(|error| AppError::Internal(error.to_string()))?;
        url.path_segments_mut()
            .map_err(|()| AppError::Internal("The Unsplash API URL cannot have a path.".into()))?
            .pop_if_empty()
            .extend(["photos", provider_id, "download"]);

        let response = self
            .client
            .get(url)
            .header(AUTHORIZATION, authorization(api_key))
            .header(API_VERSION.0, API_VERSION.1)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(response_error(status, response.headers()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{
      "total": 3,
      "total_pages": 2,
      "results": [
        {
          "id": "eOLpJytrbsQ",
          "width": 6000,
          "height": 4000,
          "description": null,
          "alt_description": "snow covered mountain under blue sky",
          "urls": {
            "raw": "https://images.unsplash.com/photo-1464822759023-fed622ff2c3b?ixid=M3w2&ixlib=rb-4.0.3",
            "full": "https://images.unsplash.com/photo-1464822759023-fed622ff2c3b?ixid=M3w2&ixlib=rb-4.0.3&q=85",
            "regular": "https://images.unsplash.com/photo-1464822759023-fed622ff2c3b?ixid=M3w2&w=1080",
            "small": "https://images.unsplash.com/photo-1464822759023-fed622ff2c3b?ixid=M3w2&w=400",
            "thumb": "https://images.unsplash.com/photo-1464822759023-fed622ff2c3b?ixid=M3w2&w=200"
          },
          "links": {
            "html": "https://unsplash.com/photos/eOLpJytrbsQ",
            "download_location": "https://api.unsplash.com/photos/eOLpJytrbsQ/download?ixid=M3w2"
          },
          "user": {
            "name": "Kalen Emsley",
            "username": "kalenemsley",
            "links": { "html": "https://unsplash.com/@kalenemsley?utm_source=other" }
          }
        },
        {
          "id": "plus1",
          "width": 5000,
          "height": 3000,
          "urls": { "raw": "https://plus.unsplash.com/premium_photo-1?ixid=M3w2" },
          "user": { "name": "Someone" }
        },
        {
          "id": "noname",
          "width": 4000,
          "height": 2500,
          "alt_description": "  lake at dawn ",
          "urls": { "raw": "https://images.unsplash.com/photo-2?ixid=M3w2&w=100" },
          "user": { "name": null }
        }
      ]
    }"##;

    fn query(url: &str) -> Vec<(String, String)> {
        Url::parse(url).unwrap().query_pairs().into_owned().collect()
    }

    #[test]
    fn parses_search_response_and_skips_unsplash_plus() {
        let page = parse_page(serde_json::from_str(SAMPLE).unwrap(), 1);
        assert!(page.has_more);
        let ids: Vec<_> = page.wallpapers.iter().map(|w| w.provider_id.as_str()).collect();
        assert_eq!(ids, ["eOLpJytrbsQ", "noname"]);

        let first = &page.wallpapers[0];
        assert_eq!(first.title, "snow covered mountain under blue sky");
        assert_eq!((first.width, first.height), (6000, 4000));
        assert!(first.image_url.starts_with("https://images.unsplash.com/photo-1464822759023-fed622ff2c3b?"));
        assert_eq!(page.wallpapers[1].title, "lake at dawn");
    }

    #[test]
    fn credits_link_to_the_photographer_and_photo_with_referral_parameters() {
        let page = parse_page(serde_json::from_str(SAMPLE).unwrap(), 1);
        let credit = &page.wallpapers[0].credit;
        assert_eq!(credit.author.as_deref(), Some("Kalen Emsley"));
        assert_eq!(
            credit.author_url.as_deref(),
            Some("https://unsplash.com/@kalenemsley?utm_source=kankulirchi&utm_medium=referral")
        );
        assert_eq!(
            credit.source_url.as_deref(),
            Some("https://unsplash.com/photos/eOLpJytrbsQ?utm_source=kankulirchi&utm_medium=referral")
        );
        let unnamed = &page.wallpapers[1].credit;
        assert_eq!((unnamed.author.as_deref(), unnamed.source_url.as_deref()), (None, None));
        assert_eq!(referral_link("https://evil.example.com/@kalenemsley"), None);
    }

    #[test]
    fn sized_urls_keep_tracking_parameters_and_replace_sizes() {
        let url = sized_url("https://images.unsplash.com/photo-2?ixid=M3w2&w=100", IMAGE_PARAMS).unwrap();
        let pairs = query(&url);
        assert!(pairs.contains(&("ixid".into(), "M3w2".into())));
        assert_eq!(pairs.iter().filter(|(key, _)| key == "w").count(), 1);
        assert!(pairs.contains(&("w".into(), "3840".into())));
        assert!(pairs.contains(&("fit".into(), "max".into())));
    }

    #[test]
    fn sized_urls_reject_other_hosts() {
        assert!(sized_url("http://images.unsplash.com/photo-2", IMAGE_PARAMS).is_none());
        assert!(sized_url("https://plus.unsplash.com/premium_photo-1", IMAGE_PARAMS).is_none());
        assert!(sized_url("not a url", IMAGE_PARAMS).is_none());
    }

    #[test]
    fn last_page_has_no_more() {
        let page = parse_page(serde_json::from_str(r#"{"total_pages": 2, "results": []}"#).unwrap(), 2);
        assert!(!page.has_more);
    }

    #[test]
    fn exhausted_rate_limit_is_explained() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-remaining", "0".parse().unwrap());
        let error = response_error(StatusCode::FORBIDDEN, &headers);
        assert!(error.to_string().contains("hourly request limit"));

        let error = response_error(StatusCode::UNAUTHORIZED, &HeaderMap::new());
        assert!(error.to_string().contains("API key"));
    }

    #[tokio::test]
    async fn search_without_api_key_fails_before_any_request() {
        let provider = UnsplashProvider::new(reqwest::Client::new());
        let request = SearchRequest { keyword: "forest", page: 1, api_key: None };
        let error = provider.search(&request).await.unwrap_err();
        assert!(error.to_string().contains("access key"));
    }

    #[tokio::test]
    async fn downloads_are_not_reported_without_api_key() {
        let provider = UnsplashProvider::new(reqwest::Client::new());
        assert!(provider.report_download("eOLpJytrbsQ", None).await.is_ok());
    }
}
