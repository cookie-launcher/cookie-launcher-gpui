//! Console: the engine's own log, the honest record of what the core did.

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
    pub(crate) fn render_console(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let header = page_header(
            "Konsol",
            "Çekirdeğin günlüğü; indirme, kurulum ve oyun çıktısı burada.",
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("console-refresh")
                        .ghost()
                        .small()
                        .label("Yenile")
                        .icon(IconName::RefreshCw)
                        .on_click(cx.listener(|this, _, _, cx| this.load_logs(cx))),
                )
                .child(
                    Button::new("console-copy")
                        .ghost()
                        .small()
                        .label("Kopyala")
                        .icon(IconName::Copy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let text = this.logs.join("\n");
                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                            this.toast("Konsol çıktısı kopyalandı.", crate::app::ToastKind::Success, cx);
                        })),
                ),
            cx,
        );

        let body = if self.logs.is_empty() {
            empty_state(
                "Henüz çıktı yok",
                "Bir profil başlattığında çekirdek indirme ve kurulum adımlarını buraya yazar.",
                "▍",
                cx,
            )
            .into_any_element()
        } else {
            let mut lines = v_flex().w_full().gap(px(2.));
            for line in &self.logs {
                lines = lines.child(
                    div()
                        .w_full()
                        .text_size(px(11.5))
                        .font_family("JetBrains Mono")
                        .text_color(if line.contains('✗') || line.contains("hata") {
                            cocoa.danger
                        } else if line.contains('✓') {
                            cocoa.amber
                        } else {
                            cocoa.text_2
                        })
                        .child(line.clone()),
                );
            }
            v_flex()
                .id("console-output")
                .h_full()
                .min_h_0()
                .w_full()
                .overflow_y_scroll()
                .p_3()
                .gap(px(2.))
                .rounded(px(RADIUS_SURFACE))
                .bg(rgb(0x0d0907))
                .border_1()
                .border_color(cocoa.line)
                .child(lines)
                .into_any_element()
        };

        v_flex()
            .size_full()
            .p(px(20.))
            .gap_4()
            .child(header)
            .child(div().flex_1().min_h_0().w_full().child(body))
            .into_any_element()
    }
}
