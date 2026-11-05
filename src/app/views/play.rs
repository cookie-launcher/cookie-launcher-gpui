//! Oyna: the screen most sessions start and end on.
//!
//! Three bands, matching the Tauri layout: a 300 px hero that names the active
//! profile and owns the launch action, a row pairing recent worlds with the
//! launch settings that apply to every profile, and the quick-action strip.

use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::{Input, InputState},
    slider::Slider,
    switch::Switch,
    WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{AppView, Tab, ToastKind};
use crate::theme::Cocoa;
use crate::widgets::LaunchMode;
use crate::widgets::*;

impl AppView {
    pub(crate) fn render_play(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let _cocoa = Cocoa::get(cx).clone_shades();
        v_flex()
            .id("play")
            .size_full()
            .overflow_y_scroll()
            .px(px(18.))
            .pt(px(8.))
            .pb(px(16.))
            .gap_3()
            .bg(cx.theme().background)
            .child(self.render_hero(cx))
            .child(
                h_flex()
                    .w_full()
                    .h(px(296.))
                    .items_stretch()
                    .gap_3()
                    .child(div().flex_1().min_w_0().child(self.render_worlds_panel(cx)))
                    .child(
                        div()
                            .w(px(324.))
                            .flex_shrink_0()
                            .child(self.render_settings_panel(window, cx)),
                    ),
            )
            .child(div().h(px(6.)))
            .child(self.render_quick_actions(cx))
            .into_any_element()
    }

    // -- hero --------------------------------------------------------------

    fn render_hero(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let foreground = cx.theme().foreground;
        let instance = self.active_instance().cloned();
        let mode = self.launch_mode();
        let status = self.status.clone();

        let (name, initial, version, loader, mods) = match &instance {
            Some(instance) => (
                instance.name.clone(),
                instance
                    .name
                    .chars()
                    .next()
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_else(|| "?".into()),
                format!("MC {}", instance.version),
                instance.loader_label().to_string(),
                instance.mod_count,
            ),
            None => (
                "Profil seçilmedi".to_string(),
                "?".to_string(),
                "MC —".to_string(),
                "—".to_string(),
                0,
            ),
        };

        let (state_label, state_tone) = if status.is_running {
            ("Oyun açık", ChipTone::Info)
        } else if status.is_installing {
            ("Kuruluyor", ChipTone::Amber)
        } else if instance.is_some() {
            ("Hazır", ChipTone::Amber)
        } else {
            ("Profil yok", ChipTone::Neutral)
        };

        let launch_label = match mode {
            LaunchMode::Installing => format!("Kuruluyor… %{}", status.percent),
            LaunchMode::Running => "Oyun açık".to_string(),
            LaunchMode::Idle | LaunchMode::NoProfile => "Oyna".to_string(),
        };

        let select_label = match &instance {
            Some(instance) => format!(
                "{} — {} ({})",
                instance.name,
                instance.version,
                instance.loader_label().to_lowercase()
            ),
            None => "Profil seç".to_string(),
        };

        v_flex()
            .relative()
            .w_full()
            .h(px(280.))
            .flex_shrink_0()
            .rounded(px(10.))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .overflow_hidden()
            .child(grid_background(1120., 280., 40.))
            .child(
                div()
                    .absolute()
                    .right(px(22.))
                    .top(px(2.))
                    .child(img("cookie-block.png").w(px(206.)).h(px(206.))),
            )
            .child(
                v_flex()
                    .relative()
                    .flex_1()
                    .p(px(24.))
                    .gap(px(14.))
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(TEXT_13)
                                    .text_color(cocoa.text_2)
                                    .child("Aktif profil"),
                            )
                            .child(hero_status_badge(state_label, state_tone, cx)),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(18.))
                            .child(
                                div()
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .justify_center()
                                    .w(px(56.))
                                    .h(px(56.))
                                    .rounded(px(8.))
                                    .bg(cocoa.surface_3)
                                    .text_size(px(26.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cocoa.amber)
                                    .child(initial),
                            )
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .flex_1()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(44.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(foreground)
                                            .truncate()
                                            .child(name),
                                    )
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .child(hero_chip(version, cx))
                                            .child(hero_chip(loader, cx))
                                            .child(hero_chip(format!("{mods} mod"), cx))
                                            .child(hero_chip("hiç oynanmadı", cx)),
                                    ),
                            ),
                    )
                    .child(div().flex_1())
                    .child(div().h(px(1.)).w_full().bg(cocoa.line))
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .gap_3()
                            .pt(px(12.))
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_size(TEXT_12)
                                    .text_color(cocoa.text_2)
                                    .child("Profil değiştir"),
                            )
                            .child(
                                h_flex()
                                    .id("profile-select")
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_2()
                                    .h(px(38.))
                                    .w(px(290.))
                                    .px_3()
                                    .rounded(px(6.))
                                    .bg(cocoa.surface_1)
                                    .border_1()
                                    .border_color(cocoa.line_strong)
                                    .cursor_pointer()
                                    .hover(|this| this.border_color(cocoa.text_3))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.profile_picker(window, cx)
                                    }))
                                    .child(
                                        Icon::new(IconName::MessageSquare)
                                            .size(px(15.))
                                            .text_color(cocoa.text_3),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(TEXT_13)
                                            .text_color(cocoa.text_2)
                                            .child(select_label),
                                    )
                                    .child(
                                        Icon::new(IconName::ChevronDown)
                                            .size(px(14.))
                                            .text_color(cocoa.text_3),
                                    ),
                            )
                            .child(
                                Button::new("hero-details")
                                    .label("Detaylar")
                                    .disabled(instance.is_none())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.tab = Tab::Instances;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("hero-manage")
                                    .ghost()
                                    .label("Profilleri yönet")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.tab = Tab::Instances;
                                        cx.notify();
                                    })),
                            )
                            .child(div().flex_1())
                            .child(
                                launch_button_at(
                                    "launch-game",
                                    launch_label,
                                    "ENTER",
                                    mode,
                                    196.,
                                    self.launch_pressed,
                                    cx.listener(|this, _, _, cx| this.launch(cx)),
                                    cx.listener(|this, _, _, cx| {
                                        this.launch_pressed = true;
                                        cx.notify();
                                    }),
                                    cx.listener(|this, _, _, cx| {
                                        this.launch_pressed = false;
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ),
            )
            .into_any_element()
    }

    // -- recent worlds ------------------------------------------------------

    fn render_worlds_panel(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let foreground = cx.theme().foreground;
        let instance = self.active_instance().cloned();
        let subtitle = match &instance {
            Some(instance) => format!("{} • MC {}", instance.name, instance.version),
            None => "Profil seçilmedi".to_string(),
        };

        let mut body = v_flex().w_full().gap_2();
        if self.worlds.is_empty() {
            body = body.child(
                div()
                    .py(px(8.))
                    .text_size(TEXT_12)
                    .text_color(cocoa.text_3)
                    .child(if instance.is_some() {
                        "Bu profilde henüz dünya yok. Bir dünya oluşturduğunda burada görünür."
                    } else {
                        "Profil seçilmedi. Bir profil oluşturduğunda son dünyalar burada görünür."
                    }),
            );
        }
        for world in self.worlds.clone() {
            let name = world.name.clone();
            body = body.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_3()
                    .p_2()
                    .rounded(px(6.))
                    .hover(|this| this.bg(cocoa.surface_2))
                    .child(icon_tile(40., 6., "W", cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(1.))
                            .child(
                                div()
                                    .text_size(TEXT_12)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(foreground)
                                    .truncate()
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(TEXT_10)
                                    .text_color(cocoa.text_3)
                                    .child(format!(
                                        "{} · {:.2} MB",
                                        if world.last_played.is_empty() {
                                            "bilinmiyor"
                                        } else {
                                            &world.last_played
                                        },
                                        world.size_mb
                                    )),
                            ),
                    )
                    .child(
                        quiet_button("world-folder", "Klasör", cx)
                            .on_click(cx.listener(|this, _, _, cx| this.open_folder("saves", cx))),
                    )
                    .child(quiet_button("world-copy", "Kopyala", cx).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.toast(
                                "Dünya kopyalama profil ekranından yapılıyor.",
                                ToastKind::Info,
                                cx,
                            );
                        }),
                    )),
            );
        }

        v_flex()
            .w_full()
            .h_full()
            .gap_3()
            .p_4()
            .rounded(px(8.))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(14.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(foreground)
                                    .child("Son dünyalar"),
                            )
                            .child(
                                div()
                                    .text_size(TEXT_11)
                                    .text_color(cocoa.text_3)
                                    .child(subtitle),
                            ),
                    )
                    .child(
                        quiet_button("worlds-refresh", "Yenile", cx)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_worlds(cx))),
                    ),
            )
            .child(body)
            .into_any_element()
    }

    // -- launch settings ----------------------------------------------------

    fn render_settings_panel(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let foreground = cx.theme().foreground;
        let ram = self.ram_gb;

        v_flex()
            .w_full()
            .h_full()
            .gap(px(8.))
            .p(px(12.))
            .rounded(px(8.))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .justify_between()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(14.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(foreground)
                                    .child("Oyun ayarları"),
                            )
                            .child(
                                div()
                                    .text_size(TEXT_10)
                                    .text_color(cocoa.text_3)
                                    .child("Tüm profillerde geçerli"),
                            ),
                    )
                    .child(tag("Çevrimdışı", cx)),
            )
            .child(
                v_flex()
                    .gap(px(5.))
                    .child(
                        div()
                            .text_size(TEXT_12)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cocoa.text_2)
                            .child("Oyuncu adı"),
                    )
                    .child(Input::new(&self.username).prefix(
                        Icon::new(IconName::User).size(px(15.)).text_color(cocoa.text_3),
                    )),
            )
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .p(px(8.))
                    .rounded(px(6.))
                    .bg(cocoa.surface_2)
                    .border_1()
                    .border_color(cocoa.line)
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .justify_center()
                            .w(px(30.))
                            .h(px(30.))
                            .rounded(px(6.))
                            .bg(cocoa.surface_3)
                            .child(Icon::new(IconName::Zap).size(px(15.)).text_color(cocoa.amber)),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(1.))
                            .child(
                                div()
                                    .text_size(TEXT_12)
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(foreground)
                                    .child("Optimize motoru"),
                            )
                            .child(
                                div()
                                    .text_size(TEXT_10)
                                    .text_color(cocoa.text_3)
                                    .truncate()
                                    .child("Sodium, Lithium ve FerriteCore"),
                            ),
                    )
                    .child(
                        Switch::new("optimize-engine")
                            .checked(self.optimize_engine)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.optimize_engine = *checked;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap(px(4.))
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(TEXT_12)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(cocoa.text_2)
                                    .child("Ayrılan bellek"),
                            )
                            .child(
                                div()
                                    .px_2()
                                    .py(px(1.))
                                    .rounded(px(3.))
                                    .bg(cocoa.amber_soft)
                                    .border_1()
                                    .border_color(Hsla::from(rgba(0xe6a05252)))
                                    .text_size(px(11.))
                                    .font_family("JetBrains Mono")
                                    .text_color(cocoa.amber)
                                    .child(format!("{ram} GB")),
                            ),
                    )
                    .child(Slider::new(&self.ram_slider))
                    .child(
                        h_flex()
                            .w_full()
                            .justify_between()
                            .child(mark("2 GB", cx))
                            .child(mark("4 GB", cx))
                            .child(mark("8 GB", cx))
                            .child(mark("16 GB", cx)),
                    ),
            )
            .child(div().flex_1())
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .gap_2()
                    .pt_2()
                    .border_t_1()
                    .border_color(cocoa.line)
                    .child(status_dot(cocoa.amber, None))
                    .child(
                        div()
                            .text_size(TEXT_10)
                            .text_color(cocoa.text_3)
                            .child(match &self.connection {
                                crate::app::Connection::Ready(_) => {
                                    "Hazır   Rust çekirdek   v4.0.0".to_string()
                                }
                                _ => "Bekleniyor…".to_string(),
                            }),
                    ),
            )
            .into_any_element()
    }

    // -- quick actions ------------------------------------------------------

    fn render_quick_actions(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let background = cx.theme().background;
        let foreground = cx.theme().foreground;
        let actions = [
            ("quick-mods", "Modlar", Tab::Modrinth),
            ("quick-profile", "Profil", Tab::Instances),
            ("quick-shots", "Görüntüler", Tab::Gallery),
        ];
        let mut row = h_flex().w_full().gap_3().bg(background);
        for (id, label, tab) in actions {
            let hovered = self.hover_quick == Some(id);
            let (from, to) = if hovered {
                (Cocoa::get(cx).surface_1, Cocoa::get(cx).surface_2)
            } else {
                (Cocoa::get(cx).surface_2, Cocoa::get(cx).surface_1)
            };
            let card = h_flex()
                .id(id)
                .flex_1()
                .items_center()
                .gap_3()
                .h(px(50.))
                .px_3()
                .rounded(px(6.))
                .border_1()
                .border_color(Cocoa::get(cx).line)
                .cursor_pointer()
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    let next = if *hovered { Some(id) } else { None };
                    if this.hover_quick != next {
                        this.hover_quick = next;
                        cx.notify();
                    }
                }))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.tab = tab;
                    cx.notify();
                }))
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .items_center()
                        .justify_center()
                        .w(px(28.))
                        .h(px(28.))
                        .rounded(px(5.))
                        .bg(Cocoa::get(cx).surface_3)
                        .child(
                            Icon::new(IconName::Folder)
                                .size(px(14.))
                                .text_color(Cocoa::get(cx).amber),
                        ),
                )
                .child(
                    div()
                        .text_size(TEXT_12)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(foreground)
                        .child(label),
                );

            row = row.child(card.fade_bg(
                SharedString::from(format!("quick-{id}-{hovered}")),
                from,
                to,
                120,
            ));
        }
        row.into_any_element()
    }

    // -- dialogs ------------------------------------------------------------

    pub(crate) fn profile_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.entity();
        let instances = self.instances.clone();
        let active = self.active_id.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let mut list = v_flex().gap_1();
            if instances.is_empty() {
                list = list.child(div().p_3().text_size(TEXT_12).child("Henüz profil yok."));
            }
            for instance in &instances {
                let id = instance.id.clone();
                let selected = active.as_deref() == Some(instance.id.as_str());
                list = list.child(
                    Button::new(format!("pick-{}", instance.id))
                        .ghost()
                        .w_full()
                        .justify_start()
                        .label(format!(
                            "{} · {} {}",
                            instance.name,
                            instance.loader_label(),
                            instance.version
                        ))
                        .when(selected, |this| this.secondary())
                        .on_click({
                            let entity = entity.clone();
                            move |_, window, cx| {
                                let id = id.clone();
                                entity.update(cx, |this, cx| this.set_active(id, cx));
                                window.close_dialog(cx);
                            }
                        }),
                );
            }
            dialog.title("Profil seç").w(px(440.)).child(list).footer(
                gpui_kit::component::dialog::DialogFooter::new().child(
                    Button::new("picker-close")
                        .label("Kapat")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
        });
    }

    pub(crate) fn new_profile_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let latest = self
            .versions
            .first()
            .map(|version| version.id.clone())
            .unwrap_or_else(|| "1.21.4".to_string());

        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Profil adı")
                .default_value("Yeni Profil")
        });
        let version = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Minecraft sürümü")
                .default_value(latest)
        });
        name.focus_handle(cx).focus(window, cx);

        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let loader = entity.read(cx).draft_loader.clone();
            dialog
                .title("Yeni profil")
                .w(px(460.))
                .child(
                    v_flex()
                        .gap_3()
                        .child(field("Profil adı", Input::new(&name), cx))
                        .child(field("Minecraft sürümü", Input::new(&version), cx))
                        .child(
                            div()
                                .text_size(TEXT_10)
                                .text_color(Cocoa::get(cx).text_3)
                                .child(format!(
                                    "Yükleyici: {}",
                                    crate::api::loader_label(&loader)
                                )),
                        ),
                )
                .footer(
                    gpui_kit::component::dialog::DialogFooter::new()
                        .child(
                            Button::new("new-cancel")
                                .label("Vazgeç")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child({
                            let entity = entity.clone();
                            let name = name.clone();
                            let version = version.clone();
                            Button::new("new-create")
                                .primary()
                                .label("Oluştur")
                                .on_click(move |_, window, cx| {
                                    let name_value = name.read(cx).value().to_string();
                                    let version_value = version.read(cx).value().to_string();
                                    entity.update(cx, |this, cx| {
                                        let loader = this.draft_loader.clone();
                                        this.create_instance(
                                            name_value,
                                            version_value,
                                            loader,
                                            cx,
                                        );
                                    });
                                    window.close_dialog(cx);
                                })
                        }),
                )
        });
    }
}

fn mark(label: &'static str, cx: &App) -> impl IntoElement {
    div()
        .text_size(px(9.))
        .font_family("JetBrains Mono")
        .text_color(Cocoa::get(cx).text_3)
        .child(label)
}

fn hero_chip(label: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    let cocoa = Cocoa::get(cx);
    div()
        .px_2()
        .py(px(1.))
        .rounded(px(4.))
        .bg(cocoa.surface_2)
        .border_1()
        .border_color(cocoa.line)
        .text_size(TEXT_10)
        .font_weight(FontWeight::MEDIUM)
        .text_color(cocoa.text_2)
        .child(label.into())
}

/// The hero's status pill: amber wash, hairline border, square dot.
fn hero_status_badge(label: &str, tone: ChipTone, cx: &App) -> AnyElement {
    let cocoa = Cocoa::get(cx);
    let (fg, bg, border) = match tone {
        ChipTone::Info => (
            cocoa.info,
            Hsla::from(rgba(0x8fb0cf1a)),
            Hsla::from(rgba(0x8fb0cf52)),
        ),
        ChipTone::Amber => (
            cocoa.amber_hover,
            Hsla::from(rgba(0xe6a0521a)),
            Hsla::from(rgba(0xe6a05252)),
        ),
        _ => (cocoa.text_3, cocoa.surface_2, cocoa.line),
    };
    h_flex()
        .items_center()
        .gap(px(5.))
        .px(px(9.))
        .py(px(3.))
        .rounded(px(4.))
        .bg(bg)
        .border_1()
        .border_color(border)
        .child(div().w(px(6.)).h(px(6.)).rounded(px(1.)).bg(fg))
        .child(
            div()
                .text_size(TEXT_10)
                .font_weight(FontWeight::BOLD)
                .text_color(fg)
                .child(label.to_string()),
        )
        .into_any_element()
}
