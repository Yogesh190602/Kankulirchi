import { Library, Plus, Trash2 } from "lucide-react";
import { useState, type FormEvent } from "react";

import { CollectionCard } from "../components/CollectionCard";
import { Button, IconButton } from "../components/ui/Button";
import { EmptyState } from "../components/ui/EmptyState";
import { Notice } from "../components/ui/Notice";
import { PageHeader } from "../components/ui/PageHeader";
import { LoadingBlock } from "../components/ui/Spinner";
import { TextField } from "../components/ui/TextField";
import { useCollections, useCreateCollection, useDeleteCollection } from "../hooks/useCollections";
import { useRotationStatus } from "../hooks/useRotation";
import { confirmDeleteCollection } from "../services/dialogs";
import { errorMessage } from "../services/errors";
import type { Collection } from "../types";

export function CollectionsPage() {
  const collections = useCollections();
  const status = useRotationStatus().data;
  const remove = useDeleteCollection();
  const [creating, setCreating] = useState(false);

  async function deleteCollection(collection: Collection) {
    if (await confirmDeleteCollection(collection.name)) {
      remove.mutate({ id: collection.id, name: collection.name });
    }
  }

  return (
    <>
      <PageHeader
        title="Collections"
        description="Group wallpapers you like, then rotate a collection on your desktop."
        actions={
          !creating && (
            <Button variant="primary" icon={<Plus className="size-4" />} onClick={() => setCreating(true)}>
              New collection
            </Button>
          )
        }
      />

      {creating && <CreateCollectionForm onDone={() => setCreating(false)} />}

      {collections.isPending ? (
        <LoadingBlock label="Loading collections" />
      ) : collections.isError ? (
        <Notice>{errorMessage(collections.error)}</Notice>
      ) : collections.data.length === 0 ? (
        !creating && (
          <EmptyState
            icon={<Library />}
            title="No collections yet"
            action={
              <Button icon={<Plus className="size-4" />} onClick={() => setCreating(true)}>
                New collection
              </Button>
            }
          >
            Create one, then add wallpapers to it from search or a wallpaper’s details.
          </EmptyState>
        )
      ) : (
        <ul className="grid grid-cols-[repeat(auto-fill,minmax(260px,1fr))] gap-4">
          {collections.data.map((collection) => (
            <CollectionCard
              key={collection.id}
              collection={collection}
              rotating={status?.active === true && status.collectionId === collection.id}
              action={
                <IconButton
                  label={`Delete ${collection.name}`}
                  onClick={() => void deleteCollection(collection)}
                  className="size-8 bg-canvas/80 hover:text-danger"
                >
                  <Trash2 className="size-4" />
                </IconButton>
              }
            />
          ))}
        </ul>
      )}
    </>
  );
}

function CreateCollectionForm({ onDone }: { onDone: () => void }) {
  const create = useCreateCollection();
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  async function submit(event: FormEvent) {
    event.preventDefault();
    try {
      await create.mutateAsync(name);
      onDone();
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  return (
    <form onSubmit={submit} className="mb-8 flex max-w-xl flex-col gap-2 rounded-panel border border-line bg-surface/70 p-5">
      <label htmlFor="collection-name" className="font-medium">
        Name the collection
      </label>
      <div className="flex gap-2">
        <TextField
          id="collection-name"
          autoFocus
          value={name}
          maxLength={60}
          placeholder="Quiet mornings"
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => event.key === "Escape" && onDone()}
          className="flex-1"
        />
        <Button type="submit" variant="primary" busy={create.isPending} disabled={!name.trim()}>
          Create
        </Button>
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
      </div>
      {error && <p className="text-[13px] text-danger">{error}</p>}
    </form>
  );
}
