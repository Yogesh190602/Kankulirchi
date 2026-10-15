import { invoke } from "@tauri-apps/api/core";

import type {
  CacheStats,
  Collection,
  CollectionDetail,
  DesktopBackup,
  FreedSpace,
  HistoryEntry,
  Palette,
  ProviderInfo,
  RotationMode,
  RotationSettings,
  RotationStatus,
  SearchResponse,
  SearchRotation,
  Settings,
  Wallpaper,
  WallpaperStatus,
} from "../types";

/**
 * Typed wrappers around the Tauri commands registered in src-tauri/src/lib.rs.
 * Every backend call goes through this object.
 */
export const api = {
  // Search
  listProviders: () => invoke<ProviderInfo[]>("list_providers"),
  searchWallpapers: (query: string, providers: string[], page: number) =>
    invoke<SearchResponse>("search_wallpapers", { query, providers, page }),

  // Collections
  listCollections: () => invoke<Collection[]>("list_collections"),
  getCollection: (id: number) => invoke<CollectionDetail>("get_collection", { id }),
  createCollection: (name: string) => invoke<Collection>("create_collection", { name }),
  renameCollection: (id: number, name: string) => invoke<Collection>("rename_collection", { id, name }),
  deleteCollection: (id: number) => invoke<FreedSpace>("delete_collection", { id }),
  addToCollection: (collectionId: number, wallpaper: Wallpaper) =>
    invoke<Wallpaper>("add_to_collection", { collectionId, wallpaper }),
  removeFromCollection: (collectionId: number, wallpaperId: number) =>
    invoke<void>("remove_from_collection", { collectionId, wallpaperId }),

  // Favorites
  listFavorites: () => invoke<Wallpaper[]>("list_favorites"),
  addFavorite: (wallpaper: Wallpaper) => invoke<Wallpaper>("add_favorite", { wallpaper }),
  removeFavorite: (wallpaperId: number) => invoke<Wallpaper>("remove_favorite", { wallpaperId }),

  // Wallpapers, cache and history
  getWallpaperStatus: (provider: string, providerId: string) =>
    invoke<WallpaperStatus>("get_wallpaper_status", { provider, providerId }),
  downloadWallpaper: (wallpaper: Wallpaper) => invoke<Wallpaper>("download_wallpaper", { wallpaper }),
  applyWallpaper: (wallpaper: Wallpaper) => invoke<HistoryEntry>("apply_wallpaper", { wallpaper }),
  listHistory: (limit: number, offset: number) => invoke<HistoryEntry[]>("list_history", { limit, offset }),
  recentWallpapers: (limit: number) => invoke<HistoryEntry[]>("recent_wallpapers", { limit }),

  // Rotation
  getRotationStatus: () => invoke<RotationStatus>("get_rotation_status"),
  getRotationSettings: (collectionId: number) =>
    invoke<RotationSettings | null>("get_rotation_settings", { collectionId }),
  saveRotationSettings: (collectionId: number, intervalSeconds: number, rotationMode: RotationMode) =>
    invoke<RotationSettings>("save_rotation_settings", { collectionId, intervalSeconds, rotationMode }),
  startRotation: (collectionId: number, intervalSeconds: number, rotationMode: RotationMode) =>
    invoke<RotationStatus>("start_rotation", { collectionId, intervalSeconds, rotationMode }),
  startSearchRotation: (name: string, wallpapers: Wallpaper[], intervalSeconds: number, rotationMode: RotationMode) =>
    invoke<SearchRotation>("start_search_rotation", { name, wallpapers, intervalSeconds, rotationMode }),
  stopRotation: () => invoke<RotationStatus>("stop_rotation"),

  // Background
  getLaunchAtLogin: () => invoke<boolean>("get_launch_at_login"),
  setLaunchAtLogin: (enabled: boolean) => invoke<boolean>("set_launch_at_login", { enabled }),
  quitApp: () => invoke<void>("quit_app"),

  // Theme
  wallpaperPalette: (wallpaper: Wallpaper) => invoke<Palette>("wallpaper_palette", { wallpaper }),
  getDesktopBackup: () => invoke<DesktopBackup | null>("get_desktop_backup"),
  restoreDesktop: () => invoke<RotationStatus>("restore_desktop"),

  // Settings
  getSettings: () => invoke<Settings>("get_settings"),
  updateProviderSettings: (providerId: string, enabled: boolean, apiKey: string | null) =>
    invoke<Settings>("update_provider_settings", { providerId, enabled, apiKey }),
  setCacheDir: (path: string | null) => invoke<Settings>("set_cache_dir", { path }),
  getCacheStats: () => invoke<CacheStats>("get_cache_stats"),
};
