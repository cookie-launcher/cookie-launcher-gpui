#!/bin/bash
# cap.sh <out.png> — capture the CookieLauncher window without raising it.
# KWin's ScreenShot2 CaptureWindow renders the window off-screen: no focus
# change, no stacking change, works while it is behind other windows.
OUT="${1:-/tmp/cl.png}"
cat > /tmp/cl_id.js <<'JS'
const wins = workspace.windowList();
for (let i = 0; i < wins.length; i++) {
  const w = wins[i];
  if (w.resourceClass === "cookie-launcher-gpui" || (w.caption && w.caption.indexOf("CookieLauncher") === 0)) { print("CLID " + w.internalId); break; }
}
JS
qdbus6 org.kde.KWin /Scripting org.kde.kwin.Scripting.unloadScript cl-id >/dev/null 2>&1
qdbus6 org.kde.KWin /Scripting org.kde.kwin.Scripting.loadScript /tmp/cl_id.js cl-id >/dev/null 2>&1
qdbus6 org.kde.KWin /Scripting org.kde.kwin.Scripting.start >/dev/null 2>&1
sleep 0.7
UUID=$(journalctl --user --since "10 sec ago" 2>/dev/null | grep -a "CLID " | tail -1 | awk '{print $NF}')
qdbus6 org.kde.KWin /Scripting org.kde.kwin.Scripting.unloadScript cl-id >/dev/null 2>&1
if [ -z "$UUID" ]; then echo "no CookieLauncher window"; exit 1; fi
python3 $(dirname "$0")/kwin_shot.py "$UUID" "$OUT" | tail -1
