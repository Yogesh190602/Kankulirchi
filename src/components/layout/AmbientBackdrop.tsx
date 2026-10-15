import { useState } from "react";

import { useRecentWallpapers } from "../../hooks/useHistory";

/**
 * Tints the window with the colors of the current desktop wallpaper: a
 * heavily blurred copy of its thumbnail sits behind the interface and
 * cross-fades whenever the wallpaper changes.
 */
export function AmbientBackdrop() {
  const recent = useRecentWallpapers(1);
  const url = recent.data?.[0]?.wallpaper.thumbnailUrl ?? null;
  const [current, setCurrent] = useState<string | null>(null);
  const [layers, setLayers] = useState<string[]>([]);

  if (url !== current) {
    setCurrent(url);
    setLayers((previous) => (url ? [...previous.slice(-1), url] : []));
  }

  return (
    <div aria-hidden className="pointer-events-none fixed inset-0 z-0 overflow-hidden">
      <div className="absolute inset-0 opacity-40">
        {layers.map((src, index) => (
          <div
            key={src}
            className="absolute -inset-[10%] animate-fade-in bg-cover bg-center"
            style={{ backgroundImage: `url("${encodeURI(src)}")`, filter: "blur(80px) saturate(1.35)" }}
            onAnimationEnd={() => {
              if (index === layers.length - 1) setLayers([src]);
            }}
          />
        ))}
      </div>
      <div className="absolute inset-0 bg-gradient-to-br from-canvas/30 via-canvas/70 to-canvas" />
    </div>
  );
}
