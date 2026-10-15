import type { ButtonHTMLAttributes, ReactNode } from "react";

import { cx } from "../../services/cx";
import { Spinner } from "./Spinner";

type Variant = "primary" | "secondary" | "ghost" | "danger";
type Size = "md" | "sm";

const variants: Record<Variant, string> = {
  primary: "bg-accent text-on-accent hover:bg-accent-strong",
  secondary: "border border-line bg-raised text-fg hover:border-muted/50",
  ghost: "text-muted hover:bg-raised hover:text-fg",
  danger: "border border-danger/40 text-danger hover:bg-danger/10",
};

const sizes: Record<Size, string> = {
  md: "h-9 px-3.5 text-sm",
  sm: "h-8 px-3 text-[13px]",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  icon?: ReactNode;
  /** Shows a spinner and blocks clicks while an action runs. */
  busy?: boolean;
}

export function Button({
  variant = "secondary",
  size = "md",
  icon,
  busy = false,
  disabled,
  className,
  children,
  ...rest
}: ButtonProps) {
  return (
    <button
      type="button"
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      className={cx(
        "inline-flex shrink-0 items-center justify-center gap-2 rounded-lg font-medium whitespace-nowrap transition-colors",
        "disabled:pointer-events-none disabled:opacity-50",
        variants[variant],
        sizes[size],
        className,
      )}
      {...rest}
    >
      {busy ? <Spinner className="size-4" /> : icon}
      {children}
    </button>
  );
}

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  children: ReactNode;
}

export function IconButton({ label, className, children, ...rest }: IconButtonProps) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className={cx(
        "inline-flex size-9 shrink-0 items-center justify-center rounded-lg text-muted transition-colors hover:bg-raised hover:text-fg",
        "disabled:pointer-events-none disabled:opacity-50",
        className,
      )}
      {...rest}
    >
      {children}
    </button>
  );
}
