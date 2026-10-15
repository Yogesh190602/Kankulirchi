import { listen } from "@tauri-apps/api/event";
import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";

import { REPEATS_REMOVED, ROTATION_STATUS, WALLPAPER_APPLIED, WALLPAPER_CACHED } from "../services/events";
import { pluralize } from "../services/format";
import { publishWallpaper } from "../services/library";
import { queryKeys } from "../services/queryKeys";
import { notify } from "../store/ui";
import type { HistoryEntry, RepeatsRemoved, RotationStatus, Wallpaper } from "../types";

/** Keeps cached queries in sync with changes made by the backend. */
export function useBackendEvents() {
  const queryClient = useQueryClient();

  useEffect(() => {
    // Removals in one collection often come in quick succession while it
    // downloads; they are announced together.
    const removals = new Map<number, { name: string; count: number; timer: number }>();
    const announceRemovals = ({ collectionId, collectionName, count }: RepeatsRemoved) => {
      const pending = removals.get(collectionId) ?? { name: collectionName, count: 0, timer: 0 };
      window.clearTimeout(pending.timer);
      pending.count += count;
      pending.timer = window.setTimeout(() => {
        removals.delete(collectionId);
        const kept = pending.count === 1 ? "copy" : "copies";
        notify(`Removed ${pluralize(pending.count, "repeated picture")} from ${pending.name}, keeping the sharpest ${kept}`);
      }, 2000);
      removals.set(collectionId, pending);
    };

    const subscriptions = [
      listen<HistoryEntry>(WALLPAPER_APPLIED, ({ payload }) => {
        publishWallpaper(payload.wallpaper);
        void queryClient.invalidateQueries({ queryKey: queryKeys.rotationStatus });
      }),
      listen<RotationStatus>(ROTATION_STATUS, ({ payload }) => {
        queryClient.setQueryData(queryKeys.rotationStatus, payload);
        void queryClient.invalidateQueries({ queryKey: ["rotation", "settings"] });
      }),
      listen<Wallpaper>(WALLPAPER_CACHED, ({ payload }) => publishWallpaper(payload)),
      listen<RepeatsRemoved>(REPEATS_REMOVED, ({ payload }) => {
        void queryClient.invalidateQueries({ queryKey: queryKeys.collections });
        void queryClient.invalidateQueries({ queryKey: queryKeys.wallpaperStatusAll });
        announceRemovals(payload);
      }),
    ];
    return () => {
      for (const { timer } of removals.values()) window.clearTimeout(timer);
      for (const subscription of subscriptions) {
        void subscription.then((unlisten) => unlisten());
      }
    };
  }, [queryClient]);
}
