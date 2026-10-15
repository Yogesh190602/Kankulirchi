import { Play, Square } from "lucide-react";
import { useState } from "react";

import { useNow } from "../hooks/useNow";
import {
  useRotationSettings,
  useRotationStatus,
  useSaveRotationSettings,
  useStartRotation,
  useStopRotation,
} from "../hooks/useRotation";
import { describeInterval, describeNextChange } from "../services/format";
import type { RotationMode, RotationSettings } from "../types";
import { IntervalPicker } from "./IntervalPicker";
import { Button } from "./ui/Button";
import { LoadingBlock } from "./ui/Spinner";
import { Switch } from "./ui/Switch";

/** Thirty minutes, in seconds. */
const DEFAULT_INTERVAL = 1800;

interface RotationControlsProps {
  collectionId: number;
  wallpaperCount: number;
  /** Hide the "changes every…" line when the page already shows the status. */
  hideRunningStatus?: boolean;
}

/** Interval, order and start/stop controls for one collection's rotation. */
export function RotationControls({ collectionId, wallpaperCount, hideRunningStatus = false }: RotationControlsProps) {
  const settings = useRotationSettings(collectionId);
  if (settings.isPending) return <LoadingBlock label="Loading rotation settings" />;
  return (
    <RotationForm
      key={collectionId}
      collectionId={collectionId}
      wallpaperCount={wallpaperCount}
      hideRunningStatus={hideRunningStatus}
      saved={settings.data ?? null}
    />
  );
}

interface RotationFormProps extends RotationControlsProps {
  saved: RotationSettings | null;
}

function RotationForm({ collectionId, wallpaperCount, hideRunningStatus, saved }: RotationFormProps) {
  const [intervalSeconds, setIntervalSeconds] = useState(saved?.intervalSeconds ?? DEFAULT_INTERVAL);
  const [random, setRandom] = useState(saved?.rotationMode === "random");
  const status = useRotationStatus().data;
  const save = useSaveRotationSettings();
  const start = useStartRotation();
  const stop = useStopRotation();
  const now = useNow();

  const running = status?.active === true && status.collectionId === collectionId;
  const otherRotation = status?.active && status.collectionId !== collectionId ? status.collectionName : null;
  const modeFor = (isRandom: boolean): RotationMode => (isRandom ? "random" : "sequential");

  function persist(nextInterval: number, nextRandom: boolean) {
    save.mutate({ collectionId, intervalSeconds: nextInterval, rotationMode: modeFor(nextRandom) });
  }

  let summary: string | null;
  if (running && hideRunningStatus) {
    summary = null;
  } else if (running) {
    summary = `Changes ${describeInterval(intervalSeconds)}.`;
    if (status?.nextRunAt) summary += ` Next change ${describeNextChange(status.nextRunAt, now, intervalSeconds)}.`;
  } else if (wallpaperCount === 0) {
    summary = "Add wallpapers to this collection to rotate them.";
  } else if (otherRotation) {
    summary = `Starting stops the rotation of ${otherRotation}.`;
  } else {
    summary = "Rotation is off.";
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-2">
        <span className="text-[13px] text-muted">
          Change wallpaper
        </span>
        <IntervalPicker
          label="Change wallpaper"
          value={intervalSeconds}
          onChange={(value) => {
            setIntervalSeconds(value);
            persist(value, random);
          }}
        />
      </div>

      <Switch
        label="Random order"
        description={
          random
            ? "Picks any wallpaper in the collection, never the one on screen."
            : "Goes through the collection in the order wallpapers were added."
        }
        checked={random}
        onChange={(value) => {
          setRandom(value);
          persist(intervalSeconds, value);
        }}
      />

      <div className="flex flex-col gap-3">
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="primary"
            icon={<Play className="size-4" />}
            busy={start.isPending}
            disabled={running || wallpaperCount === 0}
            onClick={() => start.mutate({ collectionId, intervalSeconds, rotationMode: modeFor(random) })}
          >
            Start rotation
          </Button>
          <Button
            icon={<Square className="size-3.5" />}
            busy={stop.isPending}
            disabled={!running}
            onClick={() => stop.mutate()}
          >
            Stop rotation
          </Button>
        </div>
        {summary && (
          <p className={running ? "text-fg" : "text-muted"} aria-live="polite">
            {summary}
          </p>
        )}
        {running && !hideRunningStatus && status?.lastError && (
          <p className="text-danger">The last change failed: {status.lastError} Kankulirchi will try again shortly.</p>
        )}
      </div>
    </div>
  );
}
