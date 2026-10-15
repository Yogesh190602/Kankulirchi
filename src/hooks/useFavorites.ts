import { useQuery } from "@tanstack/react-query";

import { api } from "../services/api";
import { queryKeys } from "../services/queryKeys";

export function useFavorites() {
  return useQuery({ queryKey: queryKeys.favorites, queryFn: api.listFavorites });
}
