import { useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { useState, type FormEvent } from "react";

import { useCollectionMembership, useCollections, useCreateCollection } from "../hooks/useCollections";
import { errorMessage } from "../services/errors";
import { queryKeys } from "../services/queryKeys";
import type { Wallpaper, WallpaperStatus } from "../types";
import { Button } from "./ui/Button";
import { TextField } from "./ui/TextField";

interface CollectionPickerProps {
  wallpaper: Wallpaper;
  collectionIds: number[];
}

/** Checklist of collections containing the wallpaper, with inline creation. */
export function CollectionPicker({ wallpaper, collectionIds }: CollectionPickerProps) {
  const queryClient = useQueryClient();
  const collections = useCollections();
  const membership = useCollectionMembership();
  const create = useCreateCollection();
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  function setIncluded(collectionId: number, include: boolean) {
    // Show the change right away; the status query refetches afterwards.
    queryClient.setQueryData<WallpaperStatus>(
      queryKeys.wallpaperStatus(wallpaper.provider, wallpaper.providerId),
      (old) => {
        const ids = old?.collectionIds ?? [];
        return {
          wallpaper: old?.wallpaper ?? null,
          collectionIds: include ? [...ids, collectionId] : ids.filter((id) => id !== collectionId),
        };
      },
    );
    membership.mutate({ collectionId, wallpaper, include });
  }

  async function submit(event: FormEvent) {
    event.preventDefault();
    try {
      const collection = await create.mutateAsync(name);
      setIncluded(collection.id, true);
      setName("");
      setCreating(false);
      setError(null);
    } catch (cause) {
      setError(errorMessage(cause));
    }
  }

  const list = collections.data ?? [];

  return (
    <section className="mt-8 border-t border-line pt-6" aria-labelledby="collection-picker-title">
      <h3 id="collection-picker-title" className="font-medium">
        Add to collection
      </h3>
      {list.length === 0 && !creating && (
        <p className="mt-1 text-[13px] text-muted">Collections group wallpapers for rotation.</p>
      )}
      {list.length > 0 && (
        <ul className="mt-2 -mx-2 flex flex-col">
          {list.map((collection) => {
            const included = collectionIds.includes(collection.id);
            return (
              <li key={collection.id}>
                <label className="flex h-9 items-center gap-3 rounded-md px-2 hover:bg-raised">
                  <input
                    type="checkbox"
                    checked={included}
                    onChange={() => setIncluded(collection.id, !included)}
                    className="size-4 accent-[var(--color-accent)]"
                  />
                  <span className="min-w-0 truncate">{collection.name}</span>
                </label>
              </li>
            );
          })}
        </ul>
      )}

      {creating ? (
        <form onSubmit={submit} className="mt-3 flex flex-col gap-2">
          <label className="sr-only" htmlFor="new-collection-name">
            Collection name
          </label>
          <div className="flex gap-2">
            <TextField
              id="new-collection-name"
              autoFocus
              value={name}
              maxLength={60}
              placeholder="Collection name"
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Escape") {
                  event.stopPropagation();
                  event.preventDefault();
                  setCreating(false);
                }
              }}
              className="flex-1"
            />
            <Button type="submit" variant="primary" busy={create.isPending} disabled={!name.trim()}>
              Create
            </Button>
          </div>
          {error && <p className="text-[13px] text-danger">{error}</p>}
        </form>
      ) : (
        <Button variant="ghost" size="sm" icon={<Plus className="size-4" />} onClick={() => setCreating(true)} className="mt-2 -ml-3">
          New collection
        </Button>
      )}
    </section>
  );
}
