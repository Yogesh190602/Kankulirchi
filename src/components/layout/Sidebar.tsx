import { Heart, History, House, Library, Power, Search, Settings, Timer, type LucideIcon } from "lucide-react";
import { NavLink } from "react-router-dom";

import { useQuitApp } from "../../hooks/useBackground";
import { useNow } from "../../hooks/useNow";
import { useRotationStatus } from "../../hooks/useRotation";
import { cx } from "../../services/cx";
import { describeNextChange } from "../../services/format";

interface NavItem {
  to: string;
  label: string;
  icon: LucideIcon;
  end?: boolean;
}

const MAIN_NAV: NavItem[] = [
  { to: "/", label: "Home", icon: House, end: true },
  { to: "/search", label: "Search", icon: Search },
  { to: "/collections", label: "Collections", icon: Library },
  { to: "/favorites", label: "Favorites", icon: Heart },
  { to: "/history", label: "History", icon: History },
  { to: "/scheduler", label: "Scheduler", icon: Timer },
];

const NAV_ENTRY = "flex h-9 items-center gap-3 rounded-lg px-3 font-medium transition-colors";
const IDLE_NAV_ENTRY = "text-muted hover:bg-raised/60 hover:text-fg";

function NavEntry({ item }: { item: NavItem }) {
  const Icon = item.icon;
  return (
    <NavLink
      to={item.to}
      end={item.end}
      className={({ isActive }) => cx(NAV_ENTRY, isActive ? "bg-raised text-fg" : IDLE_NAV_ENTRY)}
    >
      {({ isActive }) => (
        <>
          <Icon className={cx("size-[18px]", isActive && "text-accent")} aria-hidden />
          {item.label}
        </>
      )}
    </NavLink>
  );
}

function RotationIndicator() {
  const status = useRotationStatus().data;
  const now = useNow();
  if (!status?.active) return null;

  return (
    <NavLink
      to="/scheduler"
      className="mx-1 mb-3 flex flex-col gap-0.5 rounded-lg border border-line bg-canvas/50 px-3 py-2.5 hover:border-muted/40"
    >
      <span className="flex items-center gap-2 text-[13px] font-medium text-fg">
        <span className={cx("size-2 rounded-full", status.lastError ? "bg-danger" : "bg-accent")} aria-hidden />
        <span className="truncate">Rotating {status.collectionName}</span>
      </span>
      <span className="pl-4 text-[12px] text-muted">
        {status.lastError
          ? "The last change failed"
          : status.nextRunAt
            ? `Next change ${describeNextChange(status.nextRunAt, now, status.intervalSeconds)}`
            : "Waiting for the next change"}
      </span>
    </NavLink>
  );
}

/** Closing the window keeps a rotation running, so quitting needs its own control. */
function QuitButton() {
  const quit = useQuitApp();
  return (
    <button
      type="button"
      title="Close Kankulirchi completely and stop wallpaper rotation"
      disabled={quit.isPending}
      onClick={() => quit.mutate()}
      className={cx(NAV_ENTRY, IDLE_NAV_ENTRY, "w-full text-left")}
    >
      <Power className="size-[18px]" aria-hidden />
      Quit
    </button>
  );
}

export function Sidebar() {
  return (
    <nav aria-label="Main" className="relative z-10 flex w-[228px] shrink-0 flex-col border-r border-line/70 bg-surface/80 px-3 pt-5 pb-3">
      <div className="mb-6 flex items-center gap-2.5 px-2">
        <img src="/icon.png" alt="" className="size-7" draggable={false} />
        <span className="font-condensed text-lg font-semibold tracking-tight">Kankulirchi</span>
      </div>
      <div className="flex flex-col gap-0.5">
        {MAIN_NAV.map((item) => (
          <NavEntry key={item.to} item={item} />
        ))}
      </div>
      <div className="mt-auto flex flex-col">
        <RotationIndicator />
        <NavEntry item={{ to: "/settings", label: "Settings", icon: Settings }} />
        <QuitButton />
      </div>
    </nav>
  );
}
