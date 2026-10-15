//! Wallhaven provider: <https://wallhaven.cc/help/api>
//!
//! Search works without an API key. Results are limited to landscape
//! wallpapers, which suit a desktop background, rated safe for work. The SFW
//! purity is always requested, even with an API key whose account allows
//! more, and results rated otherwise are dropped as well.

use async_trait::async_trait;
use serde::Deserialize;

use super::{
    de_flexible_u32, https_link, status_error, ApiKeyRequirement, ProviderMetadata, ProviderPage, ProviderWallpaper,
    SearchRequest, WallpaperProvider, REQUEST_TIMEOUT,
};
use crate::error::AppResult;
use crate::models::Credit;

const SEARCH_URL: &str = "https://wallhaven.cc/api/v1/search";
const API_KEY_HEADER: &str = "X-API-Key";

const METADATA: ProviderMetadata = ProviderMetadata {
    id: "wallhaven",
    name: "Wallhaven",
    website: "https://wallhaven.cc",
    api_key_requirement: ApiKeyRequirement::Optional,
    api_key_url: Some("https://wallhaven.cc/settings/account"),
    attribution: None,
    link_hosts: &["wallhaven.cc"],
};

const IMAGE_HOSTS: &[&str] = &["w.wallhaven.cc", "th.wallhaven.cc"];

/// Wallhaven's rating for safe-for-work wallpapers, as opposed to "sketchy"
/// and "nsfw".
const SAFE_PURITY: &str = "sfw";

pub struct WallhavenProvider {
    client: reqwest::Client,
}

impl WallhavenProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }

    fn search_request(&self, request: &SearchRequest<'_>) -> reqwest::RequestBuilder {
        let page = request.page.to_string();
        let query = [
            ("q", request.keyword),
            ("page", &page),
            ("categories", "111"),
            ("purity", "100"),
            ("ratios", "landscape"),
            ("sorting", "relevance"),
            ("order", "desc"),
        ];
        let mut search = self.client.get(SEARCH_URL).timeout(REQUEST_TIMEOUT).query(&query);
        // A header rather than the `apikey` parameter, so the key is not part
        // of the URL.
        if let Some(key) = request.api_key {
            search = search.header(API_KEY_HEADER, key);
        }
        search
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    data: Vec<Item>,
    meta: Meta,
}

#[derive(Debug, Deserialize)]
struct Meta {
    #[serde(deserialize_with = "de_flexible_u32")]
    current_page: u32,
    #[serde(deserialize_with = "de_flexible_u32")]
    last_page: u32,
}

#[derive(Debug, Deserialize)]
struct Item {
    id: String,
    /// The wallpaper's page on wallhaven.cc.
    #[serde(default)]
    url: String,
    #[serde(default)]
    purity: String,
    dimension_x: u32,
    dimension_y: u32,
    path: String,
    thumbs: Thumbs,
}

#[derive(Debug, Deserialize)]
struct Thumbs {
    large: String,
}

fn parse_page(body: SearchResponse) -> ProviderPage {
    let wallpapers = body
        .data
        .into_iter()
        .filter(|item| item.purity == SAFE_PURITY)
        .map(|item| ProviderWallpaper {
            title: format!("Wallhaven {}", item.id),
            provider_id: item.id,
            image_url: item.path,
            thumbnail_url: item.thumbs.large,
            width: item.dimension_x,
            height: item.dimension_y,
            credit: Credit { source_url: https_link(&item.url, METADATA.link_hosts), ..Credit::default() },
        })
        .collect();
    ProviderPage { wallpapers, has_more: body.meta.current_page < body.meta.last_page }
}

#[async_trait]
impl WallpaperProvider for WallhavenProvider {
    fn metadata(&self) -> &ProviderMetadata {
        &METADATA
    }

    fn image_hosts(&self) -> &[&'static str] {
        IMAGE_HOSTS
    }

    async fn search(&self, request: &SearchRequest<'_>) -> AppResult<ProviderPage> {
        let response = self.search_request(request).send().await?;
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

    #[test]
    fn api_key_is_sent_in_a_header_not_the_url() {
        let provider = WallhavenProvider::new(reqwest::Client::new());
        let request = SearchRequest { keyword: "lake", page: 1, api_key: Some("SECRET123") };
        let built = provider.search_request(&request).build().unwrap();
        assert!(!built.url().as_str().contains("SECRET123"), "{}", built.url());
        assert_eq!(built.headers()[API_KEY_HEADER], "SECRET123");
        assert!(built.url().query_pairs().any(|(key, value)| key == "purity" && value == "100"));
    }

    const SAMPLE: &str = r##"{
      "data": [{
        "id": "94x38z",
        "url": "https://wallhaven.cc/w/94x38z",
        "purity": "sfw",
        "category": "anime",
        "dimension_x": 6742,
        "dimension_y": 3534,
        "resolution": "6742x3534",
        "file_type": "image/jpeg",
        "created_at": "2018-10-31 01:23:10",
        "colors": ["#000000"],
        "path": "https://w.wallhaven.cc/full/94/wallhaven-94x38z.jpg",
        "thumbs": {
          "large": "https://th.wallhaven.cc/lg/94/94x38z.jpg",
          "original": "https://th.wallhaven.cc/orig/94/94x38z.jpg",
          "small": "https://th.wallhaven.cc/small/94/94x38z.jpg"
        }
      }],
      "meta": {
        "current_page": 1,
        "last_page": 3,
        "per_page": "24",
        "total": 60,
        "query": {"id": 1, "tag": "anime"},
        "seed": null
      }
    }"##;

    #[test]
    fn parses_search_response() {
        let page = parse_page(serde_json::from_str(SAMPLE).unwrap());
        assert!(page.has_more);
        assert_eq!(page.wallpapers.len(), 1);
        let wallpaper = &page.wallpapers[0];
        assert_eq!(wallpaper.provider_id, "94x38z");
        assert_eq!(wallpaper.image_url, "https://w.wallhaven.cc/full/94/wallhaven-94x38z.jpg");
        assert_eq!(wallpaper.thumbnail_url, "https://th.wallhaven.cc/lg/94/94x38z.jpg");
        assert_eq!((wallpaper.width, wallpaper.height), (6742, 3534));
    }

    #[test]
    fn last_page_has_no_more() {
        let body = SAMPLE.replace("\"current_page\": 1", "\"current_page\": 3");
        let page = parse_page(serde_json::from_str(&body).unwrap());
        assert!(!page.has_more);
    }

    #[test]
    fn only_safe_for_work_results_are_kept() {
        for purity in ["\"sketchy\"", "\"nsfw\"", "\"\""] {
            let body = SAMPLE.replace("\"sfw\"", purity);
            let page = parse_page(serde_json::from_str(&body).unwrap());
            assert!(page.wallpapers.is_empty(), "purity {purity} is dropped");
        }
        let body = SAMPLE.replace("\"purity\": \"sfw\",", "");
        assert!(parse_page(serde_json::from_str(&body).unwrap()).wallpapers.is_empty(), "unrated results are dropped");
    }
}
