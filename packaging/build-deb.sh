#!/usr/bin/env bash
# Build a .deb from the release binary: dist/cookie-launcher_<version>_amd64.deb
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
DIST="$ROOT/dist"
PKG="$DIST/deb/cookie-launcher_${VERSION}_${ARCH}"

command -v dpkg-deb >/dev/null 2>&1 || { echo "dpkg-deb yok"; exit 1; }

cargo build --release --manifest-path "$ROOT/Cargo.toml"
rm -rf "$PKG"
mkdir -p "$PKG/DEBIAN" "$PKG/usr/bin" \
         "$PKG/usr/share/applications" \
         "$PKG/usr/share/icons/hicolor/256x256/apps"

install -m755 "$ROOT/target/release/cookie-launcher-gpui" "$PKG/usr/bin/cookie-launcher"
install -m644 "$ROOT/packaging/cookie-launcher.desktop" "$PKG/usr/share/applications/"
install -m644 "$ROOT/assets/cookie-logo.png" \
        "$PKG/usr/share/icons/hicolor/256x256/apps/cookie-launcher.png"

SIZE="$(du -sk "$PKG/usr" | awk '{print $1}')"
cat > "$PKG/DEBIAN/control" <<CONTROL
Package: cookie-launcher
Version: ${VERSION}
Section: games
Priority: optional
Architecture: ${ARCH}
Installed-Size: ${SIZE}
Maintainer: cookie-launcher <cookielauncher@users.noreply.github.com>
Depends: libxkbcommon0, libxkbcommon-x11-0, libxcb-xkb1, libfontconfig1, libfreetype6
Recommends: openjdk-17-jre | openjdk-21-jre | java-runtime
Homepage: https://github.com/cookie-launcher/cookie-launcher-gpui
Description: Cookie Launcher
 Acik kaynak Minecraft baslatici (GPUI, %100 Rust). Vanilla, Fabric, Quilt,
 Forge ve NeoForge profilleri; Modrinth'ten mod/shader/doku paketi ve modpack
 kurulumu; arkadaslarla LAN dunyasi paylasimi.
CONTROL

dpkg-deb --root-owner-group -Zxz --build "$PKG" "$DIST/cookie-launcher_${VERSION}_${ARCH}.deb" >/dev/null
echo "✓ $DIST/cookie-launcher_${VERSION}_${ARCH}.deb"
