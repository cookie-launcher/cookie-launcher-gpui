//! App-level components: the pieces that carry Cookie Launcher's product
//! language (cookie mark, pixel wordmark, caramel launch action, chips and the
//! beveled surfaces). Everything else composes from `gpui_kit::component`.

use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::theme::{Cocoa, RADIUS_CHIP, RADIUS_SURFACE};

pub const TEXT_10: Pixels = px(10.5);
pub const TEXT_11: Pixels = px(11.5);
pub const TEXT_12: Pixels = px(12.5);
pub const TEXT_13: Pixels = px(13.);
pub const TEXT_TITLE: Pixels = px(27.);

/// The voxel-adjacent cookie: an amber ring with crumb dots.
pub fn cookie_mark(size: f32, color: Hsla) -> impl IntoElement {
    let crumb = move |x: f32, y: f32, d: f32| {
        div()
            .absolute()
            .left(px(size * x))
            .top(px(size * y))
            .w(px(size * d))
            .h(px(size * d))
            .rounded_full()
            .bg(color)
    };
    div()
        .flex()
        .flex_shrink_0()
        .relative()
        .w(px(size))
        .h(px(size))
        .child(
            div()
                .absolute()
                .size_full()
                .rounded_full()
                .border_2()
                .border_color(color),
        )
        .child(crumb(0.30, 0.33, 0.13))
        .child(crumb(0.53, 0.22, 0.10))
        .child(crumb(0.63, 0.43, 0.14))
        .child(crumb(0.45, 0.60, 0.11))
        .child(crumb(0.24, 0.57, 0.09))
}

/// `COOKIE / LAUNCHER` in Silkscreen, the only place the pixel face is used.
pub fn wordmark(size: Pixels, cx: &App) -> impl IntoElement {
    wordmark_with(size, true, cx)
}

/// Compact form for the 218 px rail, where the slash overflows.
pub fn wordmark_compact(size: Pixels, cx: &App) -> impl IntoElement {
    wordmark_with(size, false, cx)
}

pub fn wordmark_with(size: Pixels, slash: bool, cx: &App) -> impl IntoElement {
    let cocoa = Cocoa::get(cx);
    let piece = |text: &'static str, color: Hsla| {
        div()
            .font_family("Silkscreen")
            .text_size(size)
            .font_weight(FontWeight::BOLD)
            .text_color(color)
            .child(text)
    };
    let mut mark = h_flex()
        .items_center()
        .min_w(px(0.))
        .child(piece("COOKIE", cx.theme().foreground));
    if slash {
        mark = mark.child(piece(" / ", cocoa.amber));
    } else {
        mark = mark.child(piece(" ", cx.theme().foreground));
    }
    mark.child(piece("LAUNCHER", cx.theme().foreground))
}

/// A raised cocoa surface: panel, card, or sidebar block.
pub fn surface(cx: &App) -> Div {
    let cocoa = Cocoa::get(cx);
    div()
        .rounded(px(RADIUS_SURFACE))
        .bg(cocoa.surface_1)
        .border_1()
        .border_color(cocoa.line)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChipTone {
    Neutral,
    Amber,
    Danger,
    Info,
}

pub fn chip(label: impl Into<SharedString>, tone: ChipTone, cx: &App) -> impl IntoElement {
    let cocoa = Cocoa::get(cx);
    let (fg, bg, border) = match tone {
        ChipTone::Neutral => (cocoa.text_3, cocoa.surface_2, cocoa.line),
        ChipTone::Amber => (cocoa.amber, cocoa.amber_soft, Hsla::from(rgba(0xe6a05261))),
        ChipTone::Danger => (cocoa.danger, Hsla::from(rgba(0xd76a5b1f)), Hsla::from(rgba(0xd76a5b61))),
        ChipTone::Info => (cocoa.info, Hsla::from(rgba(0x8fb0cf1f)), Hsla::from(rgba(0x8fb0cf61))),
    };
    div()
        .flex()
        .items_center()
        .gap_1()
        .h(px(22.))
        .px_2()
        .rounded(px(RADIUS_CHIP))
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_size(TEXT_10)
        .font_weight(FontWeight::MEDIUM)
        .text_color(fg)
        .child(label.into())
}

/// The seven-pixel connection / state dot with an optional halo ring.
pub fn status_dot(color: Hsla, halo: Option<Hsla>) -> impl IntoElement {
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .w(px(13.))
        .h(px(13.))
        .rounded_full()
        .when_some(halo, |this, halo| this.bg(halo))
        .child(div().w(px(7.)).h(px(7.)).rounded_full().bg(color))
}

/// A thin track meter (RAM allocation, install progress).
pub fn meter(ratio: f32, color: Hsla, cx: &App) -> impl IntoElement {
    let cocoa = Cocoa::get(cx);
    let ratio = ratio.clamp(0., 1.);
    div()
        .w_full()
        .h(px(5.))
        .rounded_full()
        .bg(cocoa.track)
        .overflow_hidden()
        .child(
            div()
                .h_full()
                .w(relative(ratio))
                .rounded_full()
                .bg(color),
        )
}

/// Quiet secondary line used for metadata across the app.
pub fn meta(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .text_size(TEXT_10)
        .text_color(Cocoa::get(cx).text_3)
        .child(text.into())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    Idle,
    Installing,
    Running,
    NoProfile,
}

/// The one caramel action: gradient fill, hard bottom edge, keycap hint.
pub fn launch_button(
    id: &'static str,
    label: String,
    keycap: &'static str,
    mode: LaunchMode,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    launch_button_at(
        id,
        label,
        keycap,
        mode,
        196.,
        false,
        on_click,
        |_, _, _| {},
        |_, _, _| {},
    )
}

/// Empty / unconfigured region: title, one explanatory line, optional action.
pub fn empty_state(
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    glyph: &'static str,
    cx: &App,
) -> impl IntoElement {
    let cocoa = Cocoa::get(cx);
    v_flex()
        .w_full()
        .min_h(px(180.))
        .items_center()
        .justify_center()
        .gap_3()
        .p_6()
        .rounded(px(RADIUS_SURFACE))
        .border_1()
        .border_color(cocoa.line)
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(46.))
                .h(px(46.))
                .rounded(px(6.))
                .bg(cocoa.surface_3)
                .border_1()
                .border_color(cocoa.line_strong)
                .text_size(px(18.))
                .text_color(cocoa.amber)
                .child(glyph),
        )
        .child(
            div()
                .text_size(px(15.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().foreground)
                .child(title.into()),
        )
        .child(
            div()
                .max_w(px(420.))
                .text_center()
                .text_size(TEXT_12)
                .text_color(cocoa.text_3)
                .child(body.into()),
        )
}

/// Page heading: 27 px title, one quiet subtitle line.
pub fn page_header(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    trailing: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    let cocoa = Cocoa::get(cx);
    h_flex()
        .w_full()
        .items_start()
        .justify_between()
        .gap_4()
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_size(TEXT_TITLE)
                        .font_weight(FontWeight::BOLD)
                        .text_color(cx.theme().foreground)
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_size(TEXT_12)
                        .text_color(cocoa.text_3)
                        .child(subtitle.into()),
                ),
        )
        .child(trailing)
}

/// `box_shadow` helper matching gpui's `BoxShadow` fields in order.
pub fn box_shadow(
    offset_x: f32,
    offset_y: f32,
    blur: f32,
    spread: f32,
    color: impl Into<Hsla>,
) -> BoxShadow {
    BoxShadow {
        color: color.into(),
        offset: point(px(offset_x), px(offset_y)),
        blur_radius: px(blur),
        spread_radius: px(spread),
        inset: false,
    }
}

/// A labelled form field: 11 px label above the control.
pub fn field(label: &'static str, control: impl IntoElement, cx: &App) -> impl IntoElement {
    v_flex()
        .gap_1()
        .child(
            div()
                .text_size(px(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(Cocoa::get(cx).text_2)
                .child(label),
        )
        .child(control)
}

/// Small bordered label ("Java Edition", "Çevrimdışı").
pub fn tag(label: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let cocoa = Cocoa::get(cx);
    div()
        .px(px(7.))
        .py(px(2.))
        .rounded(px(4.))
        .border_1()
        .border_color(cocoa.line_strong)
        .text_size(px(9.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cocoa.text_3)
        .child(label.into())
}

/// Window control button: 40×30, ghost, red hover for close.
pub fn window_control(
    id: &'static str,
    icon: IconName,
    close: bool,
    action: fn(&mut Window),
) -> impl IntoElement {
    let mut button = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(40.))
        .h(px(30.))
        .rounded(px(5.))
        .cursor_pointer()
        .child(Icon::new(icon).size(px(13.)).text_color(Hsla::from(rgb(0xa38b75))));
    if close {
        button = button
            .hover(|this| this.bg(rgb(0xa84337)).text_color(rgb(0xffffff)))
            .on_click(move |_, window, _| action(window));
    } else {
        button = button
            .hover(|this| this.bg(rgb(0x2c211b)))
            .on_click(move |_, window, _| action(window));
    }
    button
}

/// The hero's faint blueprint grid: 1 px cream lines on a fixed cell.
pub fn grid_background(width: f32, height: f32, cell: f32) -> impl IntoElement {
    let mut grid = div().absolute().inset_0();
    let mut x = cell;
    while x < width {
        grid = grid.child(
            div()
                .absolute()
                .left(px(x))
                .top_0()
                .w(px(1.))
                .h(px(height))
                .bg(rgba(0xffebd208)),
        );
        x += cell;
    }
    let mut y = cell;
    while y < height {
        grid = grid.child(
            div()
                .absolute()
                .top(px(y))
                .left_0()
                .h(px(1.))
                .w(px(width))
                .bg(rgba(0xffebd208)),
        );
        y += cell;
    }
    grid
}

/// A quiet 28 px action used inside panels.
pub fn quiet_button(id: &'static str, label: impl Into<SharedString>, cx: &App) -> Button {
    let cocoa = Cocoa::get(cx);
    Button::new(id)
        .ghost()
        .xsmall()
        .label(label.into())
        .text_color(cocoa.text_3)
}

/// Icon tile used by avatars, world rows and quick actions.
pub fn icon_tile(size: f32, radius: f32, glyph: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let cocoa = Cocoa::get(cx);
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .w(px(size))
        .h(px(size))
        .rounded(px(radius))
        .bg(cocoa.surface_3)
        .border_1()
        .border_color(cocoa.line_strong)
        .text_size(px(size * 0.42))
        .font_weight(FontWeight::BOLD)
        .text_color(cocoa.amber)
        .child(glyph.into())
}

/// Launch action with an explicit width, so the hero can size it.
pub fn launch_button_at(
    id: &'static str,
    label: String,
    keycap: &'static str,
    mode: LaunchMode,
    width: f32,
    pressed: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_press: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    on_release: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let mut button = h_flex()
        .id(id)
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .gap_2()
        .h(px(52.))
        .w(px(width))
        .px_4()
        .rounded(px(5.))
        .border_1()
        .text_size(px(16.))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .on_click(on_click)
        .on_mouse_down(MouseButton::Left, on_press)
        .on_mouse_up(MouseButton::Left, on_release);

    button = match mode {
        LaunchMode::Idle => button
            .bg(linear_gradient(
                180.,
                linear_color_stop(Hsla::from(rgb(0xf2b466)), 0.),
                linear_color_stop(Hsla::from(rgb(0xe6a052)), 1.),
            ))
            .border_color(rgb(0x9c6428))
            .text_color(rgb(0x241305))
            .shadow(vec![
                box_shadow(0., 4., 0., 0., rgb(0x6f471f)),
                box_shadow(0., 8., 18., 0., rgba(0x00000047)),
            ])
            .hover(|this| {
                this.bg(linear_gradient(
                    180.,
                    linear_color_stop(Hsla::from(rgb(0xf8c37c)), 0.),
                    linear_color_stop(Hsla::from(rgb(0xf2b466)), 1.),
                ))
                .border_color(rgb(0xb77835))
            }),
        LaunchMode::Installing => button
            .bg(rgb(0x2c211b))
            .border_color(Hsla::from(rgba(0xe6a05280)))
            .text_color(rgb(0xe6a052)),
        LaunchMode::Running => button
            .bg(rgb(0x8fb0cf))
            .border_color(rgb(0x6f8fa8))
            .text_color(rgb(0x07141d))
            .shadow(vec![box_shadow(0., 4., 0., 0., rgb(0x48647a))]),
        LaunchMode::NoProfile => button
            .bg(rgb(0xf4e7d3))
            .border_color(rgb(0xfff1dc))
            .text_color(rgb(0x1a0f07))
            .shadow(vec![box_shadow(0., 4., 0., 0., rgb(0x8a6a45))]),
    };

    let inner = button
        .child(Icon::new(IconName::Play).size(px(15.)))
        .child(label)
        .child(
            div()
                .flex_shrink_0()
                .px_1()
                .rounded(px(3.))
                .bg(Hsla::from(rgba(0x2413051f)))
                .text_size(px(8.))
                .font_family("JetBrains Mono")
                .child(keycap),
        );

    // CSS `.btn-mega-launch:active { transform: translateY(2px) }` over 80 ms.
    inner.press(SharedString::from(format!("launch-{id}-{pressed}")), pressed)
}

// ---------------------------------------------------------------------------
// Motion: 80–180 ms transitions, matching the web build's CSS timings
// ---------------------------------------------------------------------------

/// The web build's `ease` curve: cubic-bezier(0.25, 0.1, 0.25, 1).
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    // Cheap approximation that matches CSS `ease` closely enough.
    3.0 * t * t - 2.0 * t * t * t
}

/// Linear interpolation between two colors in HSLA space (hue wraps).
pub fn lerp_hsla(from: Hsla, to: Hsla, t: f32) -> Hsla {
    let t = t.clamp(0.0, 1.0);
    let mut dh = to.h - from.h;
    if dh > 0.5 {
        dh -= 1.0;
    } else if dh < -0.5 {
        dh += 1.0;
    }
    let h = (from.h + dh * t).rem_euclid(1.0);
    Hsla {
        h,
        s: from.s + (to.s - from.s) * t,
        l: from.l + (to.l - from.l) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

/// Builder methods for the web build's CSS transitions.
pub trait MotionExt: Sized {
    /// 120 ms background crossfade between two states (hover in/out).
    fn fade_bg(self, id: impl Into<ElementId>, from: Hsla, to: Hsla, ms: u64) -> AnyElement;
    /// Repeating 1.4 s skeleton shimmer.
    fn shimmer(self) -> AnyElement;
    /// Repeating 1.6 s expanding halo, for a busy status dot.
    fn pulse(self) -> AnyElement;
    /// 120 ms border-color crossfade (cards on hover).
    fn fade_border(self, id: impl Into<ElementId>, from: Hsla, to: Hsla, ms: u64) -> AnyElement;
    /// 120 ms background + border crossfade in one animation (cards).
    fn fade_card(
        self,
        id: impl Into<ElementId>,
        bg: (Hsla, Hsla),
        border: (Hsla, Hsla),
        ms: u64,
    ) -> AnyElement;
    /// 80 ms press offset (`translateY(2px)` on the caramel action).
    fn press(self, id: impl Into<ElementId>, pressed: bool) -> AnyElement;
}

impl<T> MotionExt for T
where
    T: IntoElement + Styled + 'static,
{
    fn fade_bg(self, id: impl Into<ElementId>, from: Hsla, to: Hsla, ms: u64) -> AnyElement {
        self.with_animation(
            id,
            Animation::new(std::time::Duration::from_millis(ms)).with_easing(ease),
            move |el: T, t: f32| el.bg(lerp_hsla(from, to, t)),
        )
        .into_any_element()
    }

    fn shimmer(self) -> AnyElement {
        self.with_animation(
            "skeleton-shimmer",
            Animation::new(std::time::Duration::from_millis(1400))
                .repeat()
                .with_easing(|t: f32| (t * std::f32::consts::PI).sin().abs()),
            |el: T, t: f32| el.opacity(0.35 + 0.45 * t),
        )
        .into_any_element()
    }

    fn pulse(self) -> AnyElement {
        self.with_animation(
            "status-pulse",
            Animation::new(std::time::Duration::from_millis(1600))
                .repeat()
                .with_easing(|t: f32| t),
            |el: T, t: f32| {
                el.opacity(1.0 - t * 0.85)
                    .w(px(7.0 + 10.0 * t))
                    .h(px(7.0 + 10.0 * t))
            },
        )
        .into_any_element()
    }

    fn fade_card(
        self,
        id: impl Into<ElementId>,
        bg: (Hsla, Hsla),
        border: (Hsla, Hsla),
        ms: u64,
    ) -> AnyElement {
        self.with_animation(
            id,
            Animation::new(std::time::Duration::from_millis(ms)).with_easing(ease),
            move |el: T, t: f32| {
                el.bg(lerp_hsla(bg.0, bg.1, t))
                    .border_color(lerp_hsla(border.0, border.1, t))
            },
        )
        .into_any_element()
    }

    fn fade_border(self, id: impl Into<ElementId>, from: Hsla, to: Hsla, ms: u64) -> AnyElement {
        self.with_animation(
            id,
            Animation::new(std::time::Duration::from_millis(ms)).with_easing(ease),
            move |el: T, t: f32| el.border_color(lerp_hsla(from, to, t)),
        )
        .into_any_element()
    }

    fn press(self, id: impl Into<ElementId>, pressed: bool) -> AnyElement {
        self.with_animation(
            id,
            Animation::new(std::time::Duration::from_millis(80)).with_easing(ease),
            move |el: T, t: f32| {
                let offset = if pressed { 2.0 * t } else { 2.0 * (1.0 - t) };
                el.mt(px(offset))
            },
        )
        .into_any_element()
    }
}

/// Status dot with the 1.6 s expanding halo the web build uses while busy.
pub fn status_dot_busy(color: Hsla) -> impl IntoElement {
    div()
        .relative()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .w(px(18.))
        .h(px(18.))
        .child(
            div()
                .absolute()
                .rounded_full()
                .bg(color)
                .pulse(),
        )
        .child(div().w(px(7.)).h(px(7.)).rounded_full().bg(color))
}
