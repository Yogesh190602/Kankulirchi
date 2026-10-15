import { skipToken, useMutation, useQuery } from "@tanstack/react-query";

import { api } from "../services/api";
import { errorMessage } from "../services/errors";
import { publishWallpaper } from "../services/library";
import { queryKeys } from "../services/queryKeys";
import { notify } from "../store/ui";
import type { Wallpaper } from "../types";

/** Live library state of a wallpaper, including unsaved search results. */
export function useWallpaperStatus(wallpaper: Wallpaper | null) {
  return useQuery({
    queryKey: wallpaper
      ? queryKeys.wallpaperStatus(wallpaper.provider, wallpaper.providerId)
      : queryKeys.wallpaperStatusAll,
    queryFn: wallpaper ? () => api.getWallpaperStatus(wallpaper.provider, wallpaper.providerId) : skipToken,
    staleTime: 0,
  });
}

export function useApplyWallpaper() {
  return useMutation({
    mutationFn: (wallpaper: Wallpaper) => api.applyWallpaper(wallpaper),
    onSuccess: (entry) => {
      publishWallpaper(entry.wallpaper);
      notify("Wallpaper applied");
    },
    onError: (error) => notify(errorMessage(error), "error"),
  });
}

export function useDownloadWallpaper() {
  return useMutation({
    mutationFn: (wallpaper: Wallpaper) => api.downloadWallpaper(wallpaper),
    onSuccess: (wallpaper) => {
      publishWallpaper(wallpaper);
      notify("Downloaded");
    },
    onError: (error) => notify(errorMessage(error), "error"),
  });
}

export function useToggleFavorite() {
  return useMutation({
    mutationFn: (wallpaper: Wallpaper) =>
      wallpaper.isFavorite && wallpaper.id !== null ? api.removeFavorite(wallpaper.id) : api.addFavorite(wallpaper),
    onSuccess: publishWallpaper,
    onError: (error) => notify(errorMessage(error), "error"),
  });
}
