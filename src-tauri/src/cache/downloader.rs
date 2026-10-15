//! Local wallpaper cache.
//!
//! Each wallpaper is stored once, as `<provider>-<provider id>.<ext>` in the
//! cache directory, and its path is recorded in SQLite. Concurrent requests
//! for the same wallpaper wait for a single download instead of starting
//! their own.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use image::{ImageFormat, ImageReader};
use tokio::io::AsyncWriteExt;

use crate::database::Database;
use crate::error::{AppError, AppResult};
use crate::models::{CacheStats, FreedSpace, Wallpaper};

/// Downloads larger than this are refused.
const MAX_IMAGE_BYTES: u64 = 200 * 1024 * 1024;

/// Thumbnails larger than this are refused.
const MAX_THUMBNAIL_BYTES: usize = 10 * 1024 * 1024;

/// Extensions a cached file can have, checked when reusing files found on
/// disk.
const CACHED_EXTENSIONS: [&str; 4] = ["jpg", "png", "webp", "jpeg"];

const PARTIAL_EXTENSION: &str = "part";

/// Background downloads that run at once. Saving many wallpapers at a time
/// queues the rest instead of flooding the network and the provider.
pub const BACKGROUND_DOWNLOADS: usize = 3;

type KeyLock = Arc<tokio::sync::Mutex<()>>;

pub struct Downloader {
    client: reqwest::Client,
    in_flight: Mutex<HashMap<String, KeyLock>>,
    background: tokio::sync::Semaphore,
}

impl Downloader {
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            in_flight: Mutex::new(HashMap::new()),
            background: tokio::sync::Semaphore::new(BACKGROUND_DOWNLOADS),
        }
    }

    /// Returns the stored wallpaper with a usable local file, downloading
    /// the image first if it is not cached yet.
    pub async fn ensure_cached(&self, db: &Database, cache_dir: &Path, wallpaper_id: i64) -> AppResult<Wallpaper> {
        let wallpaper = db.get_wallpaper(wallpaper_id)?;
        if has_local_file(&wallpaper) {
            match readable_cached_file(&wallpaper).await {
                Ok(()) => return Ok(wallpaper),
                Err(error) => forget_damaged_file(db, &wallpaper, &error).await?,
            }
        }

        let key = wallpaper.cache_key();
        let lock = self.lock_for(&key);
        let result = {
            let _guard = lock.lock().await;
            self.download(db, cache_dir, wallpaper_id).await
        };
        self.release(&key, lock);
        result
    }

    /// Downloads a thumbnail into memory, for working out a palette without
    /// downloading the full image. The caller checks the URL's host.
    pub async fn fetch_thumbnail(&self, url: &str) -> AppResult<Vec<u8>> {
        let response = self.client.get(url).send().await?.error_for_status()?;
        let address = response.url().clone();
        let mut body = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk) = body.next().await {
            bytes.extend_from_slice(&chunk.map_err(|error| error.with_url(address.clone()))?);
            if bytes.len() > MAX_THUMBNAIL_BYTES {
                return Err(AppError::Provider("The provider sent a thumbnail that is too large.".into()));
            }
        }
        Ok(bytes)
    }

    /// [`Self::ensure_cached`] for downloads nobody is waiting for. At most
    /// [`BACKGROUND_DOWNLOADS`] run at once; a wallpaper that is needed
    /// sooner (to apply it) is downloaded directly, not queued behind these.
    pub async fn prefetch(&self, db: &Database, cache_dir: &Path, wallpaper_id: i64) -> AppResult<Wallpaper> {
        let _permit = self
            .background
            .acquire()
            .await
            .map_err(|_| AppError::Internal("Background downloads have stopped.".into()))?;
        self.ensure_cached(db, cache_dir, wallpaper_id).await
    }

    fn lock_for(&self, key: &str) -> KeyLock {
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        in_flight.entry(key.to_owned()).or_default().clone()
    }

    fn release(&self, key: &str, lock: KeyLock) {
        let mut in_flight = self.in_flight.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // One reference is held by the map and one by `lock`; any more means
        // another task is waiting for this wallpaper.
        if Arc::strong_count(&lock) <= 2 {
            in_flight.remove(key);
        }
    }

    async fn download(&self, db: &Database, cache_dir: &Path, wallpaper_id: i64) -> AppResult<Wallpaper> {
        // Another task may have finished this download while we waited.
        let wallpaper = db.get_wallpaper(wallpaper_id)?;
        if has_local_file(&wallpaper) {
            return Ok(wallpaper);
        }

        tokio::fs::create_dir_all(cache_dir).await?;
        let stem = file_stem(&wallpaper);

        // Reuse a file from an earlier run, for example after the database
        // was reset while the cache directory was kept.
        if let Some((path, width, height)) = find_cached_file(cache_dir, &stem).await {
            db.set_local_file(wallpaper_id, Some(&path_to_string(&path)?), width, height)?;
            return db.get_wallpaper(wallpaper_id);
        }

        let partial = cache_dir.join(format!("{stem}.{PARTIAL_EXTENSION}"));
        let inspected = match self.fetch(&wallpaper.image_url, &partial).await {
            Ok(()) => verify_image(partial.clone()).await,
            Err(error) => Err(error),
        };
        let (format, width, height) = match inspected {
            Ok(details) => details,
            Err(error) => {
                let _ = tokio::fs::remove_file(&partial).await;
                return Err(error);
            }
        };

        let destination = cache_dir.join(format!("{stem}.{}", extension_for(format)));
        tokio::fs::rename(&partial, &destination).await?;
        db.set_local_file(wallpaper_id, Some(&path_to_string(&destination)?), width, height)?;
        log::info!("cached {} at {}", wallpaper.cache_key(), destination.display());
        db.get_wallpaper(wallpaper_id)
    }

    async fn fetch(&self, url: &str, destination: &Path) -> AppResult<()> {
        let response = self.client.get(url).send().await?.error_for_status()?;
        if response.content_length().is_some_and(|length| length > MAX_IMAGE_BYTES) {
            return Err(too_large());
        }

        let address = response.url().clone();
        let mut file = tokio::fs::File::create(destination).await?;
        let mut body = response.bytes_stream();
        let mut written: u64 = 0;
        while let Some(chunk) = body.next().await {
            // Errors reading the body do not say where it came from.
            let chunk = chunk.map_err(|error| error.with_url(address.clone()))?;
            written += chunk.len() as u64;
            if written > MAX_IMAGE_BYTES {
                return Err(too_large());
            }
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        file.sync_all().await?;

        if written == 0 {
            return Err(AppError::Provider("The provider sent an empty image.".into()));
        }
        Ok(())
    }
}

fn too_large() -> AppError {
    AppError::Provider(format!("The image is larger than {} MB and was not downloaded.", MAX_IMAGE_BYTES / 1024 / 1024))
}

pub fn has_local_file(wallpaper: &Wallpaper) -> bool {
    wallpaper
        .local_path
        .as_deref()
        .and_then(|path| std::fs::metadata(path).ok())
        .is_some_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

fn file_stem(wallpaper: &Wallpaper) -> String {
    format!("{}-{}", sanitize(&wallpaper.provider), sanitize(&wallpaper.provider_id))
}

fn sanitize(value: &str) -> String {
    value.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

fn path_to_string(path: &Path) -> AppResult<String> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| AppError::Validation(format!("{} is not a valid UTF-8 path.", path.display())))
}

fn extension_for(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Png => "png",
        ImageFormat::WebP => "webp",
        _ => "jpg",
    }
}

async fn find_cached_file(cache_dir: &Path, stem: &str) -> Option<(PathBuf, u32, u32)> {
    for extension in CACHED_EXTENSIONS {
        let candidate = cache_dir.join(format!("{stem}.{extension}"));
        if tokio::fs::metadata(&candidate).await.is_ok_and(|m| m.is_file() && m.len() > 0) {
            if let Ok((_, width, height)) = verify_image(candidate.clone()).await {
                return Some((candidate, width, height));
            }
        }
    }
    None
}

fn supported_format(format: Option<ImageFormat>) -> AppResult<ImageFormat> {
    match format {
        Some(format @ (ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP)) => Ok(format),
        _ => Err(AppError::Provider("The provider sent a file that is not a JPEG, PNG or WebP image.".into())),
    }
}

/// Quick check, made before a downloaded file is applied, that it still
/// starts like a JPEG, PNG or WebP image.
async fn readable_cached_file(wallpaper: &Wallpaper) -> AppResult<()> {
    let path = PathBuf::from(wallpaper.local_path.as_deref().unwrap_or_default());
    tauri::async_runtime::spawn_blocking(move || -> AppResult<()> {
        let reader = ImageReader::open(&path)?.with_guessed_format()?;
        supported_format(reader.format())?;
        reader.into_dimensions()?;
        Ok(())
    })
    .await?
}

/// Deletes a downloaded file that is no longer a readable image and clears
/// its path, so the wallpaper is downloaded again.
async fn forget_damaged_file(db: &Database, wallpaper: &Wallpaper, error: &AppError) -> AppResult<()> {
    let path = wallpaper.local_path.as_deref().unwrap_or_default();
    log::warn!("the downloaded file of {} is damaged ({error}); downloading it again", wallpaper.cache_key());
    if let Err(error) = tokio::fs::remove_file(path).await {
        log::warn!("could not delete {path}: {error}");
    }
    db.set_local_file(wallpaper.id.unwrap_or_default(), None, 0, 0)
}

/// Confirms the whole file is a JPEG, PNG or WebP image, so a download cut
/// short is caught, and returns its format and dimensions.
async fn verify_image(path: PathBuf) -> AppResult<(ImageFormat, u32, u32)> {
    tauri::async_runtime::spawn_blocking(move || -> AppResult<(ImageFormat, u32, u32)> {
        let bytes = std::fs::read(&path)?;
        let reader = ImageReader::new(io::Cursor::new(&bytes)).with_guessed_format()?;
        let format = supported_format(reader.format())?;
        let (width, height) = if format == ImageFormat::Jpeg {
            // The JPEG decoder draws a truncated image without complaint,
            // so completeness is checked separately.
            if !jpeg_is_complete(&bytes) {
                return Err(AppError::Provider("The provider sent an incomplete image.".into()));
            }
            reader.into_dimensions()?
        } else {
            // PNG and WebP decoding fails on missing data.
            let image = reader.decode()?;
            (image.width(), image.height())
        };
        Ok((format, width, height))
    })
    .await?
}

/// Whether a JPEG reaches its end-of-image marker. Walks the file's segments,
/// skipping each by its length, so embedded thumbnails and metadata are not
/// mistaken for the end, and scans entropy-coded data for the next marker
/// (where 0xFF is followed by 0x00 or a restart marker otherwise).
fn jpeg_is_complete(bytes: &[u8]) -> bool {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return false;
    }
    let mut i = 2;
    while i + 1 < bytes.len() {
        if bytes[i] != 0xFF {
            return false;
        }
        let marker = bytes[i + 1];
        match marker {
            // Fill byte before a marker.
            0xFF => {
                i += 1;
                continue;
            }
            // End of image.
            0xD9 => return true,
            // Markers without a length.
            0x01 | 0xD0..=0xD7 => {
                i += 2;
                continue;
            }
            _ => {}
        }
        let Some(length) = bytes.get(i + 2..i + 4).map(|pair| usize::from(u16::from_be_bytes([pair[0], pair[1]])))
        else {
            return false;
        };
        if length < 2 {
            return false;
        }
        i += 2 + length;
        // Start of scan: entropy-coded data follows until the next marker.
        if marker == 0xDA {
            loop {
                let Some(offset) = bytes.get(i..).and_then(|rest| rest.iter().position(|&byte| byte == 0xFF)) else {
                    return false;
                };
                let at = i + offset;
                match bytes.get(at + 1) {
                    None => return false,
                    Some(0x00 | 0xD0..=0xD7) => i = at + 2,
                    Some(0xFF) => i = at + 1,
                    Some(_) => {
                        i = at;
                        break;
                    }
                }
            }
        }
    }
    false
}

/// Total size and number of files in the cache directory.
pub async fn cache_stats(cache_dir: PathBuf) -> AppResult<CacheStats> {
    tauri::async_runtime::spawn_blocking(move || -> AppResult<CacheStats> {
        let mut stats = CacheStats { path: cache_dir.display().to_string(), total_bytes: 0, file_count: 0 };
        let entries = match std::fs::read_dir(&cache_dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(stats),
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let entry = entry?;
            let metadata = entry.metadata()?;
            let is_partial = entry.path().extension().is_some_and(|ext| ext == PARTIAL_EXTENSION);
            if metadata.is_file() && !is_partial {
                stats.total_bytes += metadata.len();
                stats.file_count += 1;
            }
        }
        Ok(stats)
    })
    .await?
}

/// Deletes the downloaded files of these wallpapers and returns the space
/// freed. Only files named the way the cache names them are deleted, so a
/// wrong path in the database cannot remove anything else.
pub async fn remove_cached_files(wallpapers: Vec<Wallpaper>) -> AppResult<FreedSpace> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut freed = FreedSpace::default();
        for wallpaper in &wallpapers {
            let Some(path) = wallpaper.local_path.as_deref().map(Path::new) else {
                continue;
            };
            if path.file_stem().and_then(|stem| stem.to_str()) != Some(file_stem(wallpaper).as_str()) {
                log::warn!("not deleting {}: it is not named like a cached wallpaper", path.display());
                continue;
            }
            let size = std::fs::metadata(path).map_or(0, |metadata| metadata.len());
            match std::fs::remove_file(path) {
                Ok(()) => {
                    freed.file_count += 1;
                    freed.total_bytes += size;
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => log::warn!("could not delete {}: {error}", path.display()),
            }
        }
        freed
    })
    .await
    .map_err(AppError::from)
}

/// Creates `path` if needed and checks that files can be written to it.
pub fn prepare_cache_dir(path: &Path) -> AppResult<()> {
    if !path.is_absolute() {
        return Err(AppError::Validation("Choose an absolute folder path for the cache.".into()));
    }
    std::fs::create_dir_all(path)?;
    let probe = path.join(".kankulirchi-write-test");
    std::fs::write(&probe, b"")
        .map_err(|error| AppError::Validation(format!("Kankulirchi cannot write to {}: {error}", path.display())))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::WallpaperInput;

    fn store(db: &Database, id: &str) -> Wallpaper {
        db.upsert_wallpaper(&WallpaperInput {
            provider: "wallhaven".into(),
            provider_id: id.into(),
            title: String::new(),
            // Port 9 (discard) on localhost: any accidental request fails fast.
            image_url: format!("http://127.0.0.1:9/{id}.png"),
            thumbnail_url: format!("http://127.0.0.1:9/{id}_t.png"),
            width: 0,
            height: 0,
            credit: Default::default(),
        })
        .unwrap()
    }

    fn write_png(path: &Path, width: u32, height: u32) {
        image::RgbImage::new(width, height).save_with_format(path, ImageFormat::Png).unwrap();
    }

    #[test]
    fn file_names_are_safe() {
        let wallpaper = Wallpaper {
            id: None,
            provider: "pexels".into(),
            provider_id: "../12 3".into(),
            title: String::new(),
            image_url: String::new(),
            thumbnail_url: String::new(),
            local_path: None,
            width: 0,
            height: 0,
            created_at: None,
            is_favorite: false,
            credit: Default::default(),
        };
        assert_eq!(file_stem(&wallpaper), "pexels-___12_3");
    }

    #[tokio::test]
    async fn reuses_files_already_in_the_cache_directory() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let wallpaper = store(&db, "abc");
        write_png(&dir.path().join("wallhaven-abc.png"), 8, 6);

        let downloader = Downloader::new(reqwest::Client::new());
        let cached = downloader.ensure_cached(&db, dir.path(), wallpaper.id.unwrap()).await.unwrap();
        assert_eq!((cached.width, cached.height), (8, 6));
        assert!(cached.local_path.unwrap().ends_with("wallhaven-abc.png"));
    }

    #[tokio::test]
    async fn returns_existing_local_file_without_downloading() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let wallpaper = store(&db, "abc");
        let path = dir.path().join("somewhere.png");
        write_png(&path, 4, 4);
        db.set_local_file(wallpaper.id.unwrap(), Some(path.to_str().unwrap()), 4, 4).unwrap();

        let downloader = Downloader::new(reqwest::Client::new());
        let cached = downloader.ensure_cached(&db, dir.path(), wallpaper.id.unwrap()).await.unwrap();
        assert_eq!(cached.local_path.as_deref(), path.to_str());
    }

    #[tokio::test]
    async fn failed_downloads_leave_no_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let wallpaper = store(&db, "missing");

        let downloader = Downloader::new(reqwest::Client::new());
        assert!(downloader.ensure_cached(&db, dir.path(), wallpaper.id.unwrap()).await.is_err());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert!(db.get_wallpaper(wallpaper.id.unwrap()).unwrap().local_path.is_none());
    }

    #[tokio::test]
    async fn removes_only_files_named_like_cached_wallpapers() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let with_file = |id: &str, file_name: &str| Wallpaper {
            local_path: Some(dir.path().join(file_name).to_str().unwrap().to_owned()),
            ..store(&db, id)
        };
        std::fs::write(dir.path().join("wallhaven-a.jpg"), [0; 10]).unwrap();
        std::fs::write(dir.path().join("notes.txt"), [0; 5]).unwrap();
        let wallpapers = vec![
            with_file("a", "wallhaven-a.jpg"),
            with_file("b", "notes.txt"),
            with_file("c", "wallhaven-c.jpg"),
            store(&db, "d"),
        ];

        let freed = remove_cached_files(wallpapers).await.unwrap();
        assert_eq!(freed, FreedSpace { file_count: 1, total_bytes: 10 });
        assert!(!dir.path().join("wallhaven-a.jpg").exists());
        assert!(dir.path().join("notes.txt").exists(), "a file with another name is left alone");
    }

    fn jpeg_bytes(width: u32, height: u32) -> Vec<u8> {
        let image =
            image::RgbImage::from_fn(width, height, |x, y| image::Rgb([(x * 7) as u8, (y * 13) as u8, (x ^ y) as u8]));
        let mut bytes = Vec::new();
        image.write_to(&mut io::Cursor::new(&mut bytes), ImageFormat::Jpeg).unwrap();
        bytes
    }

    #[test]
    fn jpeg_completeness_catches_cut_off_files() {
        let jpeg = jpeg_bytes(320, 200);
        assert!(jpeg_is_complete(&jpeg));
        for keep in [99, 90, 50, 10] {
            assert!(!jpeg_is_complete(&jpeg[..jpeg.len() * keep / 100]), "{keep}% of the file");
        }
        let mut with_trailer = jpeg.clone();
        with_trailer.extend_from_slice(b"extra data some cameras append");
        assert!(jpeg_is_complete(&with_trailer));

        // A complete thumbnail inside an APP1 segment must not count as the
        // end of a cut-off main image.
        let thumbnail = jpeg_bytes(16, 10);
        let mut with_thumbnail = vec![0xFF, 0xD8, 0xFF, 0xE1];
        with_thumbnail.extend_from_slice(&u16::try_from(thumbnail.len() + 2).unwrap().to_be_bytes());
        with_thumbnail.extend_from_slice(&thumbnail);
        with_thumbnail.extend_from_slice(&jpeg[2..]);
        assert!(jpeg_is_complete(&with_thumbnail));
        assert!(!jpeg_is_complete(&with_thumbnail[..with_thumbnail.len() - jpeg.len() / 2]));
    }

    #[tokio::test]
    async fn verification_rejects_cut_off_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let jpeg = jpeg_bytes(320, 200);
        let complete = dir.path().join("complete.jpg");
        std::fs::write(&complete, &jpeg).unwrap();
        assert_eq!(verify_image(complete).await.unwrap(), (ImageFormat::Jpeg, 320, 200));

        let cut_jpeg = dir.path().join("cut.jpg");
        std::fs::write(&cut_jpeg, &jpeg[..jpeg.len() / 2]).unwrap();
        assert!(verify_image(cut_jpeg).await.unwrap_err().to_string().contains("incomplete"));

        let png = dir.path().join("full.png");
        write_png(&png, 64, 40);
        let bytes = std::fs::read(&png).unwrap();
        let cut_png = dir.path().join("cut.png");
        std::fs::write(&cut_png, &bytes[..bytes.len() * 9 / 10]).unwrap();
        assert!(verify_image(png).await.is_ok());
        assert!(verify_image(cut_png).await.is_err());
    }

    #[tokio::test]
    async fn damaged_downloads_are_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let wallpaper = store(&db, "a");
        let id = wallpaper.id.unwrap();
        let damaged = dir.path().join("wallhaven-a.jpg");
        std::fs::write(&damaged, [0x42; 4096]).unwrap();
        db.set_local_file(id, Some(damaged.to_str().unwrap()), 0, 0).unwrap();
        // A good copy is on disk under another extension, so no download is needed.
        write_png(&dir.path().join("wallhaven-a.png"), 64, 40);

        let downloader = Downloader::new(reqwest::Client::new());
        let repaired = downloader.ensure_cached(&db, dir.path(), id).await.unwrap();
        assert!(!damaged.exists(), "the damaged file is deleted");
        assert!(repaired.local_path.unwrap().ends_with("wallhaven-a.png"));
    }

    #[tokio::test]
    async fn damaged_downloads_are_forgotten_when_they_cannot_be_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open_in_memory().unwrap();
        let id = store(&db, "b").id.unwrap();
        let damaged = dir.path().join("wallhaven-b.jpg");
        std::fs::write(&damaged, b"<html>not an image</html>").unwrap();
        db.set_local_file(id, Some(damaged.to_str().unwrap()), 0, 0).unwrap();

        let downloader = Downloader::new(reqwest::Client::new());
        assert!(downloader.ensure_cached(&db, dir.path(), id).await.is_err(), "the download fails offline");
        assert!(!damaged.exists());
        assert_eq!(db.get_wallpaper(id).unwrap().local_path, None);
    }

    #[tokio::test]
    async fn stats_skip_partial_downloads() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.jpg"), [0u8; 10]).unwrap();
        std::fs::write(dir.path().join("b.part"), [0u8; 99]).unwrap();
        let stats = cache_stats(dir.path().to_path_buf()).await.unwrap();
        assert_eq!(stats.file_count, 1);
        assert_eq!(stats.total_bytes, 10);

        let missing = cache_stats(dir.path().join("nope")).await.unwrap();
        assert_eq!(missing.file_count, 0);
    }

    /// Searches `provider` and downloads its first result twice at once.
    /// The live tests using it run with `cargo test -- --ignored`.
    async fn live_search_and_download(
        provider: &dyn crate::providers::WallpaperProvider,
        client: reqwest::Client,
        api_key: Option<&str>,
    ) -> Wallpaper {
        use crate::providers::SearchRequest;

        let page = provider.search(&SearchRequest { keyword: "lake", page: 1, api_key }).await.unwrap();
        let result = page.wallpapers.into_iter().next().expect("the provider returned no results");
        println!("{} credit: {:?}", provider.id(), result.credit);
        assert!(result.credit.source_url.is_some(), "every result links to its page");

        let db = Database::open_in_memory().unwrap();
        let stored = db
            .upsert_wallpaper(&WallpaperInput {
                provider: provider.id().into(),
                provider_id: result.provider_id,
                title: result.title,
                image_url: result.image_url,
                thumbnail_url: result.thumbnail_url,
                width: result.width,
                height: result.height,
                credit: result.credit,
            })
            .unwrap();

        let dir = tempfile::tempdir().unwrap();
        let downloader = Downloader::new(client);
        let id = stored.id.unwrap();
        let (first, second) =
            tokio::join!(downloader.ensure_cached(&db, dir.path(), id), downloader.ensure_cached(&db, dir.path(), id));
        let first = first.unwrap();
        assert_eq!(first.local_path, second.unwrap().local_path);
        assert!(has_local_file(&first));
        assert!(first.width > 0 && first.height > 0);
        assert!(first.width > first.height, "wallpapers are landscape");
        // One image file and no leftovers from the concurrent request.
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        first
    }

    fn live_client() -> reqwest::Client {
        reqwest::Client::builder().user_agent("Kankulirchi tests").build().unwrap()
    }

    #[tokio::test]
    #[ignore = "downloads from wallhaven.cc"]
    async fn live_wallhaven_search_and_download() {
        let client = live_client();
        live_search_and_download(&crate::providers::wallhaven::WallhavenProvider::new(client.clone()), client, None)
            .await;
    }

    #[tokio::test]
    #[ignore = "downloads from unsplash.com; needs UNSPLASH_ACCESS_KEY"]
    async fn live_unsplash_search_and_download() {
        use crate::providers::WallpaperProvider as _;

        let key = std::env::var("UNSPLASH_ACCESS_KEY").expect("set UNSPLASH_ACCESS_KEY to an Unsplash access key");
        let client = live_client();
        let provider = crate::providers::unsplash::UnsplashProvider::new(client.clone());
        let wallpaper = live_search_and_download(&provider, client, Some(&key)).await;
        provider.report_download(&wallpaper.provider_id, Some(&key)).await.unwrap();
    }

    #[tokio::test]
    #[ignore = "downloads from wikimedia.org"]
    async fn live_wikimedia_search_and_download() {
        let client = live_client();
        live_search_and_download(&crate::providers::wikimedia::WikimediaProvider::new(client.clone()), client, None)
            .await;
    }

    #[test]
    fn cache_dir_must_be_absolute() {
        assert!(prepare_cache_dir(Path::new("relative/dir")).is_err());
        let dir = tempfile::tempdir().unwrap();
        assert!(prepare_cache_dir(&dir.path().join("nested/cache")).is_ok());
    }
}
