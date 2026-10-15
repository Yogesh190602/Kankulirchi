import { Check, Download, Heart, Monitor, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { useProviders } from "../hooks/useProviders";
import {
  useApplyWallpaper,
  useDownloadWallpaper,
  useToggleFavorite,
  useWallpaperStatus,
} from "../hooks/useWallpaperActions";
import { cx } from "../services/cx";
import { formatResolution } from "../services/format";
import { fullImageSrc, wallpaperKey, withLibraryState } from "../services/wallpapers";
import { useDetailStore } from "../store/ui";
import type { Wallpaper } from "../types";
import { CollectionPicker } from "./CollectionPicker";
import { PalettePreview } from "./PalettePreview";
import { Button, IconButton } from "./ui/Button";
import { Spinner } from "./ui/Spinner";
import { LicenseName, WallpaperCredit } from "./WallpaperCredit";

/** Wallpaper detail: preview, download, favorite, collections and apply. */
export function WallpaperDetailDialog() {
  const wallpaper = useDetailStore((state) => state.wallpaper);
  const close = useDetailStore((state) => state.close);
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const element = dialog.current;
    if (!element) return;
    if (wallpaper && !element.open) element.showModal();
    if (!wallpaper && element.open) element.close();
  }, [wallpaper]);

  return (
    <dialog
      ref={dialog}
      aria-label={wallpaper?.title || "Wallpaper"}
      onClose={close}
      onClick={(event) => {
        // A click on the dialog element itself is a click on the backdrop.
        if (event.target === event.currentTarget) close();
      }}
      className={cx(
        "m-auto h-[min(880px,92vh)] max-h-none w-[min(1320px,94vw)] max-w-none overflow-hidden",
        "rounded-panel border border-line bg-surface p-0 text-fg shadow-[0_24px_80px_rgb(0_0_0/0.5)]",
        "open:animate-rise",
      )}
    >
      {wallpaper && <DetailContent key={wallpaperKey(wallpaper)} selected={wallpaper} onClose={close} />}
    </dialog>
  );
}

function DetailContent({ selected, onClose }: { selected: Wallpaper; onClose: () => void }) {
  const status = useWallpaperStatus(selected);
  const wallpaper = status.data?.wallpaper ? withLibraryState(selected, status.data.wallpaper) : selected;
  const collectionIds = status.data?.collectionIds ?? [];

  const providers = useProviders();
  const provider = providers.data?.find((provider) => provider.id === wallpaper.provider);
  const resolution = formatResolution(wallpaper.width, wallpaper.height);

  const apply = useApplyWallpaper();
  const download = useDownloadWallpaper();
  const favorite = useToggleFavorite();
  const downloaded = wallpaper.localPath !== null;

  return (
    <div className="grid h-full grid-cols-[minmax(0,1fr)_340px]">
      <Preview wallpaper={selected} />

      <aside className="flex min-h-0 flex-col overflow-y-auto border-l border-line p-6">
        <div className="flex items-start justify-between gap-3">
          <h2 data-selectable className="font-condensed text-2xl leading-tight font-semibold break-words">
            {wallpaper.title || "Untitled wallpaper"}
          </h2>
          <IconButton label="Close" onClick={onClose} className="-mt-1 -mr-2">
            <X className="size-5" />
          </IconButton>
        </div>
        <WallpaperCredit
          wallpaper={wallpaper}
          providerName={provider?.name ?? wallpaper.provider}
          providerWebsite={provider?.website ?? null}
        />
        {(resolution || wallpaper.credit.license) && (
          <p className="text-[13px] text-muted">
            {resolution}
            {resolution && wallpaper.credit.license && " · "}
            {wallpaper.credit.license && (
              <>
                License <LicenseName credit={wallpaper.credit} />
              </>
            )}
          </p>
        )}

        <div className="mt-6 flex flex-col gap-2">
          <Button
            variant="primary"
            icon={<Monitor className="size-4" />}
            busy={apply.isPending}
            onClick={() => apply.mutate(wallpaper)}
            className="h-10"
          >
            {apply.isPending ? "Applying…" : "Apply now"}
          </Button>
          <div className="grid grid-cols-2 gap-2">
            <Button
              aria-pressed={wallpaper.isFavorite}
              icon={<Heart className={cx("size-4", wallpaper.isFavorite && "fill-danger text-danger")} />}
              busy={favorite.isPending}
              onClick={() => favorite.mutate(wallpaper)}
            >
              {wallpaper.isFavorite ? "Favorited" : "Favorite"}
            </Button>
            <Button
              icon={downloaded ? <Check className="size-4 text-accent" /> : <Download className="size-4" />}
              busy={download.isPending}
              disabled={downloaded}
              onClick={() => download.mutate(wallpaper)}
            >
              {download.isPending ? "Downloading…" : downloaded ? "Downloaded" : "Download"}
            </Button>
          </div>
          {wallpaper.localPath && (
            <p data-selectable className="mt-1 text-[12px] break-all text-muted">
              Saved at {wallpaper.localPath}
            </p>
          )}
        </div>

        <PalettePreview wallpaper={wallpaper} />
        <CollectionPicker wallpaper={wallpaper} collectionIds={collectionIds} />
      </aside>
    </div>
  );
}

function Preview({ wallpaper }: { wallpaper: Wallpaper }) {
  const [src, setSrc] = useState(() => fullImageSrc(wallpaper));
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);

  return (
    <div className="relative min-h-0 bg-[#080d11]">
      <img
        src={wallpaper.thumbnailUrl}
        alt=""
        aria-hidden
        draggable={false}
        className="absolute inset-0 h-full w-full object-contain"
      />
      <img
        src={src}
        alt={wallpaper.title}
        draggable={false}
        onLoad={() => setLoaded(true)}
        onError={() => {
          // A cached file outside the allowed folders falls back to the original.
          if (src !== wallpaper.imageUrl) setSrc(wallpaper.imageUrl);
          else setFailed(true);
        }}
        className={cx(
          "absolute inset-0 h-full w-full object-contain transition-opacity duration-500",
          loaded ? "opacity-100" : "opacity-0",
        )}
      />
      {!loaded && (
        <div className="absolute top-4 left-4 flex items-center gap-2 rounded-full bg-canvas/80 px-3 py-1.5 text-[12px] text-muted">
          {failed ? (
            "The full image could not be loaded"
          ) : (
            <>
              <Spinner className="size-3.5" />
              Loading full image
            </>
          )}
        </div>
      )}
    </div>
  );
}
