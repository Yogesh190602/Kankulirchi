import { create } from "zustand";

import { isSameWallpaper } from "../services/wallpapers";
import type { Wallpaper } from "../types";

interface DetailState {
  /** Wallpaper shown in the detail dialog, if open. */
  wallpaper: Wallpaper | null;
  open: (wallpaper: Wallpaper) => void;
  close: () => void;
}

export const useDetailStore = create<DetailState>((set) => ({
  wallpaper: null,
  open: (wallpaper) => set({ wallpaper }),
  close: () => set({ wallpaper: null }),
}));

/** Keeps the open detail dialog in sync after a wallpaper changed. */
export function syncDetail(updated: Wallpaper) {
  const { wallpaper } = useDetailStore.getState();
  if (wallpaper && isSameWallpaper(wallpaper, updated)) {
    useDetailStore.setState({ wallpaper: { ...wallpaper, ...updated } });
  }
}

export type ToastTone = "info" | "error";

export interface Toast {
  id: number;
  message: string;
  tone: ToastTone;
}

interface ToastState {
  toasts: Toast[];
  notify: (message: string, tone?: ToastTone) => void;
  dismiss: (id: number) => void;
}

let nextToastId = 1;
const MAX_TOASTS = 4;

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],
  notify: (message, tone = "info") => {
    const id = nextToastId++;
    set((state) => ({ toasts: [...state.toasts, { id, message, tone }].slice(-MAX_TOASTS) }));
    window.setTimeout(() => get().dismiss(id), tone === "error" ? 8000 : 4000);
  },
  dismiss: (id) => set((state) => ({ toasts: state.toasts.filter((toast) => toast.id !== id) })),
}));

export const notify = (message: string, tone?: ToastTone) => useToastStore.getState().notify(message, tone);
