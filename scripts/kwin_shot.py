import os, sys, threading, dbus
from dbus.mainloop.glib import DBusGMainLoop
DBusGMainLoop(set_as_default=True)

uuid = sys.argv[1]
out = sys.argv[2] if len(sys.argv) > 2 else "/tmp/kwin_cap.png"

bus = dbus.SessionBus()
obj = bus.get_object("org.kde.KWin", "/org/kde/KWin/ScreenShot2")
iface = dbus.Interface(obj, "org.kde.KWin.ScreenShot2")

r, w = os.pipe()
buf = bytearray()

def reader():
    while True:
        try:
            chunk = os.read(r, 1 << 20)
        except OSError:
            break
        if not chunk:
            break
        buf.extend(chunk)

t = threading.Thread(target=reader, daemon=True)
t.start()
try:
    res = iface.CaptureWindow(uuid, {"imageformat": dbus.Int32(4)}, dbus.types.UnixFd(w), timeout=20)
except Exception as e:
    print("ERR:", type(e).__name__, e)
    os.close(r)
    sys.exit(1)
os.close(w)
t.join(timeout=10)
os.close(r)
r_ = dict(res)
print("RESULT:", {k: v for k, v in r_.items()})
print("BYTES:", len(buf))
open("/tmp/raw.bin", "wb").write(bytes(buf))
width = int(r_.get("width", 0)); height = int(r_.get("height", 0))
stride = int(r_.get("stride", width * 4)); fmt = int(r_.get("format", 0))
if width and height and len(buf) >= stride * height:
    from PIL import Image
    img = Image.frombytes("RGBA", (width, height), bytes(buf), "raw", "RGBA", stride)
    img.save(out)
    print("SAVED:", out, img.size)
