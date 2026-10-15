import { History, RotateCcw } from "lucide-react";
import { useCallback, useMemo } from "react";

import { Button } from "../components/ui/Button";
import { EmptyState } from "../components/ui/EmptyState";
import { Notice } from "../components/ui/Notice";
import { PageHeader } from "../components/ui/PageHeader";
import { LoadingBlock, Spinner } from "../components/ui/Spinner";
import { useHistoryLog } from "../hooks/useHistory";
import { useInfiniteScroll } from "../hooks/useInfiniteScroll";
import { useNow } from "../hooks/useNow";
import { useApplyWallpaper } from "../hooks/useWallpaperActions";
import { errorMessage } from "../services/errors";
import { formatDateTime, formatRelative } from "../services/format";
import { useDetailStore } from "../store/ui";
import type { HistoryEntry } from "../types";

export function HistoryPage() {
  const history = useHistoryLog();
  const { hasNextPage, isFetchingNextPage, fetchNextPage } = history;
  const pages = history.data?.pages;
  const apply = useApplyWallpaper();
  const openDetail = useDetailStore((state) => state.open);
  const now = useNow();

  const entries = useMemo(() => {
    // Offsets shift when new entries arrive; drop repeats across pages.
    const seen = new Set<number>();
    return (pages?.flat() ?? []).filter((entry) => {
      if (seen.has(entry.id)) return false;
      seen.add(entry.id);
      return true;
    });
  }, [pages]);

  const loadMore = useCallback(() => {
    if (hasNextPage && !isFetchingNextPage) void fetchNextPage();
  }, [hasNextPage, isFetchingNextPage, fetchNextPage]);
  const sentinel = useInfiniteScroll<HTMLDivElement>(loadMore, hasNextPage && !isFetchingNextPage, pages?.length ?? 0);

  const reapplying = (entry: HistoryEntry) => apply.isPending && apply.variables?.id === entry.wallpaper.id;

  return (
    <>
      <PageHeader title="History" description="The last 1,000 wallpapers Kankulirchi applied, newest first." />
      {history.isPending ? (
        <LoadingBlock label="Loading history" />
      ) : history.isError ? (
        <Notice>{errorMessage(history.error)}</Notice>
      ) : entries.length === 0 ? (
        <EmptyState icon={<History />} title="No wallpapers applied yet">
          When you apply a wallpaper, or rotation changes it, it is listed here so you can apply it again.
        </EmptyState>
      ) : (
        <>
          <ol className="flex flex-col divide-y divide-line/70">
            {entries.map((entry) => (
              <li key={entry.id} className="flex items-center gap-4 py-3">
                <button
                  type="button"
                  onClick={() => openDetail(entry.wallpaper)}
                  aria-label={`Open ${entry.wallpaper.title}`}
                  className="aspect-[16/10] w-36 shrink-0 overflow-hidden rounded-lg bg-raised"
                >
                  <img
                    src={entry.wallpaper.thumbnailUrl}
                    alt=""
                    loading="lazy"
                    draggable={false}
                    className="h-full w-full object-cover"
                  />
                </button>
                <div className="min-w-0 flex-1">
                  <p className="truncate font-medium">{entry.wallpaper.title || "Untitled wallpaper"}</p>
                  <p className="text-[13px] text-muted">
                    <time dateTime={entry.appliedAt} title={formatDateTime(entry.appliedAt)}>
                      {formatRelative(entry.appliedAt, now)}
                    </time>
                  </p>
                </div>
                <Button
                  size="sm"
                  icon={<RotateCcw className="size-4" />}
                  busy={reapplying(entry)}
                  disabled={apply.isPending && !reapplying(entry)}
                  onClick={() => apply.mutate(entry.wallpaper)}
                >
                  Apply again
                </Button>
              </li>
            ))}
          </ol>
          <div ref={sentinel} className="flex min-h-12 items-center pt-6 text-muted">
            {isFetchingNextPage && (
              <span className="flex items-center gap-2">
                <Spinner className="size-4" />
                Loading older entries
              </span>
            )}
          </div>
        </>
      )}
    </>
  );
}
