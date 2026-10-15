import { LoaderCircle } from "lucide-react";

import { cx } from "../../services/cx";

export function Spinner({ className, label }: { className?: string; label?: string }) {
  return (
    <LoaderCircle
      className={cx("animate-spin", className ?? "size-5")}
      aria-hidden={label ? undefined : true}
      aria-label={label}
      role={label ? "status" : undefined}
    />
  );
}

export function LoadingBlock({ label = "Loading" }: { label?: string }) {
  return (
    <div className="flex items-center gap-3 py-10 text-muted">
      <Spinner />
      <span>{label}</span>
    </div>
  );
}
