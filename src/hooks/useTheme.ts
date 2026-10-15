import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../services/api";
import { errorMessage } from "../services/errors";
import { queryKeys } from "../services/queryKeys";
import { notify } from "../store/ui";
import type { Wallpaper } from "../types";

/** A wallpaper's colors. They never change, so they are fetched once. */
export function useWallpaperPalette(wallpaper: Wallpaper) {
  return useQuery({
    queryKey: queryKeys.palette(wallpaper.provider, wallpaper.providerId),
    queryFn: () => api.wallpaperPalette(wallpaper),
    staleTime: Infinity,
  });
}

export function useDesktopBackup() {
  return useQuery({ queryKey: queryKeys.desktopBackup, queryFn: api.getDesktopBackup, staleTime: 0 });
}

export function useRestoreDesktop() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: api.restoreDesktop,
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.rotationStatus, status);
      notify("Desktop restored");
    },
    onError: (error) => notify(errorMessage(error), "error"),
  });
}
