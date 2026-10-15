import { Heart, Search } from "lucide-react";
import { useNavigate } from "react-router-dom";

import { Button } from "../components/ui/Button";
import { EmptyState } from "../components/ui/EmptyState";
import { Notice } from "../components/ui/Notice";
import { PageHeader } from "../components/ui/PageHeader";
import { LoadingBlock } from "../components/ui/Spinner";
import { WallpaperGrid } from "../components/WallpaperGrid";
import { useFavorites } from "../hooks/useFavorites";
import { errorMessage } from "../services/errors";
import { pluralize } from "../services/format";

export function FavoritesPage() {
  const navigate = useNavigate();
  const favorites = useFavorites();

  return (
    <>
      <PageHeader
        title="Favorites"
        description={favorites.data?.length ? pluralize(favorites.data.length, "favorite") : undefined}
      />
      {favorites.isPending ? (
        <LoadingBlock label="Loading favorites" />
      ) : favorites.isError ? (
        <Notice>{errorMessage(favorites.error)}</Notice>
      ) : favorites.data.length === 0 ? (
        <EmptyState
          icon={<Heart />}
          title="No favorites yet"
          action={
            <Button icon={<Search className="size-4" />} onClick={() => navigate("/search")}>
              Search wallpapers
            </Button>
          }
        >
          Open any wallpaper and choose Favorite to keep it here.
        </EmptyState>
      ) : (
        <WallpaperGrid label="Favorites" wallpapers={favorites.data} />
      )}
    </>
  );
}
