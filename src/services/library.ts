import type { InfiniteData } from "@tanstack/react-query";

import { syncDetail } from "../store/ui";
import type { SearchResponse, Wallpaper } from "../types";
import { queryClient } from "./queryClient";
import { queryKeys } from "./queryKeys";
import { isSameWallpaper, withLibraryState } from "./wallpapers";

/** Refetches every query backed by the local library. */
export function refreshLibrary() {
  const keys = [
    queryKeys.favorites,
    queryKeys.collections,
    queryKeys.history,
    queryKeys.wallpaperStatusAll,
    queryKeys.cacheStats,
  ];
  for (const queryKey of keys) {
    void queryClient.invalidateQueries({ queryKey });
  }
}

/**
 * Propagates a changed wallpaper everywhere it is shown. Search results are
 * patched in place rather than refetched, which would call provider APIs
 * again for every loaded page.
 */
export function publishWallpaper(updated: Wallpaper) {
  queryClient.setQueriesData<InfiniteData<SearchResponse>>({ queryKey: queryKeys.searchAll }, (data) => {
    if (!data) return data;
    return {
      ...data,
      pages: data.pages.map((page) => ({
        ...page,
        wallpapers: page.wallpapers.map((wallpaper) =>
          isSameWallpaper(wallpaper, updated) ? withLibraryState(wallpaper, updated) : wallpaper,
        ),
      })),
    };
  });
  syncDetail(updated);
  refreshLibrary();
}
