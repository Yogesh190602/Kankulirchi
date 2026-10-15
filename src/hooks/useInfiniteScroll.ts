import { useEffect, useRef } from "react";

/**
 * Calls `onReach` when the returned sentinel element comes within 600px of
 * the viewport. `version` should change whenever new items are loaded, so a
 * sentinel that is still visible triggers again.
 */
export function useInfiniteScroll<T extends HTMLElement>(onReach: () => void, enabled: boolean, version: number) {
  const sentinel = useRef<T>(null);
  const callback = useRef(onReach);

  useEffect(() => {
    callback.current = onReach;
  }, [onReach]);

  useEffect(() => {
    const node = sentinel.current;
    if (!node || !enabled) return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) callback.current();
      },
      { rootMargin: "600px 0px" },
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [enabled, version]);

  return sentinel;
}
