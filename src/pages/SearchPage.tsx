import { Search } from "lucide-react";
import { useCallback, useMemo } from "react";
import { Link, useSearchParams } from "react-router-dom";

import { ProviderFilter } from "../components/ProviderFilter";
import { SearchBar } from "../components/SearchBar";
import { SearchRotationPanel } from "../components/SearchRotationPanel";
import { Button } from "../components/ui/Button";
import { EmptyState } from "../components/ui/EmptyState";
import { ExternalLink } from "../components/ui/ExternalLink";
import { Notice } from "../components/ui/Notice";
import { PageHeader } from "../components/ui/PageHeader";
import { Spinner } from "../components/ui/Spinner";
import { WallpaperGrid } from "../components/WallpaperGrid";
import { useInfiniteScroll } from "../hooks/useInfiniteScroll";
import { useProviders } from "../hooks/useProviders";
import { ALL_PROVIDERS, useWallpaperSearch } from "../hooks/useWallpaperSearch";
import { errorMessage, isValidationError } from "../services/errors";
import { uniqueWallpapers } from "../services/wallpapers";
import type { ProviderError, ProviderInfo } from "../types";

const SKELETON_TILES = 12;

// en-GB: no serial comma, like the rest of the app's copy.
const listFormat = new Intl.ListFormat("en-GB", { type: "conjunction" });

/** "Wallhaven, Unsplash and Wikimedia Commons". */
function listNames(providers: ProviderInfo[]): string {
  return listFormat.format(providers.map((provider) => provider.name));
}

export function SearchPage() {
  const [params, setParams] = useSearchParams();
  const query = params.get("q")?.trim() ?? "";
  const requestedProvider = params.get("provider") ?? ALL_PROVIDERS;

  const providers = useProviders();
  const enabledProviders = useMemo(() => (providers.data ?? []).filter((p) => p.enabled), [providers.data]);
  // Searching all providers skips the ones still waiting for an API key.
  const readyProviders = enabledProviders.filter((p) => p.ready);
  const waitingForKey = enabledProviders.filter((p) => !p.ready);
  // A provider turned off in Settings falls back to searching everything.
  const provider =
    requestedProvider === ALL_PROVIDERS || !providers.data || enabledProviders.some((p) => p.id === requestedProvider)
      ? requestedProvider
      : ALL_PROVIDERS;

  const search = useWallpaperSearch(query, provider);
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = search;
  const pages = search.data?.pages;

  const wallpapers = useMemo(() => uniqueWallpapers(pages?.flatMap((page) => page.wallpapers) ?? []), [pages]);

  // The most recent error from each provider.
  const providerErrors = useMemo(() => {
    const byProvider = new Map<string, ProviderError>();
    for (const page of pages ?? []) {
      for (const error of page.errors) byProvider.set(error.provider, error);
    }
    return [...byProvider.values()];
  }, [pages]);

  const attributions = useMemo(() => {
    const shown = new Set(wallpapers.map((wallpaper) => wallpaper.provider));
    return (providers.data ?? []).filter((p) => p.attribution && shown.has(p.id));
  }, [wallpapers, providers.data]);

  const loadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) void fetchNextPage();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);
  const sentinel = useInfiniteScroll<HTMLDivElement>(
    loadMore,
    hasNextPage && !isFetchingNextPage && !search.isFetchNextPageError,
    pages?.length ?? 0,
  );

  function update(next: { q?: string; provider?: string }) {
    const merged = new URLSearchParams(params);
    if (next.q !== undefined) merged.set("q", next.q);
    if (next.provider !== undefined) {
      if (next.provider === ALL_PROVIDERS) merged.delete("provider");
      else merged.set("provider", next.provider);
    }
    setParams(merged);
  }

  const providerName = (id: string) => providers.data?.find((p) => p.id === id)?.name ?? id;

  return (
    <>
      <PageHeader title="Search" />
      <div className="flex flex-col gap-4">
        <SearchBar key={query} initialQuery={query} onSearch={(q) => update({ q })} autoFocus={!query} />
        <ProviderFilter providers={providers.data ?? []} value={provider} onChange={(value) => update({ provider: value })} />
      </div>

      <div className="mt-8">
        {!query ? (
          <EmptyState icon={<Search />} title="Search for wallpapers">
            {readyProviders.length > 0
              ? `Type a keyword above. Results come from ${listNames(readyProviders)}.`
              : "Turn on a provider in Settings, then type a keyword above."}
            {waitingForKey.length > 0 && (
              <>
                {" "}
                <Link to="/settings" className="text-accent hover:underline">
                  Add an API key
                </Link>{" "}
                to also search {listNames(waitingForKey)}.
              </>
            )}
          </EmptyState>
        ) : search.isPending ? (
          <ul aria-label="Loading results" className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
            {Array.from({ length: SKELETON_TILES }, (_, index) => (
              <li key={index} className="aspect-[16/10] rounded-tile bg-raised/70" />
            ))}
          </ul>
        ) : !search.data ? (
          <Notice
            action={
              !isValidationError(search.error) && (
                <Button size="sm" onClick={() => void search.refetch()}>
                  Try again
                </Button>
              )
            }
          >
            {errorMessage(search.error)}
          </Notice>
        ) : (
          <>
            {providerErrors.length > 0 && (
              <div className="mb-6 flex flex-col gap-2">
                {providerErrors.map((error) => (
                  <Notice
                    key={error.provider}
                    action={
                      <Link to="/settings" className="text-[13px] text-accent hover:underline">
                        Open settings
                      </Link>
                    }
                  >
                    <span className="font-medium">{providerName(error.provider)}:</span> {error.message}
                  </Notice>
                ))}
              </div>
            )}

            {wallpapers.length === 0 ? (
              <EmptyState icon={<Search />} title={`No wallpapers match “${query}”`}>
                Try a broader keyword or another provider.
              </EmptyState>
            ) : (
              <>
                <SearchRotationPanel key={query} query={query} wallpapers={wallpapers} />
                <WallpaperGrid label={`Results for ${query}`} wallpapers={wallpapers} />
              </>
            )}

            <div ref={sentinel} className="flex min-h-16 items-center pt-8 text-muted">
              {isFetchingNextPage && (
                <span className="flex items-center gap-2">
                  <Spinner className="size-4" />
                  Loading more wallpapers
                </span>
              )}
              {search.isFetchNextPageError && (
                <span className="flex items-center gap-3">
                  {errorMessage(search.error)}
                  <Button size="sm" onClick={loadMore}>
                    Try again
                  </Button>
                </span>
              )}
              {!hasNextPage && wallpapers.length > 0 && <span>That’s every result for “{query}”.</span>}
            </div>

            {attributions.length > 0 && (
              <p className="mt-2 text-[13px] text-muted">
                {attributions.map((p, index) => (
                  <span key={p.id}>
                    {index > 0 && " "}
                    <ExternalLink href={p.website}>{p.attribution}</ExternalLink>
                  </span>
                ))}
              </p>
            )}
          </>
        )}
      </div>
    </>
  );
}
