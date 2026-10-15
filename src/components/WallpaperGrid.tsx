import type { ReactNode } from "react";

import { wallpaperKey } from "../services/wallpapers";
import { useDetailStore } from "../store/ui";
import type { Wallpaper } from "../types";
import { WallpaperCard } from "./WallpaperCard";

interface WallpaperGridProps {
  wallpapers: Wallpaper[];
  /** Optional per-tile control, such as removing it from a collection. */
  renderAction?: (wallpaper: Wallpaper) => ReactNode;
  label?: string;
}

export function WallpaperGrid({ wallpapers, renderAction, label }: WallpaperGridProps) {
  const openDetail = useDetailStore((state) => state.open);

  return (
    <ul aria-label={label} className="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-3">
      {wallpapers.map((wallpaper) => (
        <WallpaperCard
          key={wallpaperKey(wallpaper)}
          wallpaper={wallpaper}
          onSelect={openDetail}
          action={renderAction?.(wallpaper)}
        />
      ))}
    </ul>
  );
}
