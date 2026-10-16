#!/usr/bin/env bash
# Builds FlashPad from source and installs it for the current user only
# (~/.local), for distros the CI's .deb/.rpm don't cover - e.g. Arch-based
# ones like Omarchy. Re-run it to update: the in-app updater only knows how
# to replace AppImages on Linux, so it can't update this install.
#
# Needs: base-devel, webkit2gtk-4.1, gtk3, librsvg, libayatana-appindicator,
# plus Node.js and a Rust toolchain.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
prefix="${PREFIX:-$HOME/.local}"

cd "$repo"
npm install
npm run tauri build -- --no-bundle

install -Dm755 src-tauri/target/release/flashpad "$prefix/bin/flashpad"
install -Dm644 src-tauri/icons/128x128.png "$prefix/share/icons/hicolor/128x128/apps/flashpad.png"
install -Dm644 /dev/stdin "$prefix/share/applications/flashpad.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=FlashPad
Comment=Global-hotkey quick-notes app
Exec=$prefix/bin/flashpad
Icon=flashpad
Terminal=false
Categories=Utility;TextEditor;
StartupWMClass=Flashpad
DESKTOP

echo "Installed to $prefix/bin/flashpad"
echo "On Hyprland, bind your hotkey to: $prefix/bin/flashpad --toggle"
