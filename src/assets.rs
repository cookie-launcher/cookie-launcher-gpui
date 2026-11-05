//! Asset source: the cookie-block artwork from the Tauri build plus the
//! GPUI Kit icon catalog.

use std::borrow::Cow;

use gpui::AssetSource;
use gpui_kit::*;

const COOKIE_BLOCK: &[u8] = include_bytes!("../assets/cookie-block.svg");
const COOKIE_LOGO: &[u8] = include_bytes!("../assets/cookie-logo.png");
const COOKIE_BLOCK_PNG: &[u8] = include_bytes!("../assets/cookie-block.png");

pub struct AppAssets;

impl AssetSource for AppAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match path {
            "cookie-block.svg" => return Ok(Some(Cow::Borrowed(COOKIE_BLOCK))),
            "cookie-logo.png" => return Ok(Some(Cow::Borrowed(COOKIE_LOGO))),
            "cookie-block.png" => return Ok(Some(Cow::Borrowed(COOKIE_BLOCK_PNG))),
            _ => {}
        }
        // The Kit bundle only ships a small default icon set; the rest of the
        // Lucide catalog used by this UI is embedded here under the same paths.
        if let Some(name) = path.strip_prefix("icons/") {
            if let Some(bytes) = embedded_icon(name) {
                return Ok(Some(Cow::Borrowed(bytes)));
            }
        }
        <gpui_kit::assets::Assets as AssetSource>::load(&gpui_kit::assets::Assets, path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        <gpui_kit::assets::Assets as AssetSource>::list(&gpui_kit::assets::Assets, path)
    }
}

fn embedded_icon(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "blocks.svg" => include_bytes!("../assets/icons/blocks.svg"),
        "image.svg" => include_bytes!("../assets/icons/image.svg"),
        "terminal.svg" => include_bytes!("../assets/icons/terminal.svg"),
        "users.svg" => include_bytes!("../assets/icons/users.svg"),
        "zap.svg" => include_bytes!("../assets/icons/zap.svg"),
        "message-square.svg" => include_bytes!("../assets/icons/message-square.svg"),
        "x.svg" => include_bytes!("../assets/icons/x.svg"),
        "minus.svg" => include_bytes!("../assets/icons/minus.svg"),
        "square.svg" => include_bytes!("../assets/icons/square.svg"),
        "user.svg" => include_bytes!("../assets/icons/user.svg"),
        "download.svg" => include_bytes!("../assets/icons/download.svg"),
        "settings.svg" => include_bytes!("../assets/icons/settings.svg"),
        "folder-open.svg" => include_bytes!("../assets/icons/folder-open.svg"),
        "play.svg" => include_bytes!("../assets/icons/play.svg"),
        "folder.svg" => include_bytes!("../assets/icons/folder.svg"),
        "copy.svg" => include_bytes!("../assets/icons/copy.svg"),
        "refresh-cw.svg" => include_bytes!("../assets/icons/refresh-cw.svg"),
        "search.svg" => include_bytes!("../assets/icons/search.svg"),
        "plus.svg" => include_bytes!("../assets/icons/plus.svg"),
        "chevron-down.svg" => include_bytes!("../assets/icons/chevron-down.svg"),
        "check.svg" => include_bytes!("../assets/icons/check.svg"),
        "circle-check.svg" => include_bytes!("../assets/icons/circle-check.svg"),
        "circle-alert.svg" => include_bytes!("../assets/icons/circle-alert.svg"),
        "triangle-alert.svg" => include_bytes!("../assets/icons/triangle-alert.svg"),
        "link.svg" => include_bytes!("../assets/icons/link.svg"),
        "external-link.svg" => include_bytes!("../assets/icons/external-link.svg"),
        _ => return None,
    })
}
