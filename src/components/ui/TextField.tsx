import { forwardRef, type InputHTMLAttributes } from "react";

import { cx } from "../../services/cx";

export const TextField = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function TextField(
  { className, ...rest },
  ref,
) {
  return (
    <input
      ref={ref}
      className={cx(
        "h-9 min-w-0 rounded-lg border border-line bg-canvas/70 px-3 text-sm text-fg placeholder:text-muted/70",
        "transition-colors focus:border-accent focus:outline-none",
        className,
      )}
      {...rest}
    />
  );
});
