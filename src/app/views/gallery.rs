//! Gallery: screenshots the engine wrote. Names and sizes are real; the image
//! bytes are served by the core over loopback and are not decoded here yet.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    Sizable as _,
    button::{Button, ButtonVariants as _},
};
use gpui_kit::assets::IconName;
use gpui_kit::*;

use crate::app::AppView;
use crate::theme::{Cocoa, RADIUS_SURFACE};
use crate::widgets::*;

impl AppView {
    pub(crate) fn render_gallery(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let header = page_header(
            "Galeri",
            "Oyun içinde çektiğin ekran görüntüleri.",
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("gallery-folder")
                        .ghost()
                        .small()
                        .label("Klasörü aç")
                        .icon(IconName::FolderOpen)
                        .on_click(cx.listener(|this, _, _, cx| this.open_folder("screenshots", cx))),
                )
                .child(
                    Button::new("gallery-refresh")
                        .ghost()
                        .small()
                        .label("Yenile")
                        .icon(IconName::RefreshCw)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_screenshots(cx))),
                ),
            cx,
        );

        let body = if self.screenshots.is_empty() {
            empty_state(
                "Henüz ekran görüntüsü yok",
                "Oyun içinde F2'ye bastığında görüntüler burada listelenir.",
                "▣",
                cx,
            )
            .into_any_element()
        } else {
            let mut grid = h_flex().w_full().flex_wrap().gap_3();
            for shot in &self.screenshots {
                let shot_name = shot.name.clone();
                grid = grid.child(
                    v_flex()
                        .id(SharedString::from(format!("shot-{}", shot.name)))
                        .w(px(220.))
                        .gap_2()
                        .p_2()
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_lightbox(shot_name.clone(), cx)
                        }))
                        .rounded(px(RADIUS_SURFACE))
                        .bg(cocoa.surface_1)
                        .border_1()
                        .border_color(cocoa.line)
                        .child(match self.thumbs.get(&shot.name) {
                            Some(image) => img(ImageSource::Image(image.clone()))
                                .w_full()
                                .h(px(120.))
                                .rounded(px(5.))
                                .object_fit(ObjectFit::Cover)
                                .into_any_element(),
                            None => div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .w_full()
                                .h(px(120.))
                                .rounded(px(5.))
                                .bg(rgb(0x0d0907))
                                .border_1()
                                .border_color(cocoa.line)
                                .text_size(px(20.))
                                .text_color(cocoa.line_strong)
                                .child("▣")
                                .into_any_element(),
                        })
                        .child(
                            div()
                                .text_size(TEXT_11)
                                .text_color(cocoa.text_2)
                                .truncate()
                                .child(shot.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(TEXT_10)
                                .font_family("JetBrains Mono")
                                .text_color(cocoa.text_3)
                                .child(format_size(shot.size)),
                        ),
                );
            }
            grid.into_any_element()
        };

        v_flex()
            .id("gallery")
            .size_full()
            .overflow_y_scroll()
            .p(px(20.))
            .gap_4()
            .child(header)
            .child(body)

            .into_any_element()
    }
}

fn format_size(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    if bytes > 1_048_576. {
        format!("{:.1} MB", bytes / 1_048_576.)
    } else {
        format!("{:.0} KB", bytes / 1024.)
    }
}
