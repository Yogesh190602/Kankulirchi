import { Heart, ImageOff } from "lucide-react";
import { useState, type ReactNode } from "react";

import { cx } from "../services/cx";
import type { Wallpaper } from "../types";

interface WallpaperCardProps {
  wallpaper: Wallpaper;
  onSelect: (wallpaper: Wallpaper) => void;
  /** Extra control shown over the corner of the tile, e.g. "Remove". */
  action?: ReactNode;
}

export function WallpaperCard({ wallpaper, onSelect, action }: WallpaperCardProps) {
  const [failed, setFailed] = useState(false);

  return (
    <li className="group relative">
      <button
        type="button"
        onClick={() => onSelect(wallpaper)}
        aria-label={`Open ${wallpaper.title || "wallpaper"}`}
        className="block aspect-[16/10] w-full overflow-hidden rounded-tile bg-raised"
      >
        {failed ? (
          <span className="flex h-full w-full items-center justify-center text-muted">
            <ImageOff className="size-5" aria-hidden />
          </span>
        ) : (
          <img
            src={wallpaper.thumbnailUrl}
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            onError={() => setFailed(true)}
            className="h-full w-full object-cover transition-[filter] group-hover:brightness-110"
          />
        )}
      </button>
      {wallpaper.credit.author && (
        <span className="pointer-events-none absolute inset-x-0 bottom-0 truncate rounded-b-tile bg-gradient-to-t from-black/70 to-transparent px-3 pt-6 pb-2 text-[12px] text-white opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
          Photo by {wallpaper.credit.author}
        </span>
      )}
      {wallpaper.isFavorite && (
        <span
          className="pointer-events-none absolute top-2 right-2 rounded-full bg-canvas/70 p-1.5"
          title="Favorite"
        >
          <Heart className="size-3.5 fill-danger text-danger" aria-label="Favorite" />
        </span>
      )}
      {action && (
        <div className={cx("absolute top-2 left-2 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100")}>
          {action}
        </div>
      )}
    </li>
  );
}
