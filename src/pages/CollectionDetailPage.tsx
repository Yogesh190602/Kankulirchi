import { ArrowLeft, Images, Pencil, Search, Trash2, X } from "lucide-react";
import { useState, type FormEvent } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router-dom";

import { RotationControls } from "../components/RotationControls";
import { Button, IconButton } from "../components/ui/Button";
import { EmptyState } from "../components/ui/EmptyState";
import { PageHeader } from "../components/ui/PageHeader";
import { LoadingBlock } from "../components/ui/Spinner";
import { TextField } from "../components/ui/TextField";
import { WallpaperGrid } from "../components/WallpaperGrid";
import {
  useCollection,
  useCollectionMembership,
  useDeleteCollection,
  useRenameCollection,
} from "../hooks/useCollections";
import { confirmDeleteCollection } from "../services/dialogs";
import { errorMessage } from "../services/errors";
import { pluralize } from "../services/format";
import type { Collection } from "../types";

const backLink = (
  <Link to="/collections" className="mb-4 inline-flex items-center gap-1.5 rounded text-[13px] text-muted hover:text-fg">
    <ArrowLeft className="size-4" aria-hidden />
    Collections
  </Link>
);

export function CollectionDetailPage() {
  const { id } = useParams();
  const collectionId = Number(id);
  if (!Number.isSafeInteger(collectionId)) return <Navigate to="/collections" replace />;
  return <CollectionDetail key={collectionId} collectionId={collectionId} />;
}

function CollectionDetail({ collectionId }: { collectionId: number }) {
  const navigate = useNavigate();
  const detail = useCollection(collectionId);
  const remove = useDeleteCollection();
  const membership = useCollectionMembership();
  const [renaming, setRenaming] = useState(false);

  if (detail.isPending) return <LoadingBlock label="Loading collection" />;
  if (detail.isError) {
    return (
      <>
        {backLink}
        <EmptyState icon={<Images />} title="This collection is not available">
          {errorMessage(detail.error)}
        </EmptyState>
      </>
    );
  }

  const { collection, wallpapers } = detail.data;

  async function deleteCollection() {
    if (await confirmDeleteCollection(collection.name)) {
      remove.mutate({ id: collection.id, name: collection.name }, { onSuccess: () => navigate("/collections") });
    }
  }

  return (
    <>
      <PageHeader
        before={backLink}
        title={renaming ? <RenameForm collection={collection} onDone={() => setRenaming(false)} /> : collection.name}
        description={pluralize(collection.wallpaperCount, "wallpaper")}
        actions={
          !renaming && (
            <>
              <Button icon={<Pencil className="size-4" />} onClick={() => setRenaming(true)}>
                Rename
              </Button>
              <Button variant="danger" icon={<Trash2 className="size-4" />} busy={remove.isPending} onClick={() => void deleteCollection()}>
                Delete
              </Button>
            </>
          )
        }
      />

      <section aria-labelledby="rotation-title" className="mb-10 rounded-panel border border-line bg-surface/70 p-6">
        <h2 id="rotation-title" className="mb-5 text-lg font-semibold">
          Rotation
        </h2>
        <RotationControls collectionId={collection.id} wallpaperCount={collection.wallpaperCount} />
      </section>

      {wallpapers.length === 0 ? (
        <EmptyState
          icon={<Images />}
          title="This collection is empty"
          action={
            <Button icon={<Search className="size-4" />} onClick={() => navigate("/search")}>
              Search wallpapers
            </Button>
          }
        >
          Open a wallpaper from search, favorites or history and add it to {collection.name}.
        </EmptyState>
      ) : (
        <WallpaperGrid
          label={`Wallpapers in ${collection.name}`}
          wallpapers={wallpapers}
          renderAction={(wallpaper) => (
            <IconButton
              label={`Remove from ${collection.name}`}
              onClick={() => membership.mutate({ collectionId: collection.id, wallpaper, include: false })}
              className="size-8 bg-canvas/80 hover:text-danger"
            >
              <X className="size-4" />
            </IconButton>
          )}
        />
      )}
    </>
  );
}

function RenameForm({ collection, onDone }: { collection: Collection; onDone: () => void }) {
  const rename = useRenameCollection();
  const [name, setName] = useState(collection.name);
  const [error, setError] = useState<string | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (name.trim() === collection.name) return onDone();
    try {
      await rename.mutateAsync({ id: collection.id, name });
      onDone();
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  return (
    <form onSubmit={submit} className="flex flex-col gap-2 font-sans text-sm font-normal">
      <label htmlFor="rename-collection" className="sr-only">
        Collection name
      </label>
      <div className="flex flex-wrap gap-2">
        <TextField
          id="rename-collection"
          autoFocus
          value={name}
          maxLength={60}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => event.key === "Escape" && onDone()}
          className="h-11 w-[min(420px,60vw)] text-lg"
        />
        <Button type="submit" variant="primary" className="h-11" busy={rename.isPending} disabled={!name.trim()}>
          Save name
        </Button>
        <Button variant="ghost" className="h-11" onClick={onDone}>
          Cancel
        </Button>
      </div>
      {error && <p className="text-[13px] text-danger">{error}</p>}
    </form>
  );
}
