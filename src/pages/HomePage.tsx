import { Heart, Images, Library } from "lucide-react";
import { useState } from "react";
import { useNavigate } from "react-router-dom";

import { CollectionCard } from "../components/CollectionCard";
import { SearchBar } from "../components/SearchBar";
import { EmptyState } from "../components/ui/EmptyState";
import { Section } from "../components/ui/Section";
import { WallpaperGrid } from "../components/WallpaperGrid";
import { useCollections } from "../hooks/useCollections";
import { useFavorites } from "../hooks/useFavorites";
import { useRecentWallpapers } from "../hooks/useHistory";
import { useNow } from "../hooks/useNow";
import { useRotationStatus } from "../hooks/useRotation";
import { describeInterval, describeNextChange, formatRelative } from "../services/format";
import { fullImageSrc, wallpaperKey } from "../services/wallpapers";
import { useDetailStore } from "../store/ui";
import type { HistoryEntry, RotationStatus } from "../types";

const RECENT_COUNT = 8;
const FAVORITES_COUNT = 8;
const COLLECTIONS_COUNT = 4;

export function HomePage() {
  const navigate = useNavigate();
  const recent = useRecentWallpapers(RECENT_COUNT + 1);
  const favorites = useFavorites();
  const collections = useCollections();
  const status = useRotationStatus().data;

  const [current, ...earlier] = recent.data ?? [];
  const search = (query: string) => navigate(`/search?q=${encodeURIComponent(query)}`);

  return (
    <>
      {recent.isPending ? (
        <div className="aspect-[21/9] max-h-[440px] w-full rounded-panel bg-raised/60" />
      ) : current ? (
        <>
          <DesktopHero entry={current} status={status} />
          <div className="mt-8">
            <SearchBar size="lg" onSearch={search} />
          </div>
        </>
      ) : (
        <section className="rounded-panel border border-line bg-surface/70 px-10 py-12">
          <h1 className="font-condensed text-display font-semibold">Find a wallpaper</h1>
          <p className="mt-3 mb-8 max-w-[56ch] text-base text-muted">
            Search Wallhaven and Pexels, apply what you like, and let Kankulirchi rotate your collections on the GNOME
            desktop.
          </p>
          <SearchBar size="lg" onSearch={search} autoFocus />
        </section>
      )}

      <Section title="Recent wallpapers" moreTo="/history" moreLabel="See history">
        {earlier.length > 0 ? (
          <WallpaperGrid label="Recent wallpapers" wallpapers={earlier.map((entry) => entry.wallpaper)} />
        ) : (
          <EmptyState icon={<Images />} title={current ? "No earlier wallpapers" : "Nothing applied yet"}>
            {current
              ? "Wallpapers you applied before the current one show up here."
              : "Wallpapers you apply show up here, so you can go back to one later."}
          </EmptyState>
        )}
      </Section>

      <Section title="Favorites" moreTo="/favorites">
        {favorites.data && favorites.data.length > 0 ? (
          <WallpaperGrid label="Favorites" wallpapers={favorites.data.slice(0, FAVORITES_COUNT)} />
        ) : (
          <EmptyState icon={<Heart />} title="No favorites yet">
            Open any wallpaper and choose Favorite to keep it here.
          </EmptyState>
        )}
      </Section>

      <Section title="Collections" moreTo="/collections">
        {collections.data && collections.data.length > 0 ? (
          <ul className="grid grid-cols-[repeat(auto-fill,minmax(240px,1fr))] gap-4">
            {collections.data.slice(0, COLLECTIONS_COUNT).map((collection) => (
              <CollectionCard
                key={collection.id}
                collection={collection}
                rotating={status?.active === true && status.collectionId === collection.id}
              />
            ))}
          </ul>
        ) : (
          <EmptyState icon={<Library />} title="No collections yet">
            Collections group wallpapers so Kankulirchi can rotate them on your desktop.
          </EmptyState>
        )}
      </Section>
    </>
  );
}

interface DesktopHeroProps {
  entry: HistoryEntry;
  status: RotationStatus | undefined;
}

/** The wallpaper on the desktop right now, with the rotation status. */
function DesktopHero({ entry, status }: DesktopHeroProps) {
  const openDetail = useDetailStore((state) => state.open);
  const now = useNow();
  const { wallpaper } = entry;
  let rotation = "Rotation is off.";
  if (status?.active && status.intervalSeconds) {
    rotation = `Rotating ${status.collectionName} ${describeInterval(status.intervalSeconds)}`;
    rotation += status.nextRunAt
      ? `, next change ${describeNextChange(status.nextRunAt, now, status.intervalSeconds)}.`
      : ".";
  }

  return (
    <section aria-labelledby="desktop-title">
      <button
        type="button"
        onClick={() => openDetail(wallpaper)}
        aria-label={`Open ${wallpaper.title}`}
        className="block aspect-[21/9] max-h-[440px] w-full overflow-hidden rounded-panel bg-raised"
      >
        <HeroImage
          key={wallpaperKey(wallpaper)}
          sources={[fullImageSrc(wallpaper), wallpaper.imageUrl, wallpaper.thumbnailUrl]}
        />
      </button>
      <p className="mt-6 text-muted">On your desktop</p>
      <h1 id="desktop-title" className="mt-1 font-condensed text-display font-semibold break-words">
        {wallpaper.title || "Untitled wallpaper"}
      </h1>
      <p className="mt-3 text-base text-muted">
        Applied {formatRelative(entry.appliedAt, now)}. {rotation}
      </p>
    </section>
  );
}

/** Shows the first source that loads: cached file, original, then thumbnail. */
function HeroImage({ sources }: { sources: string[] }) {
  const [index, setIndex] = useState(0);
  return (
    <img
      src={sources[index]}
      alt=""
      draggable={false}
      onError={() => setIndex((current) => Math.min(current + 1, sources.length - 1))}
      className="h-full w-full object-cover"
    />
  );
}
