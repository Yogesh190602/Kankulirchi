#!/bin/sh
# Builds Kankulirchi and installs it for the current user, so typing
# `kankulirchi` in a terminal, or picking it in the GNOME app grid, opens the
# app. Run it from anywhere: ./install.sh (or npm run install:local). Run it
# again after changing the code.
set -eu

cd "$(dirname "$0")"

# A fresh clone has no JavaScript dependencies yet.
if [ ! -d node_modules ]; then
  npm ci
fi

npm run tauri build -- --no-bundle

app_dir="$HOME/.local/lib/kankulirchi"
bin_dir="$HOME/.local/bin"
applications_dir="${XDG_DATA_HOME:-$HOME/.local/share}/applications"

install -Dm755 src-tauri/target/release/kankulirchi "$app_dir/kankulirchi"
install -Dm644 src-tauri/icons/icon.png "$app_dir/kankulirchi.png"

mkdir -p "$bin_dir"
cat > "$bin_dir/kankulirchi" <<'EOF'
#!/bin/sh
# Starts Kankulirchi detached from the terminal, so closing the terminal does
# not stop wallpaper rotation. Logs are in ~/.local/share/com.kankulirchi.desktop/logs.
exec setsid -f "$HOME/.local/lib/kankulirchi/kankulirchi" "$@" >/dev/null 2>&1
EOF
chmod 755 "$bin_dir/kankulirchi"

# The app grid entry. Its name matches the window's app id, `kankulirchi`, so
# GNOME shows the open window under this entry's icon. The icon is referenced
# by path, so no icon theme cache has to be refreshed.
mkdir -p "$applications_dir"
cat > "$applications_dir/kankulirchi.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Kankulirchi
Comment=Search, collect and rotate wallpapers
Exec="$app_dir/kankulirchi"
Icon=$app_dir/kankulirchi.png
Terminal=false
Categories=Utility;GTK;
Keywords=wallpaper;background;rotation;
StartupWMClass=kankulirchi
EOF

echo "Installed. Run: kankulirchi, or open Kankulirchi from the app grid."
case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) echo "Note: $bin_dir is not on your PATH. Add it to your shell profile to use the command." ;;
esac
