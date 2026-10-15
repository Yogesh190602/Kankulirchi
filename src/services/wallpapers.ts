import { convertFileSrc } from "@tauri-apps/api/core";

import type { Credit, Wallpaper } from "../types";

export function wallpaperKey(wallpaper: Pick<Wallpaper, "provider" | "providerId">): string {
  return `${wallpaper.provider}:${wallpaper.providerId}`;
}

export function isSameWallpaper(a: Wallpaper, b: Wallpaper): boolean {
  return a.provider === b.provider && a.providerId === b.providerId;
}

/** Full-size image source: the cached file when there is one. */
export function fullImageSrc(wallpaper: Wallpaper): string {
  return wallpaper.localPath ? convertFileSrc(wallpaper.localPath) : wallpaper.imageUrl;
}

/** Copies library state (id, favorite, cached file) from `source` onto `target`. */
export function withLibraryState(target: Wallpaper, source: Wallpaper): Wallpaper {
  return {
    ...target,
    id: source.id,
    isFavorite: source.isFavorite,
    localPath: source.localPath,
    createdAt: source.createdAt,
    width: source.width || target.width,
    height: source.height || target.height,
    credit: mergeCredit(target.credit, source.credit),
  };
}

/** `preferred`, with any missing part taken from `fallback`. */
function mergeCredit(preferred: Credit, fallback: Credit): Credit {
  return {
    author: preferred.author ?? fallback.author,
    authorUrl: preferred.authorUrl ?? fallback.authorUrl,
    sourceUrl: preferred.sourceUrl ?? fallback.sourceUrl,
    license: preferred.license ?? fallback.license,
    licenseUrl: preferred.licenseUrl ?? fallback.licenseUrl,
  };
}

/** Removes repeated wallpapers, keeping the first occurrence. */
export function uniqueWallpapers(wallpapers: Wallpaper[]): Wallpaper[] {
  const seen = new Set<string>();
  return wallpapers.filter((wallpaper) => {
    const key = wallpaperKey(wallpaper);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
