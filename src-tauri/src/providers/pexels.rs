//! Pexels provider: <https://www.pexels.com/api/documentation/>
//!
//! Pexels requires a free API key, configured in Settings. Results are
//! limited to landscape photos, which suit a desktop background. The API has
//! no content filter; Pexels does not accept explicit uploads, and
//! [`safe_search`](super::safe_search) refuses adult keywords first.

use async_trait::async_trait;
use reqwest::header::AUTHORIZATION;
use serde::Deserialize;

use super::{
    credit_text, https_link, status_error, ApiKeyRequirement, ProviderMetadata, ProviderPage, ProviderWallpaper,
    SearchRequest, WallpaperProvider, PAGE_SIZE, REQUEST_TIMEOUT,
};
use crate::error::{AppError, AppResult};
use crate::models::Credit;

const SEARCH_URL: &str = "https://api.pexels.com/v1/search";

const METADATA: ProviderMetadata = ProviderMetadata {
    id: "pexels",
    name: "Pexels",
    website: "https://www.pexels.com",
    api_key_requirement: ApiKeyRequirement::Required,
    api_key_url: Some("https://www.pexels.com/api/"),
    attribution: Some("Photos provided by Pexels"),
    link_hosts: &["www.pexels.com"],
};

const IMAGE_HOSTS: &[&str] = &["images.pexels.com"];

pub struct PexelsProvider {
    client: reqwest::Client,
}

impl PexelsProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    photos: Vec<Photo>,
    next_page: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Photo {
    id: u64,
    width: u32,
    height: u32,
    #[serde(default)]
    alt: Option<String>,
    #[serde(default)]
    photographer: Option<String>,
    #[serde(default)]
    photographer_url: Option<String>,
    /// The photo's page on pexels.com.
    #[serde(default)]
    url: Option<String>,
    src: Sources,
}

#[derive(Debug, Deserialize)]
struct Sources {
    original: String,
    medium: String,
}

fn photo_title(photo: &Photo) -> String {
    let alt = photo.alt.as_deref().map(str::trim).unwrap_or_default();
    if !alt.is_empty() {
        return alt.to_owned();
    }
    format!("Pexels photo {}", photo.id)
}

/// The photographer and the photo's page, as the Pexels guidelines ask.
fn photo_credit(photo: &Photo) -> Credit {
    let link = |url: &Option<String>| url.as_deref().and_then(|url| https_link(url, METADATA.link_hosts));
    Credit {
        author: photo.photographer.as_deref().and_then(credit_text),
        author_url: link(&photo.photographer_url),
        source_url: link(&photo.url),
        ..Credit::default()
    }
}

fn parse_page(body: SearchResponse) -> ProviderPage {
    let wallpapers = body
        .photos
        .into_iter()
        .map(|photo| ProviderWallpaper {
            title: photo_title(&photo),
            credit: photo_credit(&photo),
            provider_id: photo.id.to_string(),
            image_url: photo.src.original,
            thumbnail_url: photo.src.medium,
            width: photo.width,
            height: photo.height,
        })
        .collect();
    ProviderPage { wallpapers, has_more: body.next_page.is_some() }
}

#[async_trait]
impl WallpaperProvider for PexelsProvider {
    fn metadata(&self) -> &ProviderMetadata {
        &METADATA
    }

    fn image_hosts(&self) -> &[&'static str] {
        IMAGE_HOSTS
    }

    async fn search(&self, request: &SearchRequest<'_>) -> AppResult<ProviderPage> {
        let api_key = request
            .api_key
            .ok_or_else(|| AppError::Provider("Add a Pexels API key in Settings to search Pexels.".into()))?;
        let page = request.page.to_string();
        let per_page = PAGE_SIZE.to_string();
        let query = [
            ("query", request.keyword),
            ("page", page.as_str()),
            ("per_page", per_page.as_str()),
            ("orientation", "landscape"),
        ];

        let response = self
            .client
            .get(SEARCH_URL)
            .header(AUTHORIZATION, api_key)
            .timeout(REQUEST_TIMEOUT)
            .query(&query)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_error(METADATA.name, status));
        }
        Ok(parse_page(response.json().await?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{
      "total_results": 10000,
      "page": 1,
      "per_page": 2,
      "photos": [
        {
          "id": 3573351,
          "width": 3066,
          "height": 3968,
          "url": "https://www.pexels.com/photo/trees-during-day-3573351/",
          "photographer": "Lukas Rodriguez",
          "photographer_url": "https://www.pexels.com/@lukas-rodriguez-1845331",
          "photographer_id": 1845331,
          "avg_color": "#374824",
          "src": {
            "original": "https://images.pexels.com/photos/3573351/pexels-photo-3573351.png",
            "large2x": "https://images.pexels.com/photos/3573351/pexels-photo-3573351.png?auto=compress&cs=tinysrgb&dpr=2&h=650&w=940",
            "large": "https://images.pexels.com/photos/3573351/pexels-photo-3573351.png?auto=compress&cs=tinysrgb&h=650&w=940",
            "medium": "https://images.pexels.com/photos/3573351/pexels-photo-3573351.png?auto=compress&cs=tinysrgb&h=350",
            "small": "https://images.pexels.com/photos/3573351/pexels-photo-3573351.png?auto=compress&cs=tinysrgb&h=130",
            "portrait": "https://images.pexels.com/photos/3573351/pexels-photo-3573351.png?auto=compress&cs=tinysrgb&fit=crop&h=1200&w=800",
            "landscape": "https://images.pexels.com/photos/3573351/pexels-photo-3573351.png?auto=compress&cs=tinysrgb&fit=crop&h=627&w=1200",
            "tiny": "https://images.pexels.com/photos/3573351/pexels-photo-3573351.png?auto=compress&cs=tinysrgb&dpr=1&fit=crop&h=200&w=280"
          },
          "liked": false,
          "alt": "Brown Rocks During Golden Hour"
        },
        {
          "id": 42,
          "width": 1920,
          "height": 1080,
          "photographer": "Ada",
          "src": {
            "original": "https://images.pexels.com/photos/42/a.jpeg",
            "medium": "https://images.pexels.com/photos/42/a.jpeg?h=350"
          },
          "alt": ""
        }
      ],
      "next_page": "https://api.pexels.com/v1/search/?page=2&per_page=2&query=nature"
    }"##;

    #[test]
    fn parses_search_response() {
        let page = parse_page(serde_json::from_str(SAMPLE).unwrap());
        assert!(page.has_more);
        assert_eq!(page.wallpapers.len(), 2);
        let first = &page.wallpapers[0];
        assert_eq!(first.provider_id, "3573351");
        assert_eq!(first.title, "Brown Rocks During Golden Hour");
        assert_eq!(first.image_url, "https://images.pexels.com/photos/3573351/pexels-photo-3573351.png");
        assert_eq!(page.wallpapers[1].title, "Pexels photo 42");
    }

    #[test]
    fn credits_the_photographer_and_links_to_the_photo() {
        let page = parse_page(serde_json::from_str(SAMPLE).unwrap());
        let credit = &page.wallpapers[0].credit;
        assert_eq!(credit.author.as_deref(), Some("Lukas Rodriguez"));
        assert_eq!(credit.author_url.as_deref(), Some("https://www.pexels.com/@lukas-rodriguez-1845331"));
        assert_eq!(credit.source_url.as_deref(), Some("https://www.pexels.com/photo/trees-during-day-3573351/"));
        let second = &page.wallpapers[1].credit;
        assert_eq!(second.author.as_deref(), Some("Ada"));
        assert_eq!(second.source_url, None);
    }

    #[test]
    fn missing_next_page_means_no_more() {
        let body = r#"{"photos": [], "next_page": null}"#;
        let page = parse_page(serde_json::from_str(body).unwrap());
        assert!(!page.has_more);
        assert!(page.wallpapers.is_empty());
    }

    #[tokio::test]
    async fn search_without_api_key_fails_before_any_request() {
        let provider = PexelsProvider::new(reqwest::Client::new());
        let request = SearchRequest { keyword: "forest", page: 1, api_key: None };
        let error = provider.search(&request).await.unwrap_err();
        assert!(error.to_string().contains("API key"));
    }
}
