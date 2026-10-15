import { QueryClient } from "@tanstack/react-query";

import { isValidationError } from "./errors";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Library data is local and changes only through this app, which
      // invalidates it explicitly.
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      // Retry once, unless the request was refused as invalid.
      retry: (failureCount, error) => failureCount < 1 && !isValidationError(error),
    },
    mutations: {
      retry: false,
    },
  },
});
