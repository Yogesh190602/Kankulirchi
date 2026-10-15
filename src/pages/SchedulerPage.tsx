import { Library, Plus } from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { RotationControls } from "../components/RotationControls";
import { Button } from "../components/ui/Button";
import { EmptyState } from "../components/ui/EmptyState";
import { PageHeader } from "../components/ui/PageHeader";
import { LoadingBlock } from "../components/ui/Spinner";
import { useCollections } from "../hooks/useCollections";
import { useNow } from "../hooks/useNow";
import { useRotationStatus } from "../hooks/useRotation";
import { cx } from "../services/cx";
import { describeInterval, describeNextChange, formatRelative, pluralize } from "../services/format";

export function SchedulerPage() {
  const navigate = useNavigate();
  const collections = useCollections();
  const status = useRotationStatus().data;
  const now = useNow();
  const [selectedId, setSelectedId] = useState<number | null>(null);

  const list = collections.data ?? [];
  const selected =
    list.find((collection) => collection.id === selectedId) ??
    list.find((collection) => collection.id === status?.collectionId) ??
    list[0];

  return (
    <>
      <PageHeader
        title="Scheduler"
        description="Rotate one collection on your GNOME desktop. Wallpapers change while Kankulirchi is open, and rotation resumes when you start it again."
      />

      <section
        aria-label="Rotation status"
        className={cx(
          "mb-8 rounded-panel border p-6",
          status?.active ? "border-accent/40 bg-accent/[0.06]" : "border-line bg-surface/70",
        )}
      >
        {status?.active && status.intervalSeconds ? (
          <>
            <p className="font-condensed text-2xl font-semibold">Rotating {status.collectionName}</p>
            <p className="mt-1 text-muted">
              {`Changes ${describeInterval(status.intervalSeconds)} in ${status.rotationMode === "random" ? "random" : "collection"} order.`}
              {status.nextRunAt && ` Next change ${describeNextChange(status.nextRunAt, now, status.intervalSeconds)}.`}
              {status.lastAppliedAt && ` Last change ${formatRelative(status.lastAppliedAt, now)}.`}
            </p>
            {status.lastError && <p className="mt-2 text-danger">The last change failed: {status.lastError}</p>}
          </>
        ) : (
          <>
            <p className="font-condensed text-2xl font-semibold">Rotation is off</p>
            <p className="mt-1 text-muted">Choose a collection and an interval below, then start rotation.</p>
          </>
        )}
      </section>

      {collections.isPending ? (
        <LoadingBlock label="Loading collections" />
      ) : !selected ? (
        <EmptyState
          icon={<Library />}
          title="Create a collection first"
          action={
            <Button icon={<Plus className="size-4" />} onClick={() => navigate("/collections")}>
              Go to collections
            </Button>
          }
        >
          Rotation cycles through the wallpapers of one collection. You can also search for wallpapers and rotate
          the results.
        </EmptyState>
      ) : (
        <section aria-label="Rotation settings" className="rounded-panel border border-line bg-surface/70 p-6">
          <div className="mb-6 flex flex-col gap-2">
            <label htmlFor="scheduler-collection" className="text-[13px] text-muted">
              Collection
            </label>
            <select
              id="scheduler-collection"
              value={selected.id}
              onChange={(event) => setSelectedId(Number(event.target.value))}
              className="h-10 w-full max-w-sm rounded-lg border border-line bg-canvas/70 px-3 text-sm text-fg focus:border-accent focus:outline-none"
            >
              {list.map((collection) => (
                <option key={collection.id} value={collection.id}>
                  {collection.name} ({pluralize(collection.wallpaperCount, "wallpaper")})
                </option>
              ))}
            </select>
          </div>
          <RotationControls collectionId={selected.id} wallpaperCount={selected.wallpaperCount} hideRunningStatus />
        </section>
      )}
    </>
  );
}
