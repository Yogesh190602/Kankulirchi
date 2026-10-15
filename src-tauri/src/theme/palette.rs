//! The colors of a wallpaper: its main colors, the GNOME accent color that
//! suits it and whether it suits GNOME's dark style.
//!
//! The picture is shrunk, its pixels are grouped into a few main colors with
//! k-means in the Oklab color space (where distance matches how different
//! colors look), and the most vivid main color picks the nearest of GNOME's
//! accent colors by hue.

use std::path::PathBuf;

use image::imageops::FilterType;
use image::DynamicImage;
use serde::{Deserialize, Serialize};

use crate::error::AppResult;

/// Main colors kept.
const PALETTE_SIZE: usize = 6;
/// Longest side the picture is shrunk to first: plenty for its colors.
const SAMPLE_SIZE: u32 = 96;
const ITERATIONS: usize = 12;
/// Main colors closer than this are merged.
const MERGE_DISTANCE: f32 = 0.04;
/// Smallest share of the picture a main color covers. Less is noise, such as
/// the blend where two colors meet.
const MIN_SHARE: f32 = 0.02;
/// Main colors greyer than this suggest GNOME's neutral accent.
const MIN_ACCENT_CHROMA: f32 = 0.06;
/// Pictures darker than this on average suit the dark style. Mid grey is
/// about 0.6 in Oklab.
const DARK_LIGHTNESS: f32 = 0.6;

/// GNOME's accent colors, as libadwaita draws them.
const GNOME_ACCENTS: &[(&str, [u8; 3])] = &[
    ("blue", [0x35, 0x84, 0xe4]),
    ("teal", [0x21, 0x90, 0xa4]),
    ("green", [0x3a, 0x94, 0x4a]),
    ("yellow", [0xc8, 0x88, 0x00]),
    ("orange", [0xed, 0x5b, 0x00]),
    ("red", [0xe6, 0x2d, 0x42]),
    ("pink", [0xd5, 0x61, 0x99]),
    ("purple", [0x91, 0x41, 0xac]),
];
const NEUTRAL_ACCENT: &str = "slate";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    /// Main colors, most common first.
    pub colors: Vec<PaletteColor>,
    /// The most vivid main color, as `#rrggbb`.
    pub vivid: String,
    /// GNOME's accent color closest to `vivid`, such as "blue".
    pub accent_color: String,
    /// Whether the picture is mostly dark, which suits GNOME's dark style.
    pub dark: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteColor {
    /// `#rrggbb`.
    pub hex: String,
    /// Share of the picture, from 0 to 1.
    pub share: f32,
}

/// The palette of an image file.
pub async fn from_file(path: PathBuf) -> AppResult<Palette> {
    tauri::async_runtime::spawn_blocking(move || Ok(extract(&image::open(path)?))).await?
}

/// The palette of an encoded image, such as a downloaded thumbnail.
pub async fn from_bytes(bytes: Vec<u8>) -> AppResult<Palette> {
    tauri::async_runtime::spawn_blocking(move || Ok(extract(&image::load_from_memory(&bytes)?))).await?
}

pub fn extract(image: &DynamicImage) -> Palette {
    let sample = image.resize(SAMPLE_SIZE, SAMPLE_SIZE, FilterType::Triangle).to_rgb8();
    let pixels: Vec<Lab> = sample.pixels().map(|pixel| Lab::from_srgb(pixel.0)).collect();
    let total = pixels.len().max(1) as f32;
    let dark = pixels.iter().map(|pixel| pixel.l).sum::<f32>() / total < DARK_LIGHTNESS;

    let mut clusters = merge_close(k_means(&pixels, PALETTE_SIZE));
    clusters.retain(|cluster| cluster.count as f32 / total >= MIN_SHARE);
    clusters.sort_by_key(|cluster| std::cmp::Reverse(cluster.count));

    let vivid = clusters
        .iter()
        .max_by(|a, b| a.vividness(total).total_cmp(&b.vividness(total)))
        .map_or(Lab::default(), |cluster| cluster.center);

    Palette {
        colors: clusters
            .iter()
            .map(|cluster| PaletteColor { hex: hex(cluster.center.to_srgb()), share: cluster.count as f32 / total })
            .collect(),
        vivid: hex(vivid.to_srgb()),
        accent_color: nearest_accent(vivid).to_owned(),
        dark,
    }
}

/// The GNOME accent color whose hue is closest to `color`'s, or the neutral
/// one for a greyish color.
fn nearest_accent(color: Lab) -> &'static str {
    if color.chroma() < MIN_ACCENT_CHROMA {
        return NEUTRAL_ACCENT;
    }
    let hue_gap = |accent: &[u8; 3]| {
        let gap = (Lab::from_srgb(*accent).hue() - color.hue()).abs();
        gap.min(std::f32::consts::TAU - gap)
    };
    GNOME_ACCENTS
        .iter()
        .min_by(|(_, a), (_, b)| hue_gap(a).total_cmp(&hue_gap(b)))
        .map_or(NEUTRAL_ACCENT, |(name, _)| name)
}

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}

#[derive(Debug, Clone, Copy)]
struct Cluster {
    center: Lab,
    count: usize,
}

impl Cluster {
    /// Favors colorful colors that cover a fair part of the picture.
    fn vividness(&self, total: f32) -> f32 {
        self.center.chroma() * (self.count as f32 / total).sqrt()
    }
}

/// Groups `pixels` into at most `k` clusters. Starting centers are spread
/// out deterministically (each is the pixel farthest from those already
/// chosen), so the same picture always gives the same palette.
fn k_means(pixels: &[Lab], k: usize) -> Vec<Cluster> {
    let Some(&first) = pixels.first() else {
        return Vec::new();
    };
    let mean = pixels.iter().fold(Lab::default(), |sum, pixel| sum.add(*pixel)).scale(1.0 / pixels.len() as f32);
    let mut centers =
        vec![pixels.iter().copied().min_by(|a, b| a.distance(mean).total_cmp(&b.distance(mean))).unwrap_or(first)];
    while centers.len() < k {
        let farthest = pixels
            .iter()
            .copied()
            .max_by(|a, b| nearest_distance(*a, &centers).total_cmp(&nearest_distance(*b, &centers)))
            .unwrap_or(first);
        if nearest_distance(farthest, &centers) == 0.0 {
            break;
        }
        centers.push(farthest);
    }

    let mut counts = vec![0; centers.len()];
    for _ in 0..ITERATIONS {
        let mut sums = vec![Lab::default(); centers.len()];
        counts.iter_mut().for_each(|count| *count = 0);
        for pixel in pixels {
            let nearest = nearest_index(*pixel, &centers);
            sums[nearest] = sums[nearest].add(*pixel);
            counts[nearest] += 1;
        }
        for ((center, sum), count) in centers.iter_mut().zip(&sums).zip(&counts) {
            if *count > 0 {
                *center = sum.scale(1.0 / *count as f32);
            }
        }
    }
    centers
        .into_iter()
        .zip(counts)
        .filter(|(_, count)| *count > 0)
        .map(|(center, count)| Cluster { center, count })
        .collect()
}

/// Merges clusters whose colors look nearly the same.
fn merge_close(clusters: Vec<Cluster>) -> Vec<Cluster> {
    let mut merged: Vec<Cluster> = Vec::new();
    for cluster in clusters {
        match merged.iter_mut().find(|other| other.center.distance(cluster.center) < MERGE_DISTANCE) {
            Some(other) => {
                let total = (other.count + cluster.count) as f32;
                other.center = other
                    .center
                    .scale(other.count as f32 / total)
                    .add(cluster.center.scale(cluster.count as f32 / total));
                other.count += cluster.count;
            }
            None => merged.push(cluster),
        }
    }
    merged
}

fn nearest_index(pixel: Lab, centers: &[Lab]) -> usize {
    (0..centers.len()).min_by(|&a, &b| pixel.distance(centers[a]).total_cmp(&pixel.distance(centers[b]))).unwrap_or(0)
}

fn nearest_distance(pixel: Lab, centers: &[Lab]) -> f32 {
    centers.iter().map(|center| pixel.distance(*center)).fold(f32::INFINITY, f32::min)
}

/// A color in Oklab: lightness `l` from 0 to 1, and the `a` (green–red) and
/// `b` (blue–yellow) axes.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Lab {
    l: f32,
    a: f32,
    b: f32,
}

impl Lab {
    fn from_srgb([r, g, b]: [u8; 3]) -> Self {
        let linear = |channel: u8| {
            let c = f32::from(channel) / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let (r, g, b) = (linear(r), linear(g), linear(b));
        let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
        let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
        let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
        Self {
            l: 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
            a: 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
            b: 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
        }
    }

    fn to_srgb(self) -> [u8; 3] {
        let l = (self.l + 0.396_337_78 * self.a + 0.215_803_76 * self.b).powi(3);
        let m = (self.l - 0.105_561_346 * self.a - 0.063_854_17 * self.b).powi(3);
        let s = (self.l - 0.089_484_18 * self.a - 1.291_485_5 * self.b).powi(3);
        let encode = |linear: f32| {
            let c = linear.clamp(0.0, 1.0);
            let srgb = if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
            (srgb * 255.0).round() as u8
        };
        [
            encode(4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s),
            encode(-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s),
            encode(-0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s),
        ]
    }

    fn add(self, other: Self) -> Self {
        Self { l: self.l + other.l, a: self.a + other.a, b: self.b + other.b }
    }

    fn scale(self, factor: f32) -> Self {
        Self { l: self.l * factor, a: self.a * factor, b: self.b * factor }
    }

    fn distance(self, other: Self) -> f32 {
        ((self.l - other.l).powi(2) + (self.a - other.a).powi(2) + (self.b - other.b).powi(2)).sqrt()
    }

    fn chroma(self) -> f32 {
        self.a.hypot(self.b)
    }

    fn hue(self) -> f32 {
        self.b.atan2(self.a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(width: u32, height: u32, color: impl Fn(u32, u32) -> [u8; 3]) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_fn(width, height, |x, y| image::Rgb(color(x, y))))
    }

    fn rgb(hex: &str) -> [u8; 3] {
        let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap();
        [(value >> 16) as u8, (value >> 8) as u8, value as u8]
    }

    fn close(a: [u8; 3], b: [u8; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 6)
    }

    #[test]
    fn oklab_round_trips() {
        for color in [[0, 0, 0], [255, 255, 255], [53, 132, 228], [230, 45, 66], [12, 200, 90]] {
            assert!(close(Lab::from_srgb(color).to_srgb(), color), "{color:?}");
        }
    }

    #[test]
    fn finds_the_main_colors_and_their_shares() {
        let palette = extract(&picture(200, 100, |x, _| if x < 100 { [30, 100, 200] } else { [245, 245, 245] }));
        assert_eq!(palette.colors.len(), 2, "{palette:?}");
        for color in &palette.colors {
            assert!((color.share - 0.5).abs() < 0.05, "{palette:?}");
        }
        assert!(palette.colors.iter().any(|color| close(rgb(&color.hex), [30, 100, 200])));
        assert!(palette.colors.iter().any(|color| close(rgb(&color.hex), [245, 245, 245])));
        assert_eq!(palette.accent_color, "blue");
        assert!(!palette.dark);
    }

    #[test]
    fn a_dark_picture_with_a_red_light_suits_the_dark_style_and_a_red_accent() {
        let palette = extract(&picture(200, 100, |x, y| if x > 170 && y > 50 { [220, 40, 50] } else { [12, 12, 16] }));
        assert!(palette.dark);
        assert_eq!(palette.accent_color, "red");
        assert!(palette.colors[0].share > 0.8, "the night sky comes first: {palette:?}");
    }

    #[test]
    fn grey_pictures_suggest_the_neutral_accent() {
        let palette = extract(&picture(120, 80, |x, _| [(x * 2) as u8; 3]));
        assert_eq!(palette.accent_color, "slate");
        assert!(palette.colors.len() <= PALETTE_SIZE);
    }

    #[test]
    fn each_gnome_accent_suggests_itself() {
        for (name, color) in GNOME_ACCENTS {
            assert_eq!(extract(&picture(40, 30, |_, _| *color)).accent_color, *name);
        }
    }

    #[test]
    fn the_same_picture_always_gives_the_same_palette() {
        let photo = picture(300, 200, |x, y| [(x % 256) as u8, (y * 3 % 256) as u8, ((x + y) % 256) as u8]);
        assert_eq!(extract(&photo), extract(&photo));
    }
}
