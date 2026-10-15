import { X } from "lucide-react";

import { cx } from "../../services/cx";
import { useToastStore } from "../../store/ui";

export function Toaster() {
  const toasts = useToastStore((state) => state.toasts);
  const dismiss = useToastStore((state) => state.dismiss);

  return (
    <div aria-live="polite" className="pointer-events-none fixed right-5 bottom-5 z-50 flex w-[min(380px,calc(100vw-40px))] flex-col gap-2">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          role={toast.tone === "error" ? "alert" : "status"}
          className={cx(
            "pointer-events-auto flex animate-rise items-start gap-3 rounded-lg border bg-surface px-4 py-3 shadow-[0_12px_32px_rgb(0_0_0/0.35)]",
            toast.tone === "error" ? "border-danger/40" : "border-line",
          )}
        >
          <p className={cx("min-w-0 flex-1 break-words", toast.tone === "error" ? "text-danger" : "text-fg")}>
            {toast.message}
          </p>
          <button
            type="button"
            aria-label="Dismiss"
            onClick={() => dismiss(toast.id)}
            className="-mr-1 rounded p-0.5 text-muted hover:text-fg"
          >
            <X className="size-4" />
          </button>
        </div>
      ))}
    </div>
  );
}
