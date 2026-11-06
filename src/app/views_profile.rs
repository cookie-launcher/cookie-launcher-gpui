//! Profil detay sayfası — the per-profile page the Tauri build called `pds-*`.
//!
//! One profile, everything about it: header with the launch action, installed
//! content split by kind, worlds, and the tail of the log. Reached by clicking
//! a profile card; the back button returns to the list.

use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{AppView, Tab, ToastKind};
use crate::theme::Cocoa;
use crate::theme::RADIUS_SURFACE;
use crate::widgets::*;

impl AppView {
    pub(crate) fn render_profile(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let foreground = cx.theme().foreground;

        let Some(instance) = self.active_instance().cloned() else {
            return empty_state(
                "Profil seçili değil",
                "Profiller sekmesinden bir profilin üstüne tıkla.",
                "👤",
                cx,
            )
            .into_any_element();
        };

        let back = Button::new("pds-back")
            .ghost()
            .small()
            .label("Geri")
            .icon(IconName::ArrowLeft)
            .on_click(cx.listener(|this, _, _, cx| {
                this.tab = Tab::Instances;
                this.refresh_instances(cx);
            }));

        let title = div()
            .text_size(px(24.))
            .font_weight(FontWeight::BOLD)
            .text_color(foreground)
            .child(instance.name.clone());

        let facts = h_flex()
            .items_center()
            .gap_2()
            .child(chip(&instance.version, ChipTone::Neutral, cx))
            .child(chip(instance.loader_label(), ChipTone::Neutral, cx))
            .child(chip(
                format!("{} mod", self.content_counts[0]),
                ChipTone::Neutral,
                cx,
            ))
            .child(chip(
                format!("{} dünya", self.worlds.len()),
                ChipTone::Neutral,
                cx,
            ))
            .when(!instance.loader_version.is_empty(), |this| {
                this.child(chip(
                    format!("{} {}", instance.loader_label(), instance.loader_version),
                    ChipTone::Amber,
                    cx,
                ))
            });

        let header = v_flex()
            .w_full()
            .gap_3()
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(back)
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(title)
                                    .child(facts),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("pds-folder")
                                    .small()
                                    .outline()
                                    .label("Klasör")
                                    .icon(IconName::Folder)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_active_folder(cx);
                                    })),
                            )
                            .child(
                                Button::new("pds-launch")
                                    .small()
                                    .primary()
                                    .label("Başlat")
                                    .icon(IconName::Play)
                                    .on_click(cx.listener(|this, _, _, cx| this.launch(cx))),
                            ),
                    ),
            );

        // ---- installed content -------------------------------------------
        let kinds = [
            ("mod", "Modlar", self.content_counts[0]),
            ("shader", "Shaderlar", self.content_counts[1]),
            ("resourcepack", "Doku paketleri", self.content_counts[2]),
        ];
        let mut kind_pills = h_flex().gap_2();
        for (kind, label, count) in kinds {
            let selected = self.content_kind == kind;
            kind_pills = kind_pills.child(
                div()
                    .id(SharedString::from(format!("pds-kind-{kind}")))
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(28.))
                    .px(px(10.))
                    .rounded(px(5.))
                    .bg(if selected { cocoa.surface_3 } else { cocoa.surface_1 })
                    .border_1()
                    .border_color(if selected { cocoa.line_strong } else { cocoa.line })
                    .text_size(TEXT_11)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if selected { foreground } else { cocoa.text_3 })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.content_kind != kind {
                            this.content_kind = kind.to_string();
                            this.refresh_content(cx);
                        }
                    }))
                    .child(label)
                    .when(count > 0, |this| {
                        this.child(
                            div()
                                .text_size(px(10.))
                                .font_family("JetBrains Mono")
                                .text_color(cocoa.amber)
                                .child(count.to_string()),
                        )
                    }),
            );
        }

        let mut content_rows = v_flex().w_full().gap_1();
        if self.content_items.is_empty() {
            content_rows = content_rows.child(
                div()
                    .py_3()
                    .text_size(TEXT_12)
                    .text_color(cocoa.text_3)
                    .child("Bu kategoride içerik yok."),
            );
        }
        for item in self.content_items.clone() {
            let name = item.name.clone();
            content_rows = content_rows.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded(px(5.))
                    .bg(cocoa.surface_2)
                    .border_1()
                    .border_color(cocoa.line)
                    .child(
                        div()
                            .flex_shrink_0()
                            .w(px(6.))
                            .h(px(6.))
                            .rounded_full()
                            .bg(if item.enabled { cocoa.amber } else { cocoa.line_strong }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(TEXT_11)
                            .font_family("JetBrains Mono")
                            .text_color(if item.enabled { cocoa.text_2 } else { cocoa.text_3 })
                            .child(name.clone()),
                    )
                    .child(
                        Button::new(SharedString::from(format!("pds-toggle-{name}")))
                            .xsmall()
                            .ghost()
                            .label(if item.enabled { "Devre dışı" } else { "Etkinleştir" })
                            .on_click(cx.listener({
                                let name = name.clone();
                                move |this, _, _, cx| this.toggle_content(name.clone(), cx)
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("pds-remove-{name}")))
                            .xsmall()
                            .ghost()
                            .label("Sil")
                            .on_click(cx.listener({
                                let name = name.clone();
                                move |this, _, _, cx| this.delete_content(name.clone(), cx)
                            })),
                    ),
            );
        }

        let content_panel = v_flex()
            .w_full()
            .gap_3()
            .p_4()
            .rounded(px(RADIUS_SURFACE))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(14.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(foreground)
                            .child("İçerik"),
                    )
                    .child(kind_pills),
            )
            .child(content_rows);

        // ---- worlds -------------------------------------------------------
        let mut world_rows = v_flex().w_full().gap_1();
        if self.worlds.is_empty() {
            world_rows = world_rows.child(
                div()
                    .py_3()
                    .text_size(TEXT_12)
                    .text_color(cocoa.text_3)
                    .child("Bu profilde henüz dünya yok."),
            );
        }
        for world in self.worlds.iter().take(6) {
            let name = world.name.clone();
            world_rows = world_rows.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .rounded(px(5.))
                    .bg(cocoa.surface_2)
                    .border_1()
                    .border_color(cocoa.line)
                    .child(icon_tile(26., 5., "W", cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(TEXT_12)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cocoa.text_2)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(TEXT_10)
                                    .text_color(cocoa.text_3)
                                    .child(format!(
                                        "{} · {:.1} MB",
                                        world.last_played, world.size_mb
                                    )),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("pds-world-{}", world.name)))
                            .xsmall()
                            .ghost()
                            .label("Yedekle")
                            .on_click(cx.listener({
                                let name = world.name.clone();
                                move |this, _, _, cx| this.backup_world(name.clone(), cx)
                            })),
                    ),
            );
        }

        let worlds_panel = v_flex()
            .w_full()
            .gap_3()
            .p_4()
            .rounded(px(RADIUS_SURFACE))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .child(
                div()
                    .text_size(px(14.5))
                    .font_weight(FontWeight::BOLD)
                    .text_color(foreground)
                    .child("Dünyalar"),
            )
            .child(world_rows);

        // ---- logs ---------------------------------------------------------
        let mut log_lines = v_flex().w_full().gap(px(1.));
        for line in self.logs.iter().rev().take(8).rev() {
            log_lines = log_lines.child(
                div()
                    .text_size(px(10.))
                    .font_family("JetBrains Mono")
                    .text_color(cocoa.text_3)
                    .child(line.clone()),
            );
        }
        if self.logs.is_empty() {
            log_lines = log_lines.child(
                div()
                    .text_size(TEXT_12)
                    .text_color(cocoa.text_3)
                    .child("Henüz kayıt yok."),
            );
        }

        let log_panel = v_flex()
            .w_full()
            .gap_2()
            .p_4()
            .rounded(px(RADIUS_SURFACE))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(14.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(foreground)
                            .child("Son kayıtlar"),
                    )
                    .child(
                        Button::new("pds-logs")
                            .xsmall()
                            .ghost()
                            .label("Konsolda aç")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.tab = Tab::Console;
                                this.load_logs(cx);
                            })),
                    ),
            )
            .child(log_lines);

        v_flex()
            .id("profile-page")
            .size_full()
            .overflow_y_scroll()
            .gap_3()
            .px(px(18.))
            .py(px(16.))
            .child(header)
            .child(content_panel)
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .gap_3()
                    .child(div().flex_1().min_w_0().child(worlds_panel))
                    .child(div().flex_1().min_w_0().child(log_panel)),
            )
            .into_any_element()
    }

    fn open_active_folder(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.active_id.clone() {
            let path = format!("instances/{id}");
            self.open_folder(&path, cx);
        }
    }

    /// Zip the world into `backups/` next to the instance.
    fn backup_world(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(id) = self.active_id.clone() else {
            return;
        };
        self.request(
            cx,
            move |core| core.backup_world(&id, &name),
            |this, result, cx| {
                match result {
                    Ok(path) => this.toast(
                        format!("Dünya yedeklendi: {}", path.display()),
                        ToastKind::Success,
                        cx,
                    ),
                    Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
                }
                cx.notify();
            },
        );
    }
}
