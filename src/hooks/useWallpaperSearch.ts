import { useInfiniteQuery } from "@tanstack/react-query";

import { api } from "../services/api";
import { queryKeys } from "../services/queryKeys";

/** Provider filter value that searches every enabled provider. */
export const ALL_PROVIDERS = "all";

interface PageParam {
  page: number;
  /** Providers to query; empty means every enabled provider. */
  providers: string[];
}

export function useWallpaperSearch(query: string, provider: string) {
  const keyword = query.trim();
  const initialPageParam: PageParam = { page: 1, providers: provider === ALL_PROVIDERS ? [] : [provider] };

  return useInfiniteQuery({
    queryKey: queryKeys.search(keyword, provider),
    queryFn: ({ pageParam }) => api.searchWallpapers(keyword, pageParam.providers, pageParam.page),
    initialPageParam,
    // Later pages only ask the providers that reported more results.
    getNextPageParam: (last): PageParam | undefined =>
      last.providersWithMore.length > 0 ? { page: last.page + 1, providers: last.providersWithMore } : undefined,
    enabled: keyword.length > 0,
    staleTime: 10 * 60_000,
  });
}
