import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../services/api";
import { errorMessage } from "../services/errors";
import { queryKeys } from "../services/queryKeys";
import { notify } from "../store/ui";

export function useSettings() {
  return useQuery({ queryKey: queryKeys.settings, queryFn: api.getSettings });
}

export function useCacheStats() {
  return useQuery({ queryKey: queryKeys.cacheStats, queryFn: api.getCacheStats, staleTime: 0 });
}

interface ProviderChange {
  providerId: string;
  enabled: boolean;
  apiKey: string | null;
}

export function useUpdateProviderSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ providerId, enabled, apiKey }: ProviderChange) =>
      api.updateProviderSettings(providerId, enabled, apiKey),
    onSuccess: (settings) => {
      queryClient.setQueryData(queryKeys.settings, settings);
      void queryClient.invalidateQueries({ queryKey: queryKeys.providers });
      // Earlier results may have failed for lack of a key.
      void queryClient.invalidateQueries({ queryKey: queryKeys.searchAll });
    },
    onError: (error) => notify(errorMessage(error), "error"),
  });
}

export function useSetCacheDir() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (path: string | null) => api.setCacheDir(path),
    onSuccess: (settings) => {
      queryClient.setQueryData(queryKeys.settings, settings);
      void queryClient.invalidateQueries({ queryKey: queryKeys.cacheStats });
      notify("Cache location saved");
    },
    onError: (error) => notify(errorMessage(error), "error"),
  });
}
