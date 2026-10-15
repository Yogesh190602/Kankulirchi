import { skipToken, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../services/api";
import { errorMessage } from "../services/errors";
import { refreshLibrary } from "../services/library";
import { queryKeys } from "../services/queryKeys";
import { notify } from "../store/ui";
import type { RotationMode, Wallpaper } from "../types";

export function useRotationStatus() {
  return useQuery({
    queryKey: queryKeys.rotationStatus,
    queryFn: api.getRotationStatus,
    // The backend pushes changes as events; polling keeps the countdown honest
    // if one is missed.
    refetchInterval: 60_000,
  });
}

export function useRotationSettings(collectionId: number | null) {
  return useQuery({
    queryKey: queryKeys.rotationSettings(collectionId ?? -1),
    queryFn: collectionId === null ? skipToken : () => api.getRotationSettings(collectionId),
  });
}

interface RotationRequest {
  collectionId: number;
  intervalSeconds: number;
  rotationMode: RotationMode;
}

export function useSaveRotationSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ collectionId, intervalSeconds, rotationMode }: RotationRequest) =>
      api.saveRotationSettings(collectionId, intervalSeconds, rotationMode),
    onSuccess: (settings) => {
      queryClient.setQueryData(queryKeys.rotationSettings(settings.collectionId), settings);
      void queryClient.invalidateQueries({ queryKey: queryKeys.rotationStatus });
    },
    onError: (error) => notify(errorMessage(error), "error"),
  });
}

export function useStartRotation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ collectionId, intervalSeconds, rotationMode }: RotationRequest) =>
      api.startRotation(collectionId, intervalSeconds, rotationMode),
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.rotationStatus, status);
      void queryClient.invalidateQueries({ queryKey: ["rotation", "settings"] });
      if (status.lastError) {
        notify(`Rotation started, but the first change failed: ${status.lastError}`, "error");
      } else {
        notify("Rotation started");
      }
    },
    onError: (error) => notify(errorMessage(error), "error"),
  });
}

interface SearchRotationRequest {
  name: string;
  wallpapers: Wallpaper[];
  intervalSeconds: number;
  rotationMode: RotationMode;
}

/** Saves search results to a collection and rotates it. */
export function useStartSearchRotation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ name, wallpapers, intervalSeconds, rotationMode }: SearchRotationRequest) =>
      api.startSearchRotation(name, wallpapers, intervalSeconds, rotationMode),
    onSuccess: ({ collection, status }) => {
      queryClient.setQueryData(queryKeys.rotationStatus, status);
      void queryClient.invalidateQueries({ queryKey: ["rotation", "settings"] });
      void queryClient.invalidateQueries({ queryKey: queryKeys.collection(collection.id) });
      refreshLibrary();
      if (status.lastError) {
        notify(`Rotation started, but the first change failed: ${status.lastError}`, "error");
      } else {
        notify(`Rotating ${collection.wallpaperCount} wallpapers from ${collection.name}`);
      }
    },
    onError: (error) => notify(errorMessage(error), "error"),
  });
}

export function useStopRotation() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: api.stopRotation,
    onSuccess: (status) => {
      queryClient.setQueryData(queryKeys.rotationStatus, status);
      void queryClient.invalidateQueries({ queryKey: ["rotation", "settings"] });
      notify("Rotation stopped");
    },
    onError: (error) => notify(errorMessage(error), "error"),
  });
}
