Cookie Launcher — açık kaynak Minecraft başlatıcı (GPUI, %100 Rust; webview yok, Python yok).

**Paketler**
- `*.AppImage` — çalıştırılabilir tek dosya (`chmod +x` sonrası doğrudan çalışır)
- `*.deb` — Debian/Ubuntu: `sudo apt install ./cookie-launcher_*_amd64.deb`
- `*.tar.gz` — taşınabilir klasör, içinde `install.sh`
- `*-windows-x64.zip` / `*-windows-x64.exe` — Windows 10/11 x64, kurulum gerekmez
  (VC++ 2015-2022 Redistributable x64 gerekir)

Gereksinimler: Linux (X11 veya Wayland), Vulkan/OpenGL sürücüsü, Minecraft için Java 8–21
(launcher sürüme uygun JVM'i kendisi seçer).

Mod kaynağı Modrinth'tir. Minecraft, Mojang Synergies AB'nin ticari markasıdır;
bu proje Mojang/Microsoft ile bağlantılı veya onaylı değildir.
