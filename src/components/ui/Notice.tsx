import { CircleAlert } from "lucide-react";
import type { ReactNode } from "react";

export function Notice({ children, action }: { children: ReactNode; action?: ReactNode }) {
  return (
    <div role="alert" className="flex flex-wrap items-center gap-3 rounded-lg border border-danger/30 bg-danger/5 px-4 py-3">
      <CircleAlert className="size-4 shrink-0 text-danger" aria-hidden />
      <p className="min-w-0 flex-1 text-fg">{children}</p>
      {action}
    </div>
  );
}
