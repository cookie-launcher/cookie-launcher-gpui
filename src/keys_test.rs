//! Real key dispatch: headless window, real `KeyBinding`s, real handlers.
//!
//! Uses GPUI's own test support (`gpui-pre` with `test-support`), which lets us
//! send keystrokes through the same path a physical keyboard takes — the piece
//! the environment cannot exercise otherwise (no `wtype`/`ydotool`).

use gpui_test::{px, size, AppContext as _, Focusable as _, TestAppContext, WindowHandle};

use crate::app::{AppView, Tab};


fn open_app(
    cx: &mut TestAppContext,
) -> (gpui_test::AnyWindowHandle, gpui_test::Entity<AppView>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::theme::install(cx);
        crate::app::init_keybindings(cx);
    });
    cx.update(|cx| {
        let (handle, view) = gpui_kit::open_window(
            gpui_test::WindowOptions {
                window_bounds: Some(gpui_test::WindowBounds::Windowed(
                    gpui_test::Bounds::new(gpui_test::point(px(0.), px(0.)), size(px(1120.), px(740.))),
                )),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| AppView::test_view(window, cx)),
        )
        .expect("headless window");
        (handle, view)
    })
}

#[gpui_test::test]
fn ctrl_number_switches_tabs(cx: &mut TestAppContext) {
    let (window, view) = open_app(cx);

    cx.simulate_keystrokes(window.into(), "ctrl-5");
    cx.update(|cx| assert_eq!(view.read(cx).tab, Tab::Console, "Ctrl+5 → Konsol"));

    cx.simulate_keystrokes(window.into(), "ctrl-4");
    cx.update(|cx| assert_eq!(view.read(cx).tab, Tab::Gallery, "Ctrl+4 → Galeri"));

    cx.simulate_keystrokes(window.into(), "ctrl-1");
    cx.update(|cx| assert_eq!(view.read(cx).tab, Tab::Play, "Ctrl+1 → Oyna"));
}

#[gpui_test::test]
fn unbound_key_does_not_change_tab(cx: &mut TestAppContext) {
    let (window, view) = open_app(cx);
    cx.simulate_keystrokes(window.into(), "ctrl-3");
    cx.update(|cx| assert_eq!(view.read(cx).tab, Tab::Modrinth));
    // A chord with no binding must leave the tab alone.
    cx.simulate_keystrokes(window.into(), "ctrl-9");
    cx.update(|cx| assert_eq!(view.read(cx).tab, Tab::Modrinth));
}

#[gpui_test::test]
fn ctrl_k_opens_modrinth_and_focuses_search(cx: &mut TestAppContext) {
    let (window, view) = open_app(cx);
    cx.simulate_keystrokes(window.into(), "ctrl-k");
    cx.update(|cx| assert_eq!(view.read(cx).tab, Tab::Modrinth, "Ctrl+K → Modlar"));
    cx.update_window(window, |_, window, cx| {
        let handle = view.read(cx).search_input.read(cx).focus_handle(cx);
        assert!(
            handle.is_focused(window),
            "Ctrl+K arama alanına odaklanmalı"
        );
    })
    .expect("window update");
}

#[gpui_test::test]
fn shortcuts_still_work_after_a_tab_switch(cx: &mut TestAppContext) {
    // Regression: switching tabs unmounted the focused input, which dropped the
    // key context and made every later shortcut dead until a click.
    let (window, view) = open_app(cx);
    cx.simulate_keystrokes(window.into(), "ctrl-5");
    cx.simulate_keystrokes(window.into(), "ctrl-6");
    cx.update(|cx| assert_eq!(view.read(cx).tab, Tab::Friends));
    cx.simulate_keystrokes(window.into(), "ctrl-2");
    cx.update(|cx| assert_eq!(view.read(cx).tab, Tab::Instances));
}
