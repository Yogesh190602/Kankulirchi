# Kankulirchi

A desktop personalization engine for Linux. Kankulirchi searches Wallhaven,
Unsplash, Pexels and Wikimedia Commons for wallpapers, keeps the ones you like
in collections and favorites, caches them locally, and rotates them on the
GNOME desktop.

Built with Tauri v2, React, TypeScript, Tailwind CSS, Rust and SQLite.
The original product specification is in [`docs/`](docs/); see
[Implementation notes](#implementation-notes) for where the app differs
from it.

## Features

| Feature | Where |
| --- | --- |
| Keyword search on Wallhaven, Unsplash, Pexels and Wikimedia Commons, provider filter, infinite scroll | Search |
| Preview, download, favorite, add to collection, apply now; photographer and license credits | Wallpaper detail (click any wallpaper) |
| Main colors of a wallpaper, with the GNOME accent color and style that suit it | Wallpaper detail → Colors |
| Backup of your desktop settings before Kankulirchi first changes them, with one-click restore | Settings → Desktop backup |
| Create, rename and delete collections; add and remove wallpapers | Collections, Collection detail |
| Favorites list | Favorites |
| Local cache with duplicate-download protection, metadata in SQLite; deleting a collection frees its downloads | Settings → Cache, Collections |
| Each picture is kept once per collection: re-uploads of the same picture are removed, keeping the sharpest copy | Collections, rotation |
| Rotation every 1 min, 5 min, 15 min, 30 min, 1 hour, daily, or a custom number of seconds (5 to 86,400), in sequential or random order | Scheduler, Collection detail |
| Rotate everything a search found, on the same schedule | Search (above the results) |
| Wallpaper changes through `gsettings` | Apply now, rotation |
| History of the last 1,000 applied wallpapers with "Apply again" | Home, History |
| Rotation keeps going after the window is closed, optional start at login | Settings → Background, Quit in the sidebar |

## Providers

| Provider | API key | What it returns |
| --- | --- | --- |
| Wallhaven | Optional | Safe-for-work landscape wallpapers |
| Unsplash | Required (free) | Landscape photos with the strict content filter, downloaded at up to 3840 px wide |
| Pexels | Required (free) | Landscape photos |
| Wikimedia Commons | None | Featured pictures (the photos the Commons community rates as its best) of scenery: landscapes, mountains, lakes, waterfalls, beaches, forests, sunsets, clouds, skylines, cityscapes, astronomy and flowers. Landscape JPEG and PNG files at least 1920 px wide, downloaded at up to 3840 px wide |

Searching **All providers** uses every provider that is turned on and has the
key it needs. A provider still waiting for a key is skipped there; choosing it
in the provider filter explains what is missing.

### Safe search

Kankulirchi only shows safe-for-work wallpapers, and this cannot be turned
off:

- Searches for adult or suggestive keywords (nudity, sexual terms, lingerie,
  swimwear) are refused before any provider is asked
  (`providers/safe_search.rs`).
- Wallhaven is always asked for its SFW purity, even with an API key whose
  account allows more, and any result not rated SFW is dropped.
- Unsplash searches use its strict content filter (`content_filter=high`).
- Wikimedia Commons has no content filter, and its featured pictures include
  nude photography, nude art and war photographs, so only the scenery topics
  above are searched.
- Pexels has no content filter in its API; it does not accept explicit
  uploads, and the keyword check above applies.

The keyword list is in English, so a search in another language relies on the
providers' own filters.

### Credits and API keys

The wallpaper panel credits every wallpaper the way its provider or license
asks: "Photo by *photographer* on *provider*", with links to the
photographer's profile and to the wallpaper's page, and the license for
Wikimedia Commons pictures (most are CC BY or CC BY-SA, which require naming
the author). Links to Unsplash carry the referral parameters Unsplash asks
for. Hovering over a wallpaper in a grid shows its photographer. Wallpapers
saved before credits were recorded get their page link and, for Commons,
their author and license, looked up once in the background.

Requests name the app and its GitHub address in their User-Agent header, as
Wikimedia asks of every app that uses its API.

API keys are only stored in the settings file, which only your user can read,
and are sent to providers in request headers, never in URLs. Network errors
leave out URL query strings, so a key cannot end up in the log or on screen.

## Requirements

- Arch Linux (other distributions work if they provide the same libraries)
- A GNOME session. Kankulirchi changes the wallpaper with `gsettings`, which
  ships with `glib2`.
- To build: Rust (stable, 1.90 or newer), Node.js 20.19+ or 22.12+, and the
  Tauri system libraries.

Install the system packages on Arch:

```sh
sudo pacman -S --needed base-devel curl wget file openssl webkit2gtk-4.1 librsvg glib2 gsettings-desktop-schemas
```

Install Rust with [rustup](https://rustup.rs) and Node.js with `pacman -S nodejs npm`
(or a version manager).

## Development

```sh
npm install
npm run tauri dev
```

`tauri dev` starts the Vite dev server on port 1420 and opens the app with hot
reload for the frontend; Rust changes rebuild and restart the app.

Checks:

```sh
npm run typecheck        # TypeScript
npm run lint             # ESLint
cd src-tauri
cargo test               # Rust unit tests (no network)
cargo test -- --ignored  # Live tests: search Wallhaven, Commons and Unsplash and download one image each
                         # (the Unsplash test needs UNSPLASH_ACCESS_KEY set)
cargo clippy --all-targets
cargo fmt --check
```

## Building and installing

To install for your user and start the app by typing `kankulirchi` in a
terminal or from the GNOME app grid, install the
[requirements](#requirements), then:

```sh
git clone https://github.com/Yogesh190602/Kankulirchi.git
cd Kankulirchi
./install.sh
```

`npm run install:local` does the same. The script installs the JavaScript
dependencies on its first run, builds a release binary and copies it, with its
icon, to `~/.local/lib/kankulirchi/`. It adds a `kankulirchi` launcher to
`~/.local/bin` (which must be on your `PATH`) and an app grid entry to
`~/.local/share/applications`. The launcher detaches the app from the
terminal, so closing the terminal does not stop rotation. Run the script again
after changing the code.

To build distributable packages instead:

```sh
npm run tauri build
```

This produces, under `src-tauri/target/release/bundle/`:

- `appimage/Kankulirchi_0.1.0_amd64.AppImage` – runs on any distribution:
  `chmod +x Kankulirchi_*.AppImage && ./Kankulirchi_*.AppImage`
- `deb/` and `rpm/` packages for Debian/Ubuntu and Fedora/openSUSE

To install system-wide on Arch instead:

```sh
npm run tauri build -- --no-bundle
sudo install -Dm755 src-tauri/target/release/kankulirchi /usr/local/bin/kankulirchi
sudo install -Dm644 src-tauri/icons/128x128@2x.png /usr/local/share/icons/hicolor/256x256/apps/kankulirchi.png
sudo tee /usr/local/share/applications/kankulirchi.desktop >/dev/null <<'EOF'
[Desktop Entry]
Name=Kankulirchi
Comment=Search, collect and rotate GNOME wallpapers
Exec=kankulirchi
Icon=kankulirchi
Terminal=false
Type=Application
Categories=Utility;
EOF
```

## First run

1. Wallhaven and Wikimedia Commons work right away. Unsplash and Pexels need a
   free key: open **Settings → Providers**, follow the provider's link, paste
   the key and choose **Save key**. For Unsplash, create an application at
   unsplash.com/oauth/applications and use its **Access Key**. New Unsplash
   keys allow 50 requests an hour.
2. Search for something, open a wallpaper and choose **Apply now**, or add it
   to a collection.
3. Open the collection (or **Scheduler**), pick an interval and choose
   **Start rotation**. For an interval the presets don't offer, choose
   **Custom** and type a number of seconds, from 5 up to 86,400 (one day). The
   number takes effect when you press Enter or leave the field.

To rotate the results of a search instead, search for something (for example
“mountains”), pick an interval in the **Rotate these wallpapers** panel above
the results and choose **Start rotation**. The first wallpaper is applied right
away. The results are saved as a collection named after your keyword, so you
can remove ones you don't like from it. Only results loaded so far are
included: scroll down to load more, then choose **Update rotation** to add them.

## Desktop colors and backup

The **Colors** section of the wallpaper panel shows the wallpaper's main
colors, the GNOME accent color closest to its most vivid color, and whether
it suits GNOME's light or dark style, with a small sketch of how GNOME would
look. Kankulirchi does not change the accent color or style yet.

Just before Kankulirchi first changes your desktop, it saves your
background, lock screen picture, accent color and style. **Settings → Desktop
backup** shows what was saved, and **Restore desktop** stops rotation and puts
it all back. If your wallpaper was already one of Kankulirchi's downloads when
the backup was made (as it is after upgrading from a version without
backups), the wallpaper before it is not known, so restoring sets GNOME's
default wallpaper.

## Running in the background

While a rotation is on, closing the window does not stop Kankulirchi: it keeps
running without a window and keeps changing the wallpaper. Run `kankulirchi`
again to open the window, or choose **Quit** at the bottom of the sidebar to
stop it completely. With no rotation on, closing the window quits.

Turn on **Settings → Background → Start at login** to start Kankulirchi in the
background when you log in. This adds an entry to `~/.config/autostart` that
runs Kankulirchi with `--background`, which opens no window and exits right
away if no rotation is on. Removing the entry elsewhere, for example in GNOME
Tweaks, turns the setting off.

If Kankulirchi was not running when a change came due, the change happens as
soon as it starts again.

## Where data is stored

| What | Path |
| --- | --- |
| Library database (SQLite) | `~/.local/share/com.kankulirchi.desktop/library.db` |
| Settings and API keys (mode 0600) | `~/.config/com.kankulirchi.desktop/settings.json` |
| Downloaded wallpapers (default) | `~/.cache/com.kankulirchi.desktop/wallpapers/` |
| Logs | `~/.local/share/com.kankulirchi.desktop/logs/` |
| Start at login entry | `~/.config/autostart/kankulirchi.desktop` |
| App grid entry | `~/.local/share/applications/kankulirchi.desktop` |
| Desktop settings backup | `~/.local/share/com.kankulirchi.desktop/desktop-backup.json` |

The cache location can be changed in Settings. New downloads go to the new
folder; files already downloaded stay where they are.

If a downloaded file is damaged or missing, it is downloaded again the next
time it is needed; while offline, rotation uses the other downloads.

A collection holds each picture once. The same picture often turns up under
different ids, such as a re-upload at another size; when a copy of a picture
already in the collection is downloaded or added, the sharper copy stays and
the other is removed, along with its download if nothing else uses it. So a
rotation never shows the same picture twice under different names. Every
collection is checked once at each start, which covers wallpapers saved
before this check existed; the first start after updating spends a few
minutes of one processor core on it.

Downloads are kept until you delete the collection they belong to. Deleting a
collection deletes its downloaded wallpapers too, except ones that another
collection or your favorites have, and the wallpaper on screen. They stay in
your history and are downloaded again if you apply them. The history keeps
the last 1,000 applied wallpapers.

## Project structure

```
src/                         React frontend (no business logic)
  components/                Shared UI: grid, detail dialog, rotation controls, layout
  pages/                     One component per screen
  hooks/                     TanStack Query hooks and other React hooks
  services/                  Typed Tauri command wrappers, formatting, query keys
  store/                     Zustand stores (detail dialog, toasts)
  types/                     TypeScript mirrors of the Rust models
src-tauri/src/
  providers/                 Provider abstraction
    mod.rs                   WallpaperProvider trait, registry, multi-provider search
    wallhaven.rs
    unsplash.rs
    pexels.rs
    wikimedia.rs             Wikimedia Commons
    safe_search.rs           Refuses adult search keywords
  wallpaper/
    apply.rs                 Setting the wallpaper and history recording
    scheduler.rs             Rotation scheduler
  theme/
    palette.rs               A wallpaper's main colors and the accent that suits it
    backup.rs                Desktop settings backup and restore
  database/
    sqlite.rs                Schema migrations and every SQL query
  cache/
    downloader.rs            Local cache: download, validate, deduplicate
    fingerprint.rs           Telling when two wallpapers are the same picture
  commands/                  Tauri commands, the only frontend/backend interface
    search.rs  collections.rs  favorites.rs
    wallpapers.rs            Download, apply, history
    rotation.rs              Start, stop, settings, status
    settings.rs              Cache location, cache size, provider settings
    app.rs                   Start at login, quit
    theme.rs                 Palettes, desktop backup and restore
  gnome.rs                   gsettings: read, set and reset GNOME settings
  repeats.rs                 Keeping one copy of each picture in a collection
  background.rs              Running without a window
  autostart.rs               Start at login (XDG autostart entry)
  settings.rs                Settings file
  models.rs                  Types shared by commands and the frontend
  state.rs  events.rs  error.rs  lib.rs  main.rs
```

### How the pieces fit

- **Frontend → backend.** The frontend calls Tauri commands through
  `src/services/api.ts` and never touches the network, the file system or the
  database itself. The backend pushes changes back as events
  (`wallpaper-applied`, `rotation-status`, `wallpaper-cached`), which keep the
  TanStack Query caches current.
- **Providers.** Each provider implements `WallpaperProvider`
  (`metadata`, `image_hosts`, `search`, and optionally `report_download`).
  Searches across several providers run concurrently; one provider failing is
  reported next to the other providers' results instead of failing the search.
  Downloads are only allowed over HTTPS from the hosts a provider declares.
  `report_download` runs the first time a wallpaper is applied and when it is
  downloaded with the Download button; Unsplash uses it for the download
  tracking its API guidelines require. Each result carries a credit (author,
  profile, page, license) whose links must point to hosts the provider
  declares, and `credits` looks up credits for wallpapers saved without them
  (only Commons needs it).
- **Cache.** A wallpaper is stored once as `<provider>-<id>.<ext>`. Concurrent
  requests for the same wallpaper share one download, files are written to a
  `.part` file and checked to be a complete image before being moved into
  place (PNG and WebP are decoded; JPEG decoders draw a cut-off file without
  complaint, so its segments are walked to the end-of-image marker), and the
  measured dimensions are saved to SQLite. Before a downloaded file is
  applied, its header is checked again; a damaged file is deleted and
  downloaded again. Wallpapers added to a
  collection are downloaded in the background, three at a time, so rotation
  works offline. Search results being rotated are downloaded in rotation
  order, so the next wallpapers are ready first. Deleting a collection clears
  the local path of each download nothing else uses in the same transaction,
  then deletes the files, skipping any file not named like a cached wallpaper.
- **Apply.** `gsettings set org.gnome.desktop.background picture-uri
  file:///…`. On GNOME 42 and later `picture-uri-dark` is set too, otherwise the
  change would not show while the dark style is on. The desktop backup is
  written (once) just before the first change; if it cannot be written, the
  change goes ahead and the failure is logged.
- **Repeated pictures.** A fingerprint holds a 16×16 grey thumbnail, the fine
  detail of a 48×48 one (the thumbnail minus a blurred copy) and the average
  color. Two wallpapers are the same picture when the thumbnails correlate at
  0.95 or more, the details at 0.75 or more and the colors are within 15 per
  channel. The detail test is what tells re-uploads from pictures that are
  merely alike, such as two foggy forests, whose broad gradients match. On a
  library of 824 wallpapers it found the 11 pairs of copies a manual check
  found, and nothing else. Fingerprints are worked out from downloaded files
  and kept in the `fingerprint` column; a new download is compared only with
  the rest of its collections, a full pairwise check runs once per start.
- **Palettes.** The picture is shrunk to 96 px, its pixels are grouped into
  up to six colors with k-means in Oklab (starting centers chosen
  deterministically, so a picture always gives the same palette), colors under
  2% of the picture are dropped, and the most vivid remaining color picks
  GNOME's accent color by hue (slate for greyish pictures). The palette is
  worked out from the downloaded file, or from the thumbnail for a search
  result, and kept in the `palette` column.
- **Rotation.** One collection rotates at a time (enforced by a unique index).
  The next change is computed from the newest history entry plus the interval,
  so no timer state is lost on restart and applying a wallpaper by hand
  restarts the countdown. Sequential order follows the order wallpapers were
  added, continuing after the collection's newest history entry (or from the
  start once that entry is past the 1,000 the history keeps); random order
  never repeats the wallpaper on screen. If a wallpaper
  cannot be downloaded, the scheduler falls back to cached ones and retries
  after the interval, but no sooner than 30 seconds and no later than five
  minutes, so a short interval does not hammer a failing provider. With a short
  interval, a change can be late while the wallpaper it needs is still
  downloading.
- **Search rotation.** `start_search_rotation` saves the loaded search results
  to a collection named after the keyword (adding to it if it already exists)
  in one transaction, then starts rotating that collection. The scheduler and
  the database schema are the same as for any other rotation.

### Adding a provider

1. Create `src-tauri/src/providers/<name>.rs` implementing `WallpaperProvider`.
   Use the provider's strictest content filter, so results stay safe for work.
2. Add `pub mod <name>;` and one `registry.register(...)` line in
   `ProviderRegistry::with_defaults` (`providers/mod.rs`).
3. If Settings should link to the provider's website or key page, allow that
   site in `src-tauri/capabilities/default.json` (`opener:allow-open-url`).

Existing providers, the frontend, the database and the settings need no
changes: the provider filter, Settings (enable switch and API key field) and
attribution text are driven by the provider's metadata.

## Implementation notes

Choices made where the specification left room:

- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) lists three command
  modules. Downloading, applying, history, rotation and settings got their own
  modules (`wallpapers.rs`, `rotation.rs`, `settings.rs`) rather than being
  folded into unrelated ones.
- The database follows [`docs/DATABASE_SCHEMA.md`](docs/DATABASE_SCHEMA.md),
  with constraints and indexes added, except that
  `rotation_settings.interval_minutes` became `interval_seconds` (migration 2)
  so rotation can run faster than once a minute, and `wallpapers` gained credit
  columns (migration 3), a `palette` column (migration 4) and a `fingerprint`
  column (migration 5). Settings (cache location, provider keys) are not part
  of that schema, so they live in `settings.json`.
- [`docs/MVP_FEATURES.md`](docs/MVP_FEATURES.md) asks for a Favorites page
  and a history with reapply, which [`docs/UI_SPEC.md`](docs/UI_SPEC.md) does
  not list as screens; both are included.
- Searches return landscape images only, and only safe-for-work ones (see
  [Safe search](#safe-search)).
- Pixabay and NASA's image library were considered and left out: Pixabay's
  standard API keys only return images up to 1280 px wide, and most NASA
  results are small science images rather than wallpapers.
- Running in the background has no tray icon, because GNOME shows tray icons
  only with an extension; **Quit** in the sidebar stops the app instead.
  Closing the window destroys it rather than hiding it, so the web view's
  memory is released while it is closed.

## License

[MIT](LICENSE)
