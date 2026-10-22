#!/bin/sh
# Installs Kankulirchi for the current user from the latest GitHub release,
# without building it, installing anything else or using sudo:
#
#   curl -fsSL https://raw.githubusercontent.com/Yogesh190602/Kankulirchi/main/get.sh | sh
#   wget -qO- https://raw.githubusercontent.com/Yogesh190602/Kankulirchi/main/get.sh | sh
#
# Run it again to update. Also:
#   sh get.sh Kankulirchi_0.1.0_amd64.AppImage   install an AppImage you downloaded
#   sh get.sh --uninstall                        remove Kankulirchi, keeping your library
#
# The release's AppImage carries everything Kankulirchi needs. It is kept in
# ~/.local/lib/kankulirchi, with a `kankulirchi` command in ~/.local/bin and an
# app grid entry, the same places ./install.sh uses when building from source.
set -eu

repo="Yogesh190602/Kankulirchi"
app_dir="$HOME/.local/lib/kankulirchi"
bin_dir="$HOME/.local/bin"
applications_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
autostart_entry="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/kankulirchi.desktop"
appimage="$app_dir/Kankulirchi.AppImage"
icon="$app_dir/kankulirchi.png"
launcher="$bin_dir/kankulirchi"
entry="$applications_dir/kankulirchi.desktop"

say() { printf '%s\n' "$*"; }
fail() {
  printf 'Kankulirchi: %s\n' "$*" >&2
  exit 1
}

# Prints what is at URL, with curl or wget, whichever this system has.
fetch_text() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1"
  elif command -v wget >/dev/null 2>&1; then
    wget -qO- "$1"
  else
    fail "needs curl or wget to download."
  fi
}

# Downloads URL to FILE, showing progress.
fetch_file() {
  if command -v curl >/dev/null 2>&1; then
    curl -fL --progress-bar -o "$2" "$1"
  else
    wget -q --show-progress -O "$2" "$1"
  fi
}

if [ "${1:-}" = "--uninstall" ]; then
  rm -rf "$app_dir"
  rm -f "$launcher" "$entry" "$autostart_entry"
  say "Kankulirchi is uninstalled. If it is still running, choose Quit in its sidebar."
  say "Your collections and settings are kept in ~/.local/share/com.kankulirchi.desktop"
  say "and ~/.config/com.kankulirchi.desktop; delete those folders to remove them too."
  exit 0
fi

[ "$(uname -s)" = Linux ] && [ "$(uname -m)" = x86_64 ] || fail "only 64-bit Linux (x86_64) is supported."

mkdir -p "$app_dir" "$bin_dir" "$applications_dir"
download="$appimage.part"
trap 'rm -f "$download"; rm -rf "$app_dir/extract"' EXIT

if [ $# -gt 0 ]; then
  [ -f "$1" ] || fail "$1 is not a file."
  name=${1##*/}
  cp "$1" "$download"
else
  say "Looking up the latest release..."
  url=$(fetch_text "https://api.github.com/repos/$repo/releases/latest" |
    grep -o '"browser_download_url": *"[^"]*\.AppImage"' | head -n 1 | sed 's/.*"\(https[^"]*\)"/\1/')
  [ -n "$url" ] || fail "could not find an AppImage in the latest release."
  name=${url##*/}
  say "Downloading $name"
  fetch_file "$url" "$download"
fi
chmod +x "$download"
# Moved into place in one step, so a copy that is running keeps working.
mv -f "$download" "$appimage"

# The icon, taken from the AppImage itself (unpacking one file needs no FUSE).
# Kankulirchi.png is the AppImage's own icon; kankulirchi.png is only a link.
mkdir -p "$app_dir/extract"
if (cd "$app_dir/extract" && "$appimage" --appimage-extract Kankulirchi.png >/dev/null 2>&1) &&
  [ -f "$app_dir/extract/squashfs-root/Kankulirchi.png" ]; then
  cp "$app_dir/extract/squashfs-root/Kankulirchi.png" "$icon"
fi

cat >"$launcher" <<EOF
#!/bin/sh
# Starts Kankulirchi detached from the terminal, so closing the terminal does
# not stop wallpaper rotation. Installed by get.sh.
# Without FUSE, the AppImage unpacks itself to a temporary folder instead.
if ! command -v fusermount3 >/dev/null 2>&1 && ! command -v fusermount >/dev/null 2>&1; then
  export APPIMAGE_EXTRACT_AND_RUN=1
fi
exec setsid -f "$appimage" "\$@" >/dev/null 2>&1
EOF
chmod 755 "$launcher"

# The app grid entry. Its window reports the app id `kankulirchi`, which
# StartupWMClass matches, so GNOME shows the window under this icon.
cat >"$entry" <<EOF
[Desktop Entry]
Type=Application
Name=Kankulirchi
Comment=Search, collect and rotate wallpapers
Exec="$launcher"
Icon=$([ -f "$icon" ] && printf '%s' "$icon" || printf 'kankulirchi')
Terminal=false
Categories=Utility;GTK;
Keywords=wallpaper;background;rotation;
StartupWMClass=kankulirchi
EOF

version=$(printf '%s' "$name" | sed -n 's/^Kankulirchi_\(.*\)_amd64\.AppImage$/\1/p')
say "Installed Kankulirchi${version:+ $version}."
case ":$PATH:" in
  *":$bin_dir:"*) say "Start it from the app grid, or by typing: kankulirchi" ;;
  *)
    say "Start it from the app grid, or by typing: $launcher"
    say "(Open a new terminal, or log out and in, to type just: kankulirchi)"
    ;;
esac
case "${XDG_CURRENT_DESKTOP:-}" in
  *GNOME*) ;;
  *) say "Note: Kankulirchi changes the wallpaper on the GNOME desktop only." ;;
esac
