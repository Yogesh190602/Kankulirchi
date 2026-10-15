import { confirm, open } from "@tauri-apps/plugin-dialog";

export function confirmDeleteCollection(name: string): Promise<boolean> {
  return confirm(
    `Delete “${name}”? Its downloaded wallpapers are deleted too, unless another collection or your favorites have them. They stay in your history.`,
    {
      title: "Delete collection",
      kind: "warning",
      okLabel: "Delete",
      cancelLabel: "Cancel",
    },
  );
}

export function confirmRestoreDesktop(): Promise<boolean> {
  return confirm(
    "Put back your wallpaper, lock screen, accent color and style as they were before Kankulirchi first changed them? Wallpaper rotation stops.",
    { title: "Restore desktop", kind: "warning", okLabel: "Restore", cancelLabel: "Cancel" },
  );
}

export async function chooseFolder(defaultPath: string): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false, defaultPath, title: "Choose cache folder" });
  return typeof selected === "string" ? selected : null;
}
