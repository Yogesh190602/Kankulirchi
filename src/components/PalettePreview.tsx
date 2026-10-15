import { useWallpaperPalette } from "../hooks/useTheme";
import { errorMessage } from "../services/errors";
import { accentHex, accentLabel } from "../services/theme";
import type { Wallpaper } from "../types";

/** GNOME's window colors in its light and dark styles. */
const STYLES = {
  light: { window: "#fafafb", header: "#ebebed", text: "#323233", muted: "#c0bfbc" },
  dark: { window: "#222226", header: "#2e2e32", text: "#ffffff", muted: "#5e5c64" },
};

/** The wallpaper's main colors, and a sketch of GNOME in the accent color and style that suit it. */
export function PalettePreview({ wallpaper }: { wallpaper: Wallpaper }) {
  const palette = useWallpaperPalette(wallpaper);

  return (
    <section className="mt-8 border-t border-line pt-6" aria-labelledby="palette-title">
      <h3 id="palette-title" className="font-medium">
        Colors
      </h3>
      {palette.isPending ? (
        <div className="mt-3 h-8 animate-pulse rounded-lg bg-raised" aria-label="Working out the colors" />
      ) : palette.isError ? (
        <p className="mt-1 text-[13px] text-muted">The colors could not be worked out. {errorMessage(palette.error)}</p>
      ) : (
        <>
          <div className="mt-3 flex h-8 overflow-hidden rounded-lg" role="img" aria-label="Main colors">
            {palette.data.colors.map((color) => (
              <span
                key={color.hex}
                title={`${color.hex} · ${Math.round(color.share * 100)}%`}
                style={{ backgroundColor: color.hex, flexGrow: color.share }}
              />
            ))}
          </div>
          <ul data-selectable className="mt-2 flex flex-wrap gap-x-3 gap-y-1 text-[12px] text-muted tabular-nums">
            {palette.data.colors.map((color) => (
              <li key={color.hex}>{color.hex}</li>
            ))}
          </ul>
          <ThemeSketch
            thumbnail={wallpaper.thumbnailUrl}
            accent={accentHex(palette.data.accentColor)}
            dark={palette.data.dark}
          />
          <p className="mt-2 flex items-center gap-1.5 text-[13px] text-muted">
            <span
              className="inline-block size-2.5 rounded-full"
              style={{ backgroundColor: accentHex(palette.data.accentColor) }}
              aria-hidden
            />
            Suits a {accentLabel(palette.data.accentColor).toLowerCase()} accent and the {palette.data.dark ? "dark" : "light"}{" "}
            style
          </p>
        </>
      )}
    </section>
  );
}

/** A miniature desktop: the wallpaper, the top bar and a window. */
function ThemeSketch({ thumbnail, accent, dark }: { thumbnail: string; accent: string; dark: boolean }) {
  const style = dark ? STYLES.dark : STYLES.light;
  return (
    <div
      className="relative mt-4 aspect-[16/9] overflow-hidden rounded-lg bg-cover bg-center"
      style={{ backgroundImage: `url("${thumbnail}")` }}
      aria-hidden
    >
      <div className="absolute inset-x-0 top-0 h-2.5 bg-black/85" />
      <div
        className="absolute top-[22%] right-[14%] bottom-[12%] left-[14%] overflow-hidden rounded-md shadow-lg"
        style={{ backgroundColor: style.window }}
      >
        <div className="h-[18%]" style={{ backgroundColor: style.header }} />
        <div className="flex flex-col gap-[7%] p-[6%]">
          <div className="flex items-center justify-between">
            <span className="h-1.5 w-2/5 rounded-full" style={{ backgroundColor: style.muted }} />
            <span className="flex h-3 w-6 items-center justify-end rounded-full px-0.5" style={{ backgroundColor: accent }}>
              <span className="size-2 rounded-full bg-white" />
            </span>
          </div>
          <div className="flex items-center justify-between">
            <span className="h-1.5 w-1/3 rounded-full" style={{ backgroundColor: style.muted }} />
            <span className="h-3 w-6 rounded-full" style={{ backgroundColor: style.muted }} />
          </div>
          <span className="mt-[2%] h-4 w-1/3 self-end rounded" style={{ backgroundColor: accent }} />
        </div>
      </div>
    </div>
  );
}
