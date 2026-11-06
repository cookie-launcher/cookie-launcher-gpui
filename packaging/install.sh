#!/usr/bin/env bash
# Install Cookie Launcher for the current user.
#
#   ./packaging/install.sh              # ~/.local
#   PREFIX=/usr/local ./packaging/install.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
BIN="$PREFIX/bin"
APPS="$PREFIX/share/applications"
ICONS="$PREFIX/share/icons/hicolor/256x256/apps"

echo "→ derleniyor (release)"
cargo build --release --manifest-path "$ROOT/Cargo.toml"

echo "→ kopyalanıyor: $BIN/cookie-launcher"
install -Dm755 "$ROOT/target/release/cookie-launcher-gpui" "$BIN/cookie-launcher"

echo "→ ikon: $ICONS/cookie-launcher.png"
install -Dm644 "$ROOT/assets/cookie-logo.png" "$ICONS/cookie-launcher.png"

echo "→ masaüstü girdisi: $APPS/cookie-launcher.desktop"
install -Dm644 "$ROOT/packaging/cookie-launcher.desktop" "$APPS/cookie-launcher.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APPS" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
fi

echo "✓ kuruldu: $BIN/cookie-launcher"
case ":$PATH:" in
  *":$BIN:"*) ;;
  *) echo "  not: $BIN PATH'te değil" ;;
esac
