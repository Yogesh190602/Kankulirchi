import { useRef, type KeyboardEvent } from "react";

import { cx } from "../../services/cx";

interface Option<T> {
  value: T;
  label: string;
}

interface SegmentedProps<T> {
  label: string;
  options: readonly Option<T>[];
  value: T;
  onChange: (value: T) => void;
  disabled?: boolean;
}

/** Single-choice control rendered as a row of segments (a radio group). */
export function Segmented<T extends string | number>({ label, options, value, onChange, disabled }: SegmentedProps<T>) {
  const buttons = useRef<Array<HTMLButtonElement | null>>([]);
  const selectedIndex = Math.max(
    0,
    options.findIndex((option) => option.value === value),
  );

  function onKeyDown(event: KeyboardEvent) {
    const step = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    const next = (selectedIndex + step + options.length) % options.length;
    const option = options[next];
    if (option) {
      onChange(option.value);
      buttons.current[next]?.focus();
    }
  }

  return (
    <div
      role="radiogroup"
      aria-label={label}
      onKeyDown={onKeyDown}
      className={cx("inline-flex w-fit flex-wrap rounded-lg border border-line bg-canvas/60 p-1", disabled && "opacity-50")}
    >
      {options.map((option, index) => {
        const selected = index === selectedIndex;
        return (
          <button
            key={String(option.value)}
            ref={(node) => {
              buttons.current[index] = node;
            }}
            type="button"
            role="radio"
            aria-checked={selected}
            tabIndex={selected ? 0 : -1}
            disabled={disabled}
            onClick={() => onChange(option.value)}
            className={cx(
              "h-8 rounded-md px-3 text-[13px] font-medium whitespace-nowrap transition-colors",
              selected ? "bg-raised text-fg shadow-[inset_0_0_0_1px_var(--color-line)]" : "text-muted hover:text-fg",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
