import { ALL_PROVIDERS } from "../hooks/useWallpaperSearch";
import { cx } from "../services/cx";
import type { ProviderInfo } from "../types";

interface ProviderFilterProps {
  providers: ProviderInfo[];
  value: string;
  onChange: (value: string) => void;
}

/** Chooses which provider to search: all enabled ones or a single one. */
export function ProviderFilter({ providers, value, onChange }: ProviderFilterProps) {
  const options = [{ id: ALL_PROVIDERS, name: "All providers" }, ...providers.filter((provider) => provider.enabled)];

  return (
    <div role="radiogroup" aria-label="Provider" className="flex flex-wrap gap-2">
      {options.map((option) => {
        const selected = option.id === value;
        return (
          <button
            key={option.id}
            type="button"
            role="radio"
            aria-checked={selected}
            onClick={() => onChange(option.id)}
            className={cx(
              "h-8 rounded-full border px-3.5 text-[13px] font-medium transition-colors",
              selected ? "border-accent bg-accent/10 text-accent" : "border-line text-muted hover:text-fg",
            )}
          >
            {option.name}
          </button>
        );
      })}
    </div>
  );
}
