import type { ReactNode } from "react";

interface EmptyStateProps {
  icon: ReactNode;
  title: string;
  children?: ReactNode;
  action?: ReactNode;
}

export function EmptyState({ icon, title, children, action }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-start gap-3 rounded-panel border border-dashed border-line px-6 py-10">
      <div className="text-muted [&>svg]:size-6">{icon}</div>
      <div>
        <p className="font-medium text-fg">{title}</p>
        {children && <p className="mt-1 max-w-[56ch] text-muted">{children}</p>}
      </div>
      {action}
    </div>
  );
}
