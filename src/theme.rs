//! The product token layer: fetches the bundled fonts and installs the
//! "Cocoa Craft" theme, then exposes every palette value that has no semantic
//! role in the shared theme as [`Cocoa`].
//!
//! Raw color values live only here. Feature code asks for `Cocoa::get(cx)`
//! and names the role it needs.

use std::borrow::Cow;
use std::rc::Rc;

use gpui_kit::component::theme::{Theme, ThemeConfig, ThemeSet};
use gpui_kit::*;

const THEME_JSON: &str = include_str!("../assets/theme/cocoa-craft.json");

/// Radii beyond the theme's `radius` / `radius_lg` tiers.
pub const RADIUS_CHIP: f32 = 3.0;
pub const RADIUS_SURFACE: f32 = 8.0;
pub const RADIUS_MODAL: f32 = 10.0;

static FONTS: &[&[u8]] = &[
    include_bytes!("../assets/fonts/PlusJakartaSans-Regular.ttf"),
    include_bytes!("../assets/fonts/PlusJakartaSans-Medium.ttf"),
    include_bytes!("../assets/fonts/PlusJakartaSans-SemiBold.ttf"),
    include_bytes!("../assets/fonts/PlusJakartaSans-Bold.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
    include_bytes!("../assets/fonts/Silkscreen-Regular.ttf"),
    include_bytes!("../assets/fonts/Silkscreen-Bold.ttf"),
];

/// Palette roles the launcher uses that the shared theme does not name.
pub struct Cocoa {
    pub amber: Hsla,
    pub amber_hover: Hsla,
    pub amber_ink: Hsla,
    pub amber_soft: Hsla,
    pub paper: Hsla,
    pub paper_hover: Hsla,
    pub paper_ink: Hsla,
    pub chrome: Hsla,
    pub surface_1: Hsla,
    pub surface_2: Hsla,
    pub surface_3: Hsla,
    pub track: Hsla,
    pub line: Hsla,
    pub line_strong: Hsla,
    pub text_2: Hsla,
    pub text_3: Hsla,
    pub launch_top: Hsla,
    pub launch_bottom: Hsla,
    pub launch_edge: Hsla,
    pub danger: Hsla,
    pub info: Hsla,
}

impl Global for Cocoa {}

impl Cocoa {
    pub fn get(cx: &App) -> &Cocoa {
        cx.global::<Cocoa>()
    }

    fn new() -> Self {
        Self {
            amber: rgb(0xe6a052).into(),
            amber_hover: rgb(0xf4b96e).into(),
            amber_ink: rgb(0x261506).into(),
            amber_soft: Hsla::from(rgba(0xe6a05233)),
            paper: rgb(0xf4e7d3).into(),
            paper_hover: rgb(0xfffaef).into(),
            paper_ink: rgb(0x1a0f07).into(),
            chrome: rgb(0x15100c).into(),
            surface_1: rgb(0x1a1310).into(),
            surface_2: rgb(0x221915).into(),
            surface_3: rgb(0x2c211b).into(),
            track: rgb(0x382a22).into(),
            line: rgb(0x33251d).into(),
            line_strong: rgb(0x4a3529).into(),
            text_2: rgb(0xd7c4b0).into(),
            text_3: rgb(0xa38b75).into(),
            launch_top: rgb(0xf2b466).into(),
            launch_bottom: rgb(0xe6a052).into(),
            launch_edge: rgb(0x6f471f).into(),
            danger: rgb(0xd76a5b).into(),
            info: rgb(0x8fb0cf).into(),
        }
    }

    /// The signature raised-surface bevel: a 1 px cream highlight over a 1 px
    /// shade, no blur.
    pub fn bevel_shadows(&self) -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: Hsla::from(rgba(0xffebd20d)),
                offset: point(px(0.), px(1.)),
                blur_radius: px(0.),
                spread_radius: px(0.),
                inset: true,
            },
            BoxShadow {
                color: Hsla::from(rgba(0x0000004d)),
                offset: point(px(0.), px(-1.)),
                blur_radius: px(0.),
                spread_radius: px(0.),
                inset: true,
            },
        ]
    }
}

/// Install fonts and the Cocoa Craft theme. Call after `gpui_kit::init`.
pub fn install(cx: &mut App) {
    let fonts: Vec<Cow<'static, [u8]>> = FONTS.iter().map(|bytes| Cow::Borrowed(*bytes)).collect();
    if let Err(error) = cx.text_system().add_fonts(fonts) {
        eprintln!("fonts: {error}");
    }

    let set: ThemeSet = match serde_json::from_str(THEME_JSON) {
        Ok(set) => set,
        Err(error) => {
            eprintln!("theme: {error}");
            return;
        }
    };
    if let Some(config) = set.themes.into_iter().next() {
        let config = Rc::new(config);
        Theme::update(cx, |theme| theme.apply_config(&config));
    }
    cx.set_global(Cocoa::new());
}

/// A themed `ThemeConfig` for the dialog/overlay layer, if a caller needs the
/// raw config (used by settings surfaces).
pub fn theme_config() -> Option<ThemeConfig> {
    serde_json::from_str::<ThemeSet>(THEME_JSON)
        .ok()
        .and_then(|set| set.themes.into_iter().next())
}
