import { useQuery } from "@tanstack/react-query";

import { api } from "../services/api";
import { queryKeys } from "../services/queryKeys";

export function useProviders() {
  return useQuery({ queryKey: queryKeys.providers, queryFn: api.listProviders });
}
