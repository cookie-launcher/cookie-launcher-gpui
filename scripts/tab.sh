#!/bin/bash
# tab.sh <tab|-> <out.png> — fresh app instance on one tab, then window capture.
TAB="$1"; OUT="$2"
pkill -x cookie-launcher 2>/dev/null
for _ in $(seq 40); do pgrep -x cookie-launcher >/dev/null || break; sleep 0.2; done
sleep 1
cd /home/emre/Projeler/cookie-launcher-gpui || exit 1
if [ "$TAB" = "-" ]; then
  setsid ./target/debug/cookie-launcher-gpui > "/tmp/gpui-main.log" 2>&1 &
else
  CL_TAB="$TAB" setsid ./target/debug/cookie-launcher-gpui > "/tmp/gpui-$TAB.log" 2>&1 &
fi
sleep 12
/tmp/cap.sh "$OUT"
