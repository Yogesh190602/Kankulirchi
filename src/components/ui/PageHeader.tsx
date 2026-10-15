import type { ReactNode } from "react";

interface PageHeaderProps {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
  before?: ReactNode;
}

export function PageHeader({ title, description, actions, before }: PageHeaderProps) {
  return (
    <header className="mb-8">
      {before}
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div className="min-w-0">
          <h1 className="font-condensed text-title font-semibold text-fg">{title}</h1>
          {description && <p className="mt-2 max-w-[62ch] text-muted">{description}</p>}
        </div>
        {actions && <div className="flex flex-wrap items-center gap-2">{actions}</div>}
      </div>
    </header>
  );
}
