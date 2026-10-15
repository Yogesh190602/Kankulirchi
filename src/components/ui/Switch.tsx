import { cx } from "../../services/cx";

interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  /** Hides the label visually; it is still announced. */
  hideLabel?: boolean;
  disabled?: boolean;
  description?: string;
}

export function Switch({ checked, onChange, label, hideLabel = false, disabled = false, description }: SwitchProps) {
  return (
    <label className={cx("inline-flex items-start gap-3", disabled && "opacity-50")}>
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
        className={cx(
          "relative mt-0.5 inline-flex h-6 w-10 shrink-0 items-center rounded-full border transition-colors",
          checked ? "border-accent bg-accent" : "border-line bg-raised",
        )}
      >
        <span
          className={cx(
            "inline-block size-4 rounded-full transition-transform",
            checked ? "translate-x-5 bg-on-accent" : "translate-x-1 bg-muted",
          )}
        />
      </button>
      <span className={cx("flex flex-col", hideLabel && "sr-only")}>
        <span className="text-fg">{label}</span>
        {description && <span className="text-[13px] text-muted">{description}</span>}
      </span>
    </label>
  );
}
