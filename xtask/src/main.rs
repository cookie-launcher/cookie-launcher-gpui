//! Dev helper for the screenshot loop: capture the launcher window through
//! KWin's `ScreenShot2.CaptureWindow`, which renders the window off-screen —
//! no focus change, no stacking change, works while it sits behind everything.
//!
//! ```text
//! cargo run -p xtask -- capture out.png
//! cargo run -p xtask -- tab modrinth out.png
//! ```
//!
//! KWin, D-Bus and the journal are Linux-only, so the capture code only exists
//! there. On Windows the binary is a stub, which keeps the workspace (and
//! `cargo build --all-targets`) compiling on every platform.

#[cfg(target_os = "linux")]
mod capture;

fn main() {
    #[cfg(target_os = "linux")]
    {
        capture::main();
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("xtask: pencere yakalama yalnızca Linux + KWin üzerinde çalışır.");
        std::process::exit(2);
    }
}
