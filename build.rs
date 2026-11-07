//! Build script: attach the application icon to the Windows executable.
//!
//! `assets/cookie-launcher.ico` is committed (multi-size, generated from
//! `assets/cookie-logo.png`), so the build never needs an image library. The
//! icon is compiled into a resource object by the MinGW resource compiler that
//! ships with the `x86_64-pc-windows-gnu` toolchain; when that compiler is not
//! on PATH the build carries on without an icon instead of failing.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/cookie-launcher.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let icon = Path::new("assets/cookie-launcher.ico");
    if !icon.exists() {
        return;
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let rc = out_dir.join("cookie-launcher.rc");
    let resource = out_dir.join("cookie-launcher.res");
    if std::fs::write(
        &rc,
        "1 ICON \"cookie-launcher.ico\"\n1 24 \"cookie-launcher.manifest\"\n",
    )
    .is_err()
    {
        return;
    }
    let _ = std::fs::copy(icon, out_dir.join("cookie-launcher.ico"));
    let _ = std::fs::write(out_dir.join("cookie-launcher.manifest"), MANIFEST);

    let Some(compiler) = find("windres").or_else(|| find("x86_64-w64-mingw32-windres")) else {
        println!("cargo:warning=windres not found; building without the window icon");
        return;
    };

    let status = Command::new(compiler)
        .current_dir(&out_dir)
        .arg(&rc)
        .args(["-O", "coff"])
        .arg(&resource)
        .status();
    match status {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg={}", resource.display());
        }
        _ => println!("cargo:warning=window icon could not be compiled"),
    }
}

/// Windows 10+ / DPI aware, and asks for the common controls version so the
/// window frame matches the system theme.
const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity type="win32" name="CookieLauncher" version="4.0.0.0" processorArchitecture="amd64"/>
  <application xmlns="urn:schemas-microsoft-com:asm.v3">
    <windowsSettings>
      <dpiAwareness xmlns="http://schemas.microsoft.com/SMI/2016/WindowsSettings">PerMonitorV2</dpiAwareness>
      <activeCodePage xmlns="http://schemas.microsoft.com/SMI/2019/WindowsSettings">UTF-8</activeCodePage>
    </windowsSettings>
  </application>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
    </application>
  </compatibility>
</assembly>
"#;

fn find(program: &str) -> Option<PathBuf> {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let name = format!("{program}{suffix}");
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .map(|dir| dir.join(&name))
        .find(|candidate| candidate.is_file())
}
