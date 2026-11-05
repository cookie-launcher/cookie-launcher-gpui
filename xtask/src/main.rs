//! Dev helper for the screenshot loop: capture the launcher window through
//! KWin's `ScreenShot2.CaptureWindow`, which renders the window off-screen —
//! no focus change, no stacking change, works while it sits behind everything.
//!
//! ```text
//! cargo run -p xtask -- capture out.png
//! cargo run -p xtask -- tab modrinth out.png
//! ```
//!
//! Replaces the earlier `scripts/capture.sh` + `scripts/kwin_shot.py` pair.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use zbus::blocking::Connection;
use zbus::names::{BusName, InterfaceName, MemberName};
use zbus::zvariant::{ObjectPath, Value};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("capture") => {
            let out = args.get(1).map(String::as_str).unwrap_or("shot.png");
            capture(out);
        }
        Some("tab") => {
            let tab = args.get(1).map(String::as_str).unwrap_or("");
            let out = args.get(2).map(String::as_str).unwrap_or("shot.png");
            restart(tab);
            capture(out);
        }
        _ => {
            eprintln!("usage: xtask capture <out.png>");
            eprintln!("       xtask tab <tab|-> <out.png>");
            std::process::exit(2);
        }
    }
}

fn restart(tab: &str) {
    let _ = Command::new("pkill").arg("-x").arg("cookie-launcher").status();
    std::thread::sleep(Duration::from_millis(600));
    let mut command = Command::new("setsid");
    command.arg("./target/debug/cookie-launcher-gpui");
    if !tab.is_empty() && tab != "-" {
        command.env("CL_TAB", tab);
    }
    let _ = command.spawn();
    std::thread::sleep(Duration::from_secs(9));
}

fn capture(out: &str) {
    let Some(uuid) = window_id() else {
        eprintln!("CookieLauncher penceresi bulunamadı");
        std::process::exit(1);
    };

    let (mut reader, writer) = std::io::pipe().expect("pipe");

    // KWin writes the raw frame into the pipe while the method call is still in
    // flight; a full pipe buffer would deadlock, so drain it concurrently.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut data = Vec::new();
        let _ = reader.read_to_end(&mut data);
        let _ = tx.send(data);
    });

    let connection = Connection::session().expect("D-Bus oturumu");
    let destination: Option<BusName> = Some("org.kde.KWin".try_into().expect("bus adı"));
    let path = ObjectPath::try_from("/org/kde/KWin/ScreenShot2").expect("yol");
    let interface: Option<InterfaceName> =
        Some("org.kde.KWin.ScreenShot2".try_into().expect("arayüz"));
    let method: MemberName = "CaptureWindow".try_into().expect("metot");

    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("imageformat", Value::from(4i32));

    let fd = zvariant::OwnedFd::from(OwnedFd::from(writer));
    let reply = connection
        .call_method(destination, path, interface, method, &(uuid, options, fd))
        .expect("CaptureWindow çağrısı");
    let body = reply.body();
    let result: HashMap<String, Value> = body.deserialize().expect("sonuç haritası");
    drop(reply);

    let width = number(&result, "width");
    let height = number(&result, "height");
    let stride = number(&result, "stride").max(width * 4);
    let data = rx.recv_timeout(Duration::from_secs(10)).unwrap_or_default();

    if width == 0 || height == 0 || data.len() < stride * height {
        eprintln!(
            "görüntü alınamadı: {}x{} stride={} bytes={}",
            width,
            height,
            stride,
            data.len()
        );
        std::process::exit(1);
    }

    // The pipe carries RGBA8888 rows of `stride` bytes.
    let mut pixels = Vec::with_capacity(width * height * 4);
    for row in 0..height {
        let start = row * stride;
        pixels.extend_from_slice(&data[start..start + width * 4]);
    }
    let buffer = image::RgbaImage::from_raw(width as u32, height as u32, pixels).expect("piksel");
    buffer.save(out).expect("png yazılamadı");
    println!("saved {out} ({width}x{height})");
}

fn number(map: &HashMap<String, Value>, key: &str) -> usize {
    map.get(key)
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or(0) as usize
}

/// Ask KWin for the launcher's internal id through a throwaway scripting
/// snippet; the value is printed into the user journal.
fn window_id() -> Option<String> {
    const SCRIPT: &str = r#"const wins = workspace.windowList();
for (let i = 0; i < wins.length; i++) {
  const w = wins[i];
  if (w.resourceClass === "cookie-launcher-gpui" || (w.caption && w.caption.indexOf("CookieLauncher") === 0)) {
    print("CLID " + w.internalId);
    break;
  }
}
"#;
    std::fs::write("/tmp/cl_id.js", SCRIPT).ok()?;
    let qdbus = |args: &[&str]| {
        let _ = Command::new("qdbus6").args(args).output();
    };
    qdbus(&[
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting.unloadScript",
        "cl-id",
    ]);
    qdbus(&[
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting.loadScript",
        "/tmp/cl_id.js",
        "cl-id",
    ]);
    qdbus(&["org.kde.KWin", "/Scripting", "org.kde.kwin.Scripting.start"]);
    std::thread::sleep(Duration::from_millis(800));
    qdbus(&[
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting.unloadScript",
        "cl-id",
    ]);

    let journal = Command::new("journalctl")
        .args(["--user", "--since", "10 sec ago"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&journal.stdout);
    let line = text.lines().filter(|line| line.contains("CLID ")).last()?;
    let index = line.find("CLID ")? + "CLID ".len();
    let id = line[index..].trim().to_string();
    if id.starts_with('{') {
        Some(id)
    } else {
        None
    }
}

#[allow(dead_code)]
fn flush(mut writer: impl Write) {
    let _ = writer.flush();
}
