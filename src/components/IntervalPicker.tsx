import { useId, useState } from "react";

import { INTERVAL_OPTIONS, MAX_INTERVAL_SECONDS, MIN_INTERVAL_SECONDS } from "../services/format";
import { Segmented } from "./ui/Segmented";
import { TextField } from "./ui/TextField";

type Choice = number | "custom";

const choices: { value: Choice; label: string }[] = [
  ...INTERVAL_OPTIONS.map((option) => ({ value: option.seconds, label: option.label })),
  { value: "custom", label: "Custom" },
];

const numberFormat = new Intl.NumberFormat();

function isPreset(seconds: number): boolean {
  return INTERVAL_OPTIONS.some((option) => option.seconds === seconds);
}

/** Typed seconds, or null unless they are a whole number in the accepted range. */
function parseSeconds(text: string): number | null {
  const trimmed = text.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const seconds = Number(trimmed);
  return seconds >= MIN_INTERVAL_SECONDS && seconds <= MAX_INTERVAL_SECONDS ? seconds : null;
}

interface IntervalPickerProps {
  label: string;
  /** The interval, in seconds. */
  value: number;
  /**
   * Called with a preset as soon as it is picked, and with typed seconds
   * once they are confirmed with Enter or by leaving the field, so a
   * half-typed number never takes effect.
   */
  onChange: (seconds: number) => void;
}

/** Preset rotation intervals, plus a field for any number of seconds. */
export function IntervalPicker({ label, value, onChange }: IntervalPickerProps) {
  const [customChosen, setCustomChosen] = useState(false);
  // What the user is typing; null shows the current value.
  const [draft, setDraft] = useState<string | null>(null);
  const inputId = useId();
  const hintId = useId();

  const custom = customChosen || !isPreset(value);
  const typed = draft === null ? value : parseSeconds(draft);

  function choose(choice: Choice) {
    setDraft(null);
    if (choice === "custom") {
      setCustomChosen(true);
      return;
    }
    setCustomChosen(false);
    onChange(choice);
  }

  function commit() {
    if (typed === null) return;
    setDraft(null);
    if (typed !== value) onChange(typed);
  }

  return (
    <div className="flex flex-col gap-3">
      <Segmented label={label} options={choices} value={custom ? "custom" : value} onChange={choose} />
      {custom && (
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
          <label htmlFor={inputId} className="text-[13px] text-muted">
            Seconds
          </label>
          <TextField
            id={inputId}
            inputMode="numeric"
            autoComplete="off"
            value={draft ?? String(value)}
            onChange={(event) => setDraft(event.target.value)}
            onBlur={commit}
            onKeyDown={(event) => {
              if (event.key === "Enter") commit();
              if (event.key === "Escape") setDraft(null);
            }}
            aria-invalid={typed === null}
            aria-describedby={hintId}
            className="w-28 tabular-nums"
          />
          <span id={hintId} className={typed === null ? "text-[13px] text-danger" : "text-[13px] text-muted"}>
            {typed === null
              ? `Enter a whole number from ${MIN_INTERVAL_SECONDS} to ${numberFormat.format(MAX_INTERVAL_SECONDS)}.`
              : `From ${MIN_INTERVAL_SECONDS} seconds to one day (${numberFormat.format(MAX_INTERVAL_SECONDS)}).`}
          </span>
        </div>
      )}
    </div>
  );
}
