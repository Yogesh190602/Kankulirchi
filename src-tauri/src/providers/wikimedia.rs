//! Wikimedia Commons provider: <https://commons.wikimedia.org/wiki/Commons:Featured_pictures>
//!
//! Searches the featured pictures of Wikimedia Commons, the photos its
//! community picked as the finest on the site. No API key is needed. Only
//! landscape JPEG and PNG files at least [`MIN_WIDTH`] pixels wide are
//! returned, and originals wider than [`MAX_WIDTH`] are downloaded as copies
//! scaled by Wikimedia.
//!
//! Commons has no adult-content filter, and its featured pictures include
//! nude photography, nude art and war photographs. Searches are therefore
//! limited to the scenery topics in [`SAFE_TOPICS`].

use std::collections::HashMap;

use async_trait::async_trait;
use serde::Deserialize;

use super::{
    credit_text, https_link, status_error, ApiKeyRequirement, ProviderMetadata, ProviderPage, ProviderWallpaper,
    SearchRequest, WallpaperProvider, PAGE_SIZE, REQUEST_TIMEOUT,
};
use crate::error::{AppError, AppResult};
use crate::models::Credit;

const API_URL: &str = "https://commons.wikimedia.org/w/api.php";

/// Featured-picture topics searched, as in "Category:Featured pictures of
/// landscapes". Only pictures filed directly in these categories match, not
/// those in subcategories, which reach into people and art.
const SAFE_TOPICS: &[&str] = &[
    "landscapes",
    "mountains",
    "lakes",
    "waterfalls",
    "beaches",
    "forests",
    "sunsets",
    "clouds",
    "skylines",
    "cityscapes",
    "astronomy",
    "flowers",
];

const MIN_WIDTH: u32 = 1920;
const MAX_WIDTH: u32 = 3840;
const THUMBNAIL_WIDTH: u32 = 960;

/// Files per credit lookup, the most the API accepts in one request.
const CREDIT_BATCH: usize = 50;

/// Accepted width-to-height ratios, as tenths: from 1.2 (wider than 5:4)
/// to 2.5 (a little wider than 21:9). Panoramas beyond that do not fill a
/// screen sensibly.
const MIN_RATIO_TENTHS: u64 = 12;
const MAX_RATIO_TENTHS: u64 = 25;

const SUPPORTED_MIME_TYPES: [&str; 2] = ["image/jpeg", "image/png"];

const METADATA: ProviderMetadata = ProviderMetadata {
    id: "wikimedia",
    name: "Wikimedia Commons",
    website: "https://commons.wikimedia.org/wiki/Commons:Featured_pictures",
    api_key_requirement: ApiKeyRequirement::None,
    api_key_url: None,
    attribution: Some("Featured pictures from Wikimedia Commons"),
    // Commons file pages and author pages, and the licenses they link to.
    link_hosts: &["commons.wikimedia.org", "creativecommons.org", "www.gnu.org"],
};

const IMAGE_HOSTS: &[&str] = &["upload.wikimedia.org", "thumb.wikimedia.org"];

pub struct WikimediaProvider {
    client: reqwest::Client,
}

impl WikimediaProvider {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[serde(default)]
    query: Option<Query>,
    /// Present when more results follow.
    #[serde(default, rename = "continue")]
    continuation: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<ApiError>,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    #[serde(default)]
    info: String,
}

#[derive(Debug, Deserialize)]
struct Query {
    #[serde(default)]
    pages: Vec<FilePage>,
}

#[derive(Debug, Deserialize)]
struct FilePage {
    pageid: u64,
    title: String,
    /// Position in the search results; pages arrive in no particular order.
    #[serde(default)]
    index: u32,
    #[serde(default)]
    imageinfo: Vec<ImageInfo>,
}

#[derive(Debug, Deserialize)]
struct ImageInfo {
    width: u32,
    height: u32,
    url: String,
    #[serde(default)]
    thumburl: Option<String>,
    #[serde(default)]
    mime: String,
    /// The file's page on Commons.
    #[serde(default)]
    descriptionurl: Option<String>,
    #[serde(default)]
    extmetadata: ExtMetadata,
}

/// The fields of Commons' file metadata that make up the attribution its
/// licenses require.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
struct ExtMetadata {
    /// HTML, often a link to the author's user page.
    artist: Option<MetadataValue>,
    license_short_name: Option<MetadataValue>,
    license_url: Option<MetadataValue>,
}

#[derive(Debug, Deserialize)]
struct MetadataValue {
    #[serde(default)]
    value: serde_json::Value,
}

impl MetadataValue {
    fn text(&self) -> Option<&str> {
        self.value.as_str()
    }
}

/// The CirrusSearch query: `keyword` among featured pictures of the safe
/// topics that are raster images wide enough for a desktop.
fn search_text(keyword: &str) -> String {
    let topics: Vec<String> = SAFE_TOPICS.iter().map(|topic| format!("Featured_pictures_of_{topic}")).collect();
    format!("{keyword} incategory:{} filetype:bitmap filew:>{}", topics.join("|"), MIN_WIDTH - 1)
}

/// "File:Mount Fuji at dawn.jpg" becomes "Mount Fuji at dawn".
fn file_title(page_title: &str) -> String {
    let name = page_title.strip_prefix("File:").unwrap_or(page_title);
    let name = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    name.trim().to_owned()
}

/// The author and license Commons lists for the file.
fn file_credit(info: &ImageInfo) -> Credit {
    let metadata = &info.extmetadata;
    let artist = metadata.artist.as_ref().and_then(MetadataValue::text).unwrap_or_default();
    let license_url =
        metadata.license_url.as_ref().and_then(MetadataValue::text).map(|url| match url.strip_prefix("http://") {
            Some(rest) => format!("https://{rest}"),
            None => url.to_owned(),
        });
    Credit {
        author: credit_text(&plain_text(artist)),
        author_url: first_link(artist).and_then(|url| https_link(&url, &["commons.wikimedia.org"])),
        source_url: info.descriptionurl.as_deref().and_then(|url| https_link(url, &["commons.wikimedia.org"])),
        license: metadata.license_short_name.as_ref().and_then(MetadataValue::text).and_then(credit_text),
        license_url: license_url.and_then(|url| https_link(&url, METADATA.link_hosts)),
    }
}

/// The text of a snippet of HTML: tags dropped, entities decoded and
/// whitespace collapsed.
fn plain_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for character in html.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    decode_entities(&text).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first link in a snippet of HTML, with protocol-relative links
/// (`//commons.wikimedia.org/…`) made absolute.
fn first_link(html: &str) -> Option<String> {
    let start = html.find("href=\"")? + "href=\"".len();
    let end = start + html[start..].find('"')?;
    let href = decode_entities(&html[start..end]);
    Some(match href.strip_prefix("//") {
        Some(rest) => format!("https://{rest}"),
        None => href,
    })
}

fn decode_entities(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        decoded.push_str(&rest[..start]);
        rest = &rest[start..];
        let entity = rest.find(';').filter(|&end| end <= 10).and_then(|end| Some((decode_entity(&rest[1..end])?, end)));
        match entity {
            Some((character, end)) => {
                decoded.push(character);
                rest = &rest[end + 1..];
            }
            None => {
                decoded.push('&');
                rest = &rest[1..];
            }
        }
    }
    decoded.push_str(rest);
    decoded
}

fn decode_entity(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

fn suits_desktop(info: &ImageInfo) -> bool {
    let (width, height) = (u64::from(info.width), u64::from(info.height));
    SUPPORTED_MIME_TYPES.contains(&info.mime.as_str())
        && info.width >= MIN_WIDTH
        && width * 10 >= height * MIN_RATIO_TENTHS
        && width * 10 <= height * MAX_RATIO_TENTHS
}

/// Rewrites a Wikimedia thumbnail URL (`…/960px-Name.jpg`) to another
/// width. `None` if the URL does not have that shape.
fn thumbnail_with_width(thumbnail_url: &str, width: u32) -> Option<String> {
    let (path, query) = match thumbnail_url.split_once('?') {
        Some((path, query)) => (path, Some(query)),
        None => (thumbnail_url, None),
    };
    let (directory, file) = path.rsplit_once('/')?;
    let (digits, name) = file.split_once("px-")?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let mut url = format!("{directory}/{width}px-{name}");
    if let Some(query) = query {
        url.push('?');
        url.push_str(query);
    }
    Some(url)
}

fn to_wallpaper(page: FilePage) -> Option<ProviderWallpaper> {
    let info = page.imageinfo.into_iter().next()?;
    if !suits_desktop(&info) {
        return None;
    }
    let credit = file_credit(&info);
    let thumbnail_url = info.thumburl.clone().unwrap_or_else(|| info.url.clone());
    let image_url = if info.width > MAX_WIDTH {
        info.thumburl.as_deref().and_then(|url| thumbnail_with_width(url, MAX_WIDTH)).unwrap_or(info.url)
    } else {
        info.url
    };
    let width = info.width.min(MAX_WIDTH);
    let height = if info.width > MAX_WIDTH {
        (u64::from(info.height) * u64::from(MAX_WIDTH) / u64::from(info.width)) as u32
    } else {
        info.height
    };
    Some(ProviderWallpaper {
        provider_id: page.pageid.to_string(),
        title: file_title(&page.title),
        image_url,
        thumbnail_url,
        width,
        height,
        credit,
    })
}

/// Credits from a `pageids` query, by page id.
fn parse_credits(body: ApiResponse) -> AppResult<HashMap<String, Credit>> {
    if let Some(error) = body.error {
        return Err(AppError::Provider(format!("Wikimedia Commons could not look up credits: {}", error.info)));
    }
    let pages = body.query.map(|query| query.pages).unwrap_or_default();
    Ok(pages
        .into_iter()
        .filter_map(|page| Some((page.pageid.to_string(), file_credit(page.imageinfo.first()?))))
        .collect())
}

fn parse_page(body: ApiResponse) -> AppResult<ProviderPage> {
    if let Some(error) = body.error {
        return Err(AppError::Provider(format!("Wikimedia Commons could not search: {}", error.info)));
    }
    let mut pages = body.query.map(|query| query.pages).unwrap_or_default();
    pages.sort_by_key(|page| page.index);
    let wallpapers = pages.into_iter().filter_map(to_wallpaper).collect();
    Ok(ProviderPage { wallpapers, has_more: body.continuation.is_some() })
}

#[async_trait]
impl WallpaperProvider for WikimediaProvider {
    fn metadata(&self) -> &ProviderMetadata {
        &METADATA
    }

    fn image_hosts(&self) -> &[&'static str] {
        IMAGE_HOSTS
    }

    async fn search(&self, request: &SearchRequest<'_>) -> AppResult<ProviderPage> {
        let search = search_text(request.keyword);
        let limit = PAGE_SIZE.to_string();
        let offset = (request.page.saturating_sub(1) * PAGE_SIZE).to_string();
        let thumbnail_width = THUMBNAIL_WIDTH.to_string();
        let query = [
            ("action", "query"),
            ("format", "json"),
            ("formatversion", "2"),
            ("generator", "search"),
            ("gsrsearch", search.as_str()),
            ("gsrnamespace", "6"),
            ("gsrlimit", limit.as_str()),
            ("gsroffset", offset.as_str()),
            ("prop", "imageinfo"),
            ("iiprop", "url|size|mime|extmetadata"),
            ("iiextmetadatafilter", "Artist|LicenseShortName|LicenseUrl"),
            ("iiurlwidth", thumbnail_width.as_str()),
        ];

        let response = self.client.get(API_URL).timeout(REQUEST_TIMEOUT).query(&query).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_error(METADATA.name, status));
        }
        parse_page(response.json().await?)
    }

    /// Commons' licenses require crediting the author, so files saved
    /// before credits were recorded get them looked up, 50 at a time.
    async fn credits(&self, provider_ids: &[String]) -> AppResult<HashMap<String, Credit>> {
        let mut credits = HashMap::new();
        for batch in provider_ids.chunks(CREDIT_BATCH) {
            let page_ids = batch.join("|");
            let query = [
                ("action", "query"),
                ("format", "json"),
                ("formatversion", "2"),
                ("pageids", page_ids.as_str()),
                ("prop", "imageinfo"),
                ("iiprop", "url|size|extmetadata"),
                ("iiextmetadatafilter", "Artist|LicenseShortName|LicenseUrl"),
            ];
            let response = self.client.get(API_URL).timeout(REQUEST_TIMEOUT).query(&query).send().await?;
            let status = response.status();
            if !status.is_success() {
                return Err(status_error(METADATA.name, status));
            }
            credits.extend(parse_credits(response.json().await?)?);
        }
        Ok(credits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{
      "batchcomplete": true,
      "continue": { "gsroffset": 24, "continue": "gsroffset||" },
      "query": {
        "pages": [
          {
            "pageid": 113030211,
            "ns": 6,
            "title": "File:Mountains of Wadi Shawka denoised.jpg",
            "index": 2,
            "imageinfo": [{
              "size": 6567346, "width": 5184, "height": 3456,
              "thumburl": "https://thumb.wikimedia.org/wikipedia/commons/thumb/5/59/Mountains.jpg/960px-Mountains.jpg?utm_source=commons.wikimedia.org",
              "thumbwidth": 960, "thumbheight": 640,
              "url": "https://upload.wikimedia.org/wikipedia/commons/5/59/Mountains.jpg?utm_source=commons.wikimedia.org",
              "mime": "image/jpeg",
              "descriptionurl": "https://commons.wikimedia.org/wiki/File:Mountains_of_Wadi_Shawka_denoised.jpg",
              "extmetadata": {
                "Artist": { "value": "<a href=\"//commons.wikimedia.org/wiki/User:Myrabella\" title=\"User:Myrabella\">Myrabella</a> &amp; friends", "source": "commons-desc-page" },
                "LicenseShortName": { "value": "CC BY-SA 3.0", "source": "commons-desc-page" },
                "LicenseUrl": { "value": "http://creativecommons.org/licenses/by-sa/3.0", "source": "commons-desc-page" }
              }
            }]
          },
          {
            "pageid": 99193060,
            "ns": 6,
            "title": "File:Sofia Massif, Caucasus Mountains.jpg",
            "index": 1,
            "imageinfo": [{
              "width": 2560, "height": 1440,
              "thumburl": "https://thumb.wikimedia.org/wikipedia/commons/thumb/1/16/Sofia.jpg/960px-Sofia.jpg",
              "url": "https://upload.wikimedia.org/wikipedia/commons/1/16/Sofia.jpg",
              "mime": "image/jpeg"
            }]
          },
          {
            "pageid": 3, "ns": 6, "title": "File:Tall tower.jpg", "index": 3,
            "imageinfo": [{ "width": 3000, "height": 4500, "url": "https://upload.wikimedia.org/a.jpg", "mime": "image/jpeg" }]
          },
          {
            "pageid": 4, "ns": 6, "title": "File:Old scan.tif", "index": 4,
            "imageinfo": [{ "width": 6000, "height": 4000, "url": "https://upload.wikimedia.org/b.tif", "mime": "image/tiff" }]
          },
          {
            "pageid": 5, "ns": 6, "title": "File:Panorama.jpg", "index": 5,
            "imageinfo": [{ "width": 20000, "height": 4000, "url": "https://upload.wikimedia.org/c.jpg", "mime": "image/jpeg" }]
          }
        ]
      }
    }"##;

    #[test]
    fn parses_results_in_search_order_and_keeps_desktop_images() {
        let page = parse_page(serde_json::from_str(SAMPLE).unwrap()).unwrap();
        assert!(page.has_more);
        let ids: Vec<_> = page.wallpapers.iter().map(|w| w.provider_id.as_str()).collect();
        assert_eq!(ids, ["99193060", "113030211"], "portraits, TIFFs and panoramas are skipped");

        let small = &page.wallpapers[0];
        assert_eq!(small.title, "Sofia Massif, Caucasus Mountains");
        assert_eq!(small.image_url, "https://upload.wikimedia.org/wikipedia/commons/1/16/Sofia.jpg");
        assert_eq!((small.width, small.height), (2560, 1440));
    }

    #[test]
    fn large_originals_are_downloaded_scaled_down() {
        let page = parse_page(serde_json::from_str(SAMPLE).unwrap()).unwrap();
        let large = &page.wallpapers[1];
        assert_eq!(
            large.image_url,
            "https://thumb.wikimedia.org/wikipedia/commons/thumb/5/59/Mountains.jpg/3840px-Mountains.jpg?utm_source=commons.wikimedia.org"
        );
        assert!(large.thumbnail_url.contains("/960px-Mountains.jpg"));
        assert_eq!((large.width, large.height), (3840, 2560));
    }

    #[test]
    fn thumbnail_width_rewrite_needs_a_thumbnail_url() {
        assert_eq!(thumbnail_with_width("https://h/a/120px-B.jpg", 3840).as_deref(), Some("https://h/a/3840px-B.jpg"));
        assert_eq!(thumbnail_with_width("https://h/a/B.jpg", 3840), None);
        assert_eq!(thumbnail_with_width("https://h/a/xpx-B.jpg", 3840), None);
    }

    #[test]
    fn no_results_and_api_errors() {
        let empty = parse_page(serde_json::from_str(r#"{"batchcomplete": true}"#).unwrap()).unwrap();
        assert!(empty.wallpapers.is_empty());
        assert!(!empty.has_more);

        let failed = parse_page(serde_json::from_str(r#"{"error": {"code": "x", "info": "Bad search"}}"#).unwrap());
        assert!(failed.unwrap_err().to_string().contains("Bad search"));
    }

    #[test]
    fn searches_only_safe_topics() {
        let search = search_text("sunrise");
        assert!(
            search.starts_with("sunrise incategory:Featured_pictures_of_landscapes|Featured_pictures_of_mountains|")
        );
        assert!(search.ends_with("|Featured_pictures_of_flowers filetype:bitmap filew:>1919"));
        assert_eq!(search.matches("incategory:").count(), 1, "one filter, with the topics as alternatives");
    }

    #[test]
    fn credits_the_author_and_license() {
        let page = parse_page(serde_json::from_str(SAMPLE).unwrap()).unwrap();
        let credited = page.wallpapers.iter().find(|w| w.provider_id == "113030211").unwrap();
        assert_eq!(credited.credit.author.as_deref(), Some("Myrabella & friends"));
        assert_eq!(credited.credit.author_url.as_deref(), Some("https://commons.wikimedia.org/wiki/User:Myrabella"));
        assert_eq!(
            credited.credit.source_url.as_deref(),
            Some("https://commons.wikimedia.org/wiki/File:Mountains_of_Wadi_Shawka_denoised.jpg")
        );
        assert_eq!(credited.credit.license.as_deref(), Some("CC BY-SA 3.0"));
        assert_eq!(credited.credit.license_url.as_deref(), Some("https://creativecommons.org/licenses/by-sa/3.0"));

        let uncredited = page.wallpapers.iter().find(|w| w.provider_id != "113030211").unwrap();
        assert_eq!(uncredited.credit, Credit::default());
    }

    #[test]
    fn credit_lookups_are_keyed_by_page_id() {
        let credits = parse_credits(serde_json::from_str(SAMPLE).unwrap()).unwrap();
        assert_eq!(credits["113030211"].author.as_deref(), Some("Myrabella & friends"));
        assert_eq!(credits["113030211"].license.as_deref(), Some("CC BY-SA 3.0"));
        let failed = parse_credits(serde_json::from_str(r#"{"error": {"code": "x", "info": "Bad ids"}}"#).unwrap());
        assert!(failed.unwrap_err().to_string().contains("Bad ids"));
    }

    #[test]
    fn artist_html_becomes_plain_text() {
        assert_eq!(plain_text("<span>Jos&#233; &lt;Pepe&gt;\n  Garc&#xED;a</span>"), "José <Pepe> García");
        assert_eq!(plain_text("A &unknown; B & C"), "A &unknown; B & C");
        assert_eq!(
            first_link("<a href=\"https://www.flickr.com/people/x\">x</a>").as_deref(),
            Some("https://www.flickr.com/people/x")
        );
        assert_eq!(first_link("Unknown author"), None);
    }

    #[test]
    fn titles_drop_the_namespace_and_extension() {
        assert_eq!(file_title("File:Mount Fuji at dawn.jpg"), "Mount Fuji at dawn");
        assert_eq!(file_title("File:No extension"), "No extension");
    }
}
