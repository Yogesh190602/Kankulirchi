import { useInfiniteQuery, useQuery } from "@tanstack/react-query";

import { api } from "../services/api";
import { queryKeys } from "../services/queryKeys";

const HISTORY_PAGE_SIZE = 50;

/** Recently applied wallpapers without repeats. */
export function useRecentWallpapers(limit: number) {
  return useQuery({ queryKey: queryKeys.recentWallpapers(limit), queryFn: () => api.recentWallpapers(limit) });
}

/** Every application, newest first, loaded page by page. */
export function useHistoryLog() {
  return useInfiniteQuery({
    queryKey: queryKeys.historyLog,
    queryFn: ({ pageParam }) => api.listHistory(HISTORY_PAGE_SIZE, pageParam),
    initialPageParam: 0,
    getNextPageParam: (last, pages) =>
      last.length === HISTORY_PAGE_SIZE ? pages.length * HISTORY_PAGE_SIZE : undefined,
  });
}
