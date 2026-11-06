#!/usr/bin/env bash
# Build a portable bundle: dist/cookie-launcher-<version>.tar.gz
#
# If `appimagetool` is on PATH an AppImage is produced as well; the tarball is
# always created so the result is usable on machines without AppImage support.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
NAME="cookie-launcher-${VERSION}"
DIST="$ROOT/dist"
APPDIR="$DIST/$NAME"

echo "→ derleniyor (release)"
cargo build --release --manifest-path "$ROOT/Cargo.toml"

rm -rf "$APPDIR"
mkdir -p "$APPDIR/bin" "$APPDIR/share/applications" \
         "$APPDIR/share/icons/hicolor/256x256/apps"

install -m755 "$ROOT/target/release/cookie-launcher-gpui" "$APPDIR/bin/cookie-launcher"
install -m644 "$ROOT/packaging/cookie-launcher.desktop" "$APPDIR/share/applications/"
install -m644 "$ROOT/assets/cookie-logo.png" \
        "$APPDIR/share/icons/hicolor/256x256/apps/cookie-launcher.png"

cat > "$APPDIR/install.sh" <<'INNER'
#!/usr/bin/env bash
# Install this bundle for the current user.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
install -Dm755 "$HERE/bin/cookie-launcher" "$PREFIX/bin/cookie-launcher"
install -Dm644 "$HERE/share/icons/hicolor/256x256/apps/cookie-launcher.png" \
  "$PREFIX/share/icons/hicolor/256x256/apps/cookie-launcher.png"
install -Dm644 "$HERE/share/applications/cookie-launcher.desktop" \
  "$PREFIX/share/applications/cookie-launcher.desktop"
echo "✓ kuruldu: $PREFIX/bin/cookie-launcher"
INNER
chmod +x "$APPDIR/install.sh"

cat > "$APPDIR/README.txt" <<'INNER'
Cookie Launcher
===============
Minecraft başlatıcı (GPUI, %100 Rust).

Kurulum:  ./install.sh
Çalıştırma (kurulumsuz):  ./bin/cookie-launcher

Gereksinimler: Linux (X11 veya Wayland), Vulkan/OpenGL sürücüsü, internet
(ilk çalıştırmada sürüm manifesti ve oyun dosyaları indirilir).
Minecraft için ayrıca bir JVM gerekir (Java 8-21; launcher uygun olanı seçer).
INNER

echo "→ tarball"
( cd "$DIST" && tar -czf "$NAME.tar.gz" "$NAME" )

if command -v appimagetool >/dev/null 2>&1; then
  echo "→ AppImage"
  APPIMAGE_DIR="$DIST/AppDir"
  rm -rf "$APPIMAGE_DIR"
  cp -r "$APPDIR" "$APPIMAGE_DIR"
  cat > "$APPIMAGE_DIR/AppRun" <<'INNER'
#!/bin/sh
HERE="$(dirname "$(readlink -f "$0")")"
exec "$HERE/bin/cookie-launcher" "$@"
INNER
  chmod +x "$APPIMAGE_DIR/AppRun"
  cp "$APPDIR/share/applications/cookie-launcher.desktop" "$APPIMAGE_DIR/"
  cp "$APPDIR/share/icons/hicolor/256x256/apps/cookie-launcher.png" "$APPIMAGE_DIR/cookie-launcher.png"
  ARCH=x86_64 appimagetool "$APPIMAGE_DIR" "$DIST/$NAME-x86_64.AppImage"
else
  echo "  (appimagetool yok — yalnızca tarball üretildi)"
fi

echo "✓ $DIST/$NAME.tar.gz"
ls -lh "$DIST"/*.tar.gz "$DIST"/*.AppImage 2>/dev/null || true
