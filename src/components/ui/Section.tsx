import type { ReactNode } from "react";
import { Link } from "react-router-dom";

interface SectionProps {
  title: string;
  /** Link to the page that lists everything in this section. */
  moreTo?: string;
  moreLabel?: string;
  children: ReactNode;
}

export function Section({ title, moreTo, moreLabel = "See all", children }: SectionProps) {
  return (
    <section className="mt-12">
      <div className="mb-4 flex items-baseline justify-between gap-4">
        <h2 className="text-lg font-semibold">{title}</h2>
        {moreTo && (
          <Link to={moreTo} className="rounded text-[13px] text-muted hover:text-accent">
            {moreLabel}
          </Link>
        )}
      </div>
      {children}
    </section>
  );
}
