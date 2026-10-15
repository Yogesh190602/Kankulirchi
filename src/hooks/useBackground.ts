import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { api } from "../services/api";
import { errorMessage } from "../services/errors";
import { queryKeys } from "../services/queryKeys";
import { notify } from "../store/ui";

export function useLaunchAtLogin() {
  return useQuery({ queryKey: queryKeys.launchAtLogin, queryFn: api.getLaunchAtLogin });
}

export function useSetLaunchAtLogin() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (enabled: boolean) => api.setLaunchAtLogin(enabled),
    onSuccess: (enabled) => queryClient.setQueryData(queryKeys.launchAtLogin, enabled),
    onError: (error) => notify(errorMessage(error), "error"),
  });
}

/** Quits completely, stopping rotation. The request never answers on success. */
export function useQuitApp() {
  return useMutation({
    mutationFn: api.quitApp,
    onError: (error) => notify(errorMessage(error), "error"),
  });
}
