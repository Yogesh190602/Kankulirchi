import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../services/api";
import { errorMessage } from "../services/errors";
import { formatBytes } from "../services/format";
import { publishWallpaper, refreshLibrary } from "../services/library";
import { queryKeys } from "../services/queryKeys";
import { notify } from "../store/ui";
import type { Wallpaper } from "../types";

export function useCollections() {
  return useQuery({ queryKey: queryKeys.collections, queryFn: api.listCollections });
}

export function useCollection(id: number) {
  return useQuery({ queryKey: queryKeys.collection(id), queryFn: () => api.getCollection(id) });
}

export function useCreateCollection() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (name: string) => api.createCollection(name),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: queryKeys.collections }),
  });
}

export function useRenameCollection() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, name }: { id: number; name: string }) => api.renameCollection(id, name),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: queryKeys.collections });
      void queryClient.invalidateQueries({ queryKey: queryKeys.rotationStatus });
    },
  });
}

export function useDeleteCollection() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id }: { id: number; name: string }) => api.deleteCollection(id),
    onSuccess: (freed, { id, name }) => {
      // Mark the detail stale without refetching it while its page is still open.
      void queryClient.invalidateQueries({ queryKey: queryKeys.collection(id), refetchType: "none" });
      void queryClient.invalidateQueries({ queryKey: queryKeys.collections, exact: true });
      void queryClient.invalidateQueries({ queryKey: queryKeys.rotationStatus });
      void queryClient.invalidateQueries({ queryKey: queryKeys.wallpaperStatusAll });
      // History entries may point at downloads that were just deleted.
      void queryClient.invalidateQueries({ queryKey: queryKeys.history });
      void queryClient.invalidateQueries({ queryKey: queryKeys.cacheStats });
      notify(freed.fileCount > 0 ? `Deleted ${name} and freed ${formatBytes(freed.totalBytes)}` : `Deleted ${name}`);
    },
    onError: (error) => notify(errorMessage(error), "error"),
  });
}

interface MembershipChange {
  collectionId: number;
  wallpaper: Wallpaper;
  include: boolean;
}

/** Adds a wallpaper to a collection or removes it. */
export function useCollectionMembership() {
  return useMutation({
    mutationFn: async ({ collectionId, wallpaper, include }: MembershipChange) => {
      if (include) return api.addToCollection(collectionId, wallpaper);
      if (wallpaper.id !== null) await api.removeFromCollection(collectionId, wallpaper.id);
      return wallpaper;
    },
    onSuccess: (wallpaper, { include }) => {
      if (include) publishWallpaper(wallpaper);
      else refreshLibrary();
    },
    onError: (error) => {
      notify(errorMessage(error), "error");
      // Undo optimistic updates.
      refreshLibrary();
    },
  });
}
