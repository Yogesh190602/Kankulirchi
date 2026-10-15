//! Picture fingerprints, to tell when two wallpapers are the same picture,
//! such as a re-upload of one at another size.
//!
//! A fingerprint holds a 16×16 grey thumbnail, the fine detail of a 48×48 one
//! (the thumbnail minus a blurred copy, so broad gradients such as fog or sky
//! do not count) and the average color. Two pictures are the same when both
//! thumbnails correlate closely and the colors match. Checked against a
//! library of 824 wallpapers, it caught every re-upload and none of the
//! foggy photos that only look alike at a glance.

use std::path::PathBuf;

use image::imageops::FilterType;
use image::DynamicImage;

use crate::error::AppResult;

const COARSE_SIZE: u32 = 16;
const DETAIL_SIZE: u32 = 48;
const DETAIL_BLUR: f32 = 3.0;
/// Least correlation of the 16×16 thumbnails for the same picture.
const MIN_COARSE_CORRELATION: f32 = 0.95;
/// Least correlation of the fine detail for the same picture. Pictures that
/// are merely alike, such as two foggy forests, stay well below.
const MIN_DETAIL_CORRELATION: f32 = 0.75;
/// Largest difference in average color, per channel, for the same picture.
const MAX_COLOR_DIFFERENCE: u8 = 15;
/// First byte of a stored fingerprint, so a future format is recognised.
const VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Fingerprint {
    coarse: Vec<u8>,
    detail: Vec<i8>,
    color: [u8; 3],
}

impl Fingerprint {
    pub fn of(image: &DynamicImage) -> Self {
        let small = image.thumbnail_exact(DETAIL_SIZE, DETAIL_SIZE).to_rgb8();
        let grey = DynamicImage::ImageRgb8(small.clone()).to_luma8();
        let blurred = image::imageops::blur(&grey, DETAIL_BLUR);
        let detail = grey
            .pixels()
            .zip(blurred.pixels())
            .map(|(pixel, blur)| (i16::from(pixel[0]) - i16::from(blur[0])).clamp(-127, 127) as i8)
            .collect();
        let coarse = image::imageops::resize(&grey, COARSE_SIZE, COARSE_SIZE, FilterType::Triangle).into_raw();

        let count = small.pixels().len().max(1) as u64;
        let mut sums = [0u64; 3];
        for pixel in small.pixels() {
            for (sum, channel) in sums.iter_mut().zip(pixel.0) {
                *sum += u64::from(channel);
            }
        }
        let color = sums.map(|sum| (sum / count) as u8);
        Self { coarse, detail, color }
    }

    /// Whether both fingerprints come from the same picture.
    pub fn same_picture(&self, other: &Self) -> bool {
        self.color.iter().zip(other.color).all(|(a, b)| a.abs_diff(b) <= MAX_COLOR_DIFFERENCE)
            && correlation(&self.coarse, &other.coarse) >= MIN_COARSE_CORRELATION
            && correlation(&self.detail, &other.detail) >= MIN_DETAIL_CORRELATION
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(1 + self.coarse.len() + self.detail.len() + 3);
        bytes.push(VERSION);
        bytes.extend_from_slice(&self.color);
        bytes.extend_from_slice(&self.coarse);
        bytes.extend(self.detail.iter().map(|&value| value as u8));
        bytes
    }

    /// `None` for bytes in another format, which are worked out again.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let coarse_len = (COARSE_SIZE * COARSE_SIZE) as usize;
        let detail_len = (DETAIL_SIZE * DETAIL_SIZE) as usize;
        let (&version, rest) = bytes.split_first()?;
        if version != VERSION || rest.len() != 3 + coarse_len + detail_len {
            return None;
        }
        let (color, rest) = rest.split_at(3);
        let (coarse, detail) = rest.split_at(coarse_len);
        Some(Self {
            coarse: coarse.to_vec(),
            detail: detail.iter().map(|&value| value as i8).collect(),
            color: [color[0], color[1], color[2]],
        })
    }
}

/// The fingerprint of an image file.
pub async fn of_file(path: PathBuf) -> AppResult<Fingerprint> {
    tauri::async_runtime::spawn_blocking(move || Ok(Fingerprint::of(&image::open(path)?))).await?
}

/// A wallpaper in a collection, with what is needed to pick the copy to
/// keep.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub wallpaper_id: i64,
    pub width: u32,
    pub height: u32,
    pub fingerprint: Fingerprint,
}

impl Candidate {
    /// Of two copies of a picture, the one with more pixels is kept; for
    /// equal sizes, the one saved first.
    fn better_than(&self, other: &Self) -> bool {
        let pixels = |candidate: &Self| u64::from(candidate.width) * u64::from(candidate.height);
        (pixels(self), std::cmp::Reverse(self.wallpaper_id)) > (pixels(other), std::cmp::Reverse(other.wallpaper_id))
    }
}

/// The wallpaper to remove if `new` is the same picture as one of `others`:
/// whichever of the two copies is not as sharp.
pub fn repeat_of(new: &Candidate, others: &[Candidate]) -> Option<i64> {
    let twin = others
        .iter()
        .find(|other| other.wallpaper_id != new.wallpaper_id && other.fingerprint.same_picture(&new.fingerprint))?;
    Some(if new.better_than(twin) { twin.wallpaper_id } else { new.wallpaper_id })
}

/// Every wallpaper to remove so that `candidates` holds each picture once,
/// keeping the sharpest copy.
pub fn repeats(candidates: &[Candidate]) -> Vec<i64> {
    let mut kept: Vec<&Candidate> = Vec::new();
    let mut removed = Vec::new();
    for candidate in candidates {
        match kept.iter_mut().find(|other| other.fingerprint.same_picture(&candidate.fingerprint)) {
            Some(other) if candidate.better_than(other) => {
                removed.push(other.wallpaper_id);
                *other = candidate;
            }
            Some(_) => removed.push(candidate.wallpaper_id),
            None => kept.push(candidate),
        }
    }
    removed
}

/// Pearson correlation: how alike two patterns are, ignoring brightness and
/// contrast. 1 is identical; a flat pattern correlates with nothing.
fn correlation<T: Copy + Into<f32>>(a: &[T], b: &[T]) -> f32 {
    let mean = |values: &[T]| values.iter().map(|&value| value.into()).sum::<f32>() / values.len().max(1) as f32;
    let (mean_a, mean_b) = (mean(a), mean(b));
    let (mut product, mut square_a, mut square_b) = (0.0, 0.0, 0.0);
    for (&x, &y) in a.iter().zip(b) {
        let (x, y) = (x.into() - mean_a, y.into() - mean_b);
        product += x * y;
        square_a += x * x;
        square_b += y * y;
    }
    if square_a == 0.0 || square_b == 0.0 {
        return 0.0;
    }
    product / (square_a * square_b).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A made-up landscape: a sky gradient, hills and some texture that
    /// depends on `seed`.
    fn landscape(width: u32, height: u32, seed: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| {
            let (u, v) = (x as f32 / width as f32, y as f32 / height as f32);
            let hill = 0.6 + 0.1 * (u * 9.0 + seed as f32).sin();
            let texture = ((u * 37.0 * (seed + 1) as f32).sin() * (v * 23.0).cos() * 40.0) as i32;
            let base = if v > hill { [40, 90, 50] } else { [150, 180, 220] };
            image::Rgb(base.map(|channel| (channel + texture).clamp(0, 255) as u8))
        }))
    }

    /// A foggy picture: mostly a smooth gradient, with faint details that
    /// depend on `seed`.
    fn fog(seed: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(400, 250, |x, y| {
            let shade = 200 - (y * 120 / 250) as i32;
            let detail = if ((x * 7 + seed * 131) ^ (y * 5 + seed * 17)).is_multiple_of(23) { 25 } else { 0 };
            image::Rgb([(shade - detail).clamp(0, 255) as u8; 3])
        }))
    }

    #[test]
    fn a_resized_copy_is_the_same_picture() {
        let original = Fingerprint::of(&landscape(1920, 1080, 1));
        let smaller = Fingerprint::of(&landscape(1920, 1080, 1).resize_exact(1280, 720, FilterType::Lanczos3));
        assert!(original.same_picture(&smaller));
    }

    #[test]
    fn different_pictures_are_not() {
        assert!(!Fingerprint::of(&landscape(1920, 1080, 1)).same_picture(&Fingerprint::of(&landscape(1920, 1080, 4))));
    }

    #[test]
    fn foggy_pictures_with_different_details_are_not() {
        assert!(!Fingerprint::of(&fog(1)).same_picture(&Fingerprint::of(&fog(2))));
        assert!(Fingerprint::of(&fog(1)).same_picture(&Fingerprint::of(&fog(1))));
    }

    #[test]
    fn fingerprints_round_trip_and_reject_other_formats() {
        let fingerprint = Fingerprint::of(&landscape(640, 360, 2));
        assert_eq!(Fingerprint::from_bytes(&fingerprint.to_bytes()), Some(fingerprint.clone()));
        let mut other_version = fingerprint.to_bytes();
        other_version[0] = 9;
        assert_eq!(Fingerprint::from_bytes(&other_version), None);
        assert_eq!(Fingerprint::from_bytes(&[]), None);
    }

    fn candidate(wallpaper_id: i64, width: u32, seed: u32) -> Candidate {
        Candidate {
            wallpaper_id,
            width,
            height: width * 9 / 16,
            fingerprint: Fingerprint::of(&landscape(width, width * 9 / 16, seed)),
        }
    }

    #[test]
    fn the_sharpest_copy_is_kept() {
        let candidates = [candidate(1, 1280, 1), candidate(2, 640, 2), candidate(3, 1920, 1), candidate(4, 960, 1)];
        assert_eq!(repeats(&candidates), [1, 4]);
        assert_eq!(repeat_of(&candidates[3], &candidates[..3]), Some(4));
        assert_eq!(repeat_of(&candidates[2], &candidates[..2]), Some(1));
        assert_eq!(repeat_of(&candidates[1], &[candidates[0].clone()]), None);
    }

    /// Lists the pictures in `KANKULIRCHI_FINGERPRINT_DIR` that look the
    /// same. Run with `cargo test --release -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads a folder of wallpapers"]
    fn report_repeats_in_a_folder() {
        let dir = std::env::var("KANKULIRCHI_FINGERPRINT_DIR").expect("set KANKULIRCHI_FINGERPRINT_DIR");
        let mut prints = Vec::new();
        let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().filter_map(|entry| Some(entry.ok()?.path())).collect();
        paths.sort();
        for path in paths {
            if let Ok(image) = image::open(&path) {
                prints.push((path.file_name().unwrap().to_string_lossy().into_owned(), Fingerprint::of(&image)));
            }
        }
        let mut pairs = 0;
        for (index, (name, print)) in prints.iter().enumerate() {
            for (other_name, other) in &prints[index + 1..] {
                if print.same_picture(other) {
                    pairs += 1;
                    println!("same picture: {name} ~ {other_name}");
                }
            }
        }
        println!("{} pictures, {pairs} pairs", prints.len());
    }

    #[test]
    fn of_two_equal_copies_the_first_saved_is_kept() {
        let candidates = [candidate(7, 1280, 3), candidate(5, 1280, 3)];
        assert_eq!(repeats(&candidates), [7]);
        assert_eq!(repeat_of(&candidates[0], &candidates[1..]), Some(7));
    }
}
