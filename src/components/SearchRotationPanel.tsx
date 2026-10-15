import { Play, Square } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";

import { useCollections } from "../hooks/useCollections";
import { useNow } from "../hooks/useNow";
import { useRotationStatus, useStartSearchRotation, useStopRotation } from "../hooks/useRotation";
import { cx } from "../services/cx";
import { describeInterval, describeNextChange, pluralize } from "../services/format";
import type { Wallpaper } from "../types";
import { IntervalPicker } from "./IntervalPicker";
import { Button } from "./ui/Button";
import { Switch } from "./ui/Switch";

/** Longest collection name the backend accepts. */
const MAX_COLLECTION_NAME = 60;

/** Most results the backend saves in one request. */
const MAX_WALLPAPERS = 500;

/** Thirty minutes, in seconds. */
const DEFAULT_INTERVAL = 1800;

/** Search results are saved to a collection named after the query. */
function collectionNameFor(query: string): string {
  return Array.from(query.trim()).slice(0, MAX_COLLECTION_NAME).join("").trim();
}

/** The case folding of SQLite's NOCASE collation, which collection names use. */
function foldName(name: string): string {
  return name.replace(/[A-Z]/g, (letter) => letter.toLowerCase());
}

interface SearchRotationPanelProps {
  query: string;
  /** The results loaded so far, in the order they are shown. */
  wallpapers: Wallpaper[];
}

/**
 * Rotates the wallpapers a search found. The results are saved to a
 * collection named after the query, which the scheduler then rotates.
 */
export function SearchRotationPanel({ query, wallpapers }: SearchRotationPanelProps) {
  const name = collectionNameFor(query);
  const status = useRotationStatus().data;
  const collections = useCollections().data;
  const start = useStartSearchRotation();
  const stop = useStopRotation();
  const now = useNow();

  const collection = collections?.find((c) => foldName(c.name) === foldName(name));
  const running = status?.active === true && collection !== undefined && status.collectionId === collection.id;

  // Until the user picks something, show the running rotation's settings.
  const [chosenInterval, setChosenInterval] = useState<number | null>(null);
  const [chosenRandom, setChosenRandom] = useState<boolean | null>(null);
  const intervalSeconds = chosenInterval ?? (running ? status.intervalSeconds : null) ?? DEFAULT_INTERVAL;
  const random = chosenRandom ?? (running && status.rotationMode === "random");

  const selection = wallpapers.slice(0, MAX_WALLPAPERS);
  const these = selection.length === 1 ? "this wallpaper" : `these ${selection.length} wallpapers`;

  let title: string;
  let summary: string;
  if (running) {
    title = `Rotating ${collection.name}`;
    const runningInterval = status.intervalSeconds ?? intervalSeconds;
    summary = `${pluralize(collection.wallpaperCount, "wallpaper")}, changing ${describeInterval(runningInterval)}.`;
    if (status.nextRunAt) summary += ` Next change ${describeNextChange(status.nextRunAt, now, runningInterval)}.`;
  } else {
    title = selection.length === 1 ? "Rotate this wallpaper" : "Rotate these wallpapers";
    summary = collection
      ? `Adds ${these} to your collection “${collection.name}”`
      : `Saves ${these} as a new collection “${name}”`;
    summary += ` and changes your wallpaper ${describeInterval(intervalSeconds)}, starting now.`;
    if (status?.active && status.collectionName) summary += ` This stops the rotation of ${status.collectionName}.`;
  }

  return (
    <section
      aria-labelledby="search-rotation-title"
      className={cx(
        "mb-6 flex flex-col gap-4 rounded-panel border p-5",
        running ? "border-accent/40 bg-accent/[0.06]" : "border-line bg-surface/70",
      )}
    >
      <div className="flex flex-wrap items-start justify-between gap-x-6 gap-y-3">
        <div className="min-w-0 flex-1 basis-80">
          <h2 id="search-rotation-title" className="font-semibold">
            {title}
          </h2>
          <p className="mt-0.5 text-[13px] text-muted" aria-live="polite">
            {summary}
            {running && (
              <>
                {" "}
                <Link to={`/collections/${collection.id}`} className="rounded text-accent hover:underline">
                  View collection
                </Link>
              </>
            )}
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          {running && (
            <Button icon={<Square className="size-3.5" />} busy={stop.isPending} onClick={() => stop.mutate()}>
              Stop
            </Button>
          )}
          <Button
            variant="primary"
            icon={<Play className="size-4" />}
            busy={start.isPending}
            disabled={selection.length === 0 || name.length === 0}
            onClick={() =>
              start.mutate({
                name,
                wallpapers: selection,
                intervalSeconds,
                rotationMode: random ? "random" : "sequential",
              })
            }
          >
            {running ? "Update rotation" : "Start rotation"}
          </Button>
        </div>
      </div>

      <div className="flex flex-wrap items-start gap-x-8 gap-y-3">
        <IntervalPicker label="Change wallpaper" value={intervalSeconds} onChange={setChosenInterval} />
        <div className="flex h-10 items-center">
          <Switch label="Random order" checked={random} onChange={setChosenRandom} />
        </div>
      </div>

      {running && status.lastError && (
        <p className="text-[13px] text-danger">
          The last change failed: {status.lastError} Kankulirchi will try again shortly.
        </p>
      )}
    </section>
  );
}
