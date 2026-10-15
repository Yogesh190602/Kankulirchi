import { useEffect, useRef } from "react";
import { Outlet, useLocation } from "react-router-dom";

import { Toaster } from "../ui/Toaster";
import { WallpaperDetailDialog } from "../WallpaperDetailDialog";
import { AmbientBackdrop } from "./AmbientBackdrop";
import { Sidebar } from "./Sidebar";

export function AppLayout() {
  const location = useLocation();
  const main = useRef<HTMLElement>(null);

  useEffect(() => {
    main.current?.scrollTo({ top: 0 });
  }, [location.pathname, location.search]);

  return (
    <div className="relative flex h-full">
      <AmbientBackdrop />
      <Sidebar />
      <main ref={main} className="relative z-10 min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-[1400px] px-10 pt-10 pb-16">
          <Outlet />
        </div>
      </main>
      <WallpaperDetailDialog />
      <Toaster />
    </div>
  );
}
