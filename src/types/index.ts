// Types mirror the serialized Rust models in src-tauri/src/models.rs and the
// command responses in src-tauri/src/commands.

/** Who made a wallpaper and where it comes from. Links point to the provider's own hosts. */
export interface Credit {
  author: string | null;
  /** The author's profile on the provider's website. */
  authorUrl: string | null;
  /** The wallpaper's page on the provider's website. */
  sourceUrl: string | null;
  license: string | null;
  licenseUrl: string | null;
}

export interface Wallpaper {
  /** Database id. `null` for search results that were never saved. */
  id: number | null;
  provider: string;
  providerId: string;
  title: string;
  imageUrl: string;
  thumbnailUrl: string;
  /** Absolute path of the cached file, once downloaded. */
  localPath: string | null;
  width: number;
  height: number;
  createdAt: string | null;
  isFavorite: boolean;
  credit: Credit;
}

export interface WallpaperStatus {
  wallpaper: Wallpaper | null;
  collectionIds: number[];
}

export interface Collection {
  id: number;
  name: string;
  createdAt: string;
  wallpaperCount: number;
  previewThumbnails: string[];
}

export interface CollectionDetail {
  collection: Collection;
  wallpapers: Wallpaper[];
}

export interface HistoryEntry {
  id: number;
  appliedAt: string;
  wallpaper: Wallpaper;
}

export type RotationMode = "sequential" | "random";

export interface RotationSettings {
  id: number;
  collectionId: number;
  intervalSeconds: number;
  rotationMode: RotationMode;
  enabled: boolean;
}

export interface RotationStatus {
  active: boolean;
  collectionId: number | null;
  collectionName: string | null;
  intervalSeconds: number | null;
  rotationMode: RotationMode | null;
  lastAppliedAt: string | null;
  nextRunAt: string | null;
  lastError: string | null;
}

/** Search results saved to a collection that is now rotating. */
export interface SearchRotation {
  collection: Collection;
  status: RotationStatus;
}

export type ApiKeyRequirement = "none" | "optional" | "required";

export interface ProviderInfo {
  id: string;
  name: string;
  website: string;
  apiKeyRequirement: ApiKeyRequirement;
  apiKeyUrl: string | null;
  attribution: string | null;
  enabled: boolean;
  hasApiKey: boolean;
  /** Whether the provider can be searched with the current settings. */
  ready: boolean;
}

export interface ProviderSettings extends ProviderInfo {
  apiKey: string | null;
}

export interface Settings {
  cacheDir: string;
  defaultCacheDir: string;
  providers: ProviderSettings[];
}

export interface ProviderError {
  provider: string;
  message: string;
}

export interface SearchResponse {
  wallpapers: Wallpaper[];
  page: number;
  providersWithMore: string[];
  errors: ProviderError[];
}

export interface CacheStats {
  path: string;
  totalBytes: number;
  fileCount: number;
}

/** A wallpaper's colors. */
export interface Palette {
  /** Main colors, most common first. */
  colors: { hex: string; share: number }[];
  /** The most vivid main color. */
  vivid: string;
  /** GNOME's accent color that suits the wallpaper, such as "blue". */
  accentColor: string;
  /** Whether the wallpaper suits GNOME's dark style. */
  dark: boolean;
}

/** What restoring the desktop sets. */
export interface DesktopBackup {
  takenAt: string;
  /** Absent if not saved; `null` means GNOME's default wallpaper. */
  wallpaper?: string | null;
  accentColor: string | null;
  colorScheme: string | null;
}

/** Copies of pictures removed from a collection, which kept the sharpest copy of each. */
export interface RepeatsRemoved {
  collectionId: number;
  collectionName: string;
  count: number;
}

/** Downloads deleted to free up space. */
export interface FreedSpace {
  fileCount: number;
  totalBytes: number;
}

export interface AppErrorPayload {
  kind: string;
  message: string;
}
