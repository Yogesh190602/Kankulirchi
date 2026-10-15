import { Search } from "lucide-react";
import { useState, type FormEvent } from "react";

import { cx } from "../services/cx";
import { Button } from "./ui/Button";

interface SearchBarProps {
  initialQuery?: string;
  onSearch: (query: string) => void;
  size?: "md" | "lg";
  autoFocus?: boolean;
}

export function SearchBar({ initialQuery = "", onSearch, size = "md", autoFocus }: SearchBarProps) {
  const [query, setQuery] = useState(initialQuery);

  function submit(event: FormEvent) {
    event.preventDefault();
    const trimmed = query.trim();
    if (trimmed) onSearch(trimmed);
  }

  return (
    <form role="search" onSubmit={submit} className="flex w-full max-w-2xl gap-2">
      <label className="relative flex min-w-0 flex-1 items-center">
        <span className="sr-only">Search wallpapers</span>
        <Search className="pointer-events-none absolute left-3 size-4 text-muted" aria-hidden />
        <input
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          placeholder="Mountains, rain, neon city…"
          autoFocus={autoFocus}
          maxLength={200}
          className={cx(
            "w-full min-w-0 rounded-lg border border-line bg-canvas/70 pr-3 pl-9 text-fg placeholder:text-muted/70",
            "transition-colors focus:border-accent focus:outline-none",
            size === "lg" ? "h-11 text-base" : "h-10 text-sm",
          )}
        />
      </label>
      <Button type="submit" variant="primary" className={size === "lg" ? "h-11 px-5" : "h-10"} disabled={!query.trim()}>
        Search
      </Button>
    </form>
  );
}
