import { Images } from "lucide-react";
import type { ReactNode } from "react";
import { Link } from "react-router-dom";

import { pluralize } from "../services/format";
import type { Collection } from "../types";

interface CollectionCardProps {
  collection: Collection;
  rotating?: boolean;
  /** Control shown over the top-right corner on hover or focus. */
  action?: ReactNode;
}

export function CollectionCard({ collection, rotating = false, action }: CollectionCardProps) {
  const tiles = collection.previewThumbnails.slice(0, 4);

  return (
    <li className="group relative">
      <Link
        to={`/collections/${collection.id}`}
        className="group block rounded-panel border border-line bg-surface/70 p-2 transition-colors hover:border-muted/40"
      >
        <div className="grid aspect-[16/10] grid-cols-2 grid-rows-2 gap-1 overflow-hidden rounded-tile bg-raised">
          {tiles.length === 0 ? (
            <div className="col-span-2 row-span-2 flex items-center justify-center text-muted">
              <Images className="size-6" aria-hidden />
            </div>
          ) : (
            tiles.map((src, index) => (
              <img
                key={`${src}-${index}`}
                src={src}
                alt=""
                loading="lazy"
                draggable={false}
                className={
                  tiles.length === 1
                    ? "col-span-2 row-span-2 h-full w-full object-cover"
                    : tiles.length === 2
                      ? "row-span-2 h-full w-full object-cover"
                      : tiles.length === 3 && index === 0
                        ? "row-span-2 h-full w-full object-cover"
                        : "h-full w-full object-cover"
                }
              />
            ))
          )}
        </div>
        <div className="flex items-baseline justify-between gap-3 px-1.5 pt-3 pb-1">
          <span className="truncate font-medium text-fg">{collection.name}</span>
          <span className="shrink-0 text-[13px] text-muted">
            {rotating ? <span className="text-accent">Rotating</span> : pluralize(collection.wallpaperCount, "wallpaper")}
          </span>
        </div>
      </Link>
      {action && (
        <div className="absolute top-4 right-4 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
          {action}
        </div>
      )}
    </li>
  );
}
