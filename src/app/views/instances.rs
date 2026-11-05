//! Profiles: the collection the launcher is built around. One card per
//! profile, with the actions that belong to that object.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::assets::IconName;
use gpui_kit::*;

use crate::app::{AppView, ToastKind};
use crate::theme::{Cocoa, RADIUS_SURFACE};
use crate::widgets::*;

impl AppView {
    pub(crate) fn render_instances(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let header = page_header(
            "Oyun profilleri",
            "Her profil kendi sürümünü, yükleyicisini ve modlarını taşır.",
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("refresh-instances")
                        .ghost()
                        .small()
                        .label("Yenile")
                        .icon(IconName::RefreshCw)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_instances(cx))),
                )
                .child(
                    Button::new("new-profile-top")
                        .primary()
                        .small()
                        .label("Yeni profil")
                        .icon(IconName::Plus)
                        .on_click(
                            cx.listener(|this, _, window, cx| this.new_profile_dialog(window, cx)),
                        ),
                ),
            cx,
        );

        let mut loaders = h_flex().items_center().gap_1();
        for loader in ["vanilla", "fabric", "quilt", "forge", "neoforge"] {
            let value = loader.to_string();
            let selected = self.draft_loader == loader;
            let supported = !matches!(loader, "forge" | "neoforge");
            loaders = loaders.child(
                Button::new(format!("draft-{loader}"))
                    .small()
                    .when(selected, |this| this.secondary())
                    .when(!selected, |this| this.ghost())
                    .disabled(!supported)
                    .label(if supported {
                        crate::api::loader_label(loader).to_string()
                    } else {
                        format!("{} · yakında", crate::api::loader_label(loader))
                    })
                    .when(supported, |this| {
                        this.on_click(cx.listener(move |this, _, _, cx| {
                            this.draft_loader = value.clone();
                            cx.notify();
                        }))
                    }),
            );
        }
        let loader_row = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            .p_3()
            .rounded(px(6.))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .child(
                v_flex()
                    .gap(px(1.))
                    .child(
                        div()
                            .text_size(TEXT_11)
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cocoa.text_2)
                            .child("Yeni profilin yükleyicisi"),
                    )
                    .child(
                        div()
                            .text_size(TEXT_10)
                            .text_color(cocoa.text_3)
                            .child("Yeni profil oluştururken bu yükleyici kullanılır."),
                    ),
            )
            .child(loaders);

        let mut cards = v_flex().w_full().gap_3();
        if self.instances.is_empty() {
            cards = cards.child(empty_state(
                "Henüz profil yok",
                "İlk profili oluştur; sürüm ve yükleyici seçimi bir sonraki adımda.",
                "🍪",
                cx,
            ));
        }

        for instance in self.instances.clone() {
            let id = instance.id.clone();
            let name = instance.name.clone();
            let selected = self.active_id.as_deref() == Some(id.as_str());
            let initial = name
                .chars()
                .next()
                .map(|c| c.to_uppercase().to_string())
                .unwrap_or_else(|| "?".into());

            let hovered = self.hover_card.as_deref() == Some(id.as_str());
            let base_bg = if selected { cocoa.surface_2 } else { cocoa.surface_1 };
            let hover_bg = if selected { cocoa.surface_2 } else { cocoa.surface_2 };
            let base_border = if selected { cocoa.line_strong } else { cocoa.line };
            let hover_border = cocoa.line_strong;
            let (from_bg, to_bg) = if hovered { (base_bg, hover_bg) } else { (hover_bg, base_bg) };
            let (from_border, to_border) = if hovered {
                (base_border, hover_border)
            } else {
                (hover_border, base_border)
            };
            let hover_key = id.clone();

            let card = h_flex()
                    .id(SharedString::from(format!("instance-{id}")))
                    .w_full()
                    .items_center()
                    .gap_4()
                    .p_4()
                    .rounded(px(RADIUS_SURFACE))
                    .bg(cocoa.surface_1)
                    .border_1()
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        let next = if *hovered { Some(hover_key.clone()) } else { None };
                        if this.hover_card != next {
                            this.hover_card = next;
                            cx.notify();
                        }
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(42.))
                            .h(px(42.))
                            .flex_shrink_0()
                            .rounded(px(6.))
                            .bg(cocoa.surface_3)
                            .border_1()
                            .border_color(cocoa.line_strong)
                            .text_size(px(17.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(cocoa.amber)
                            .child(initial),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_size(px(17.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().foreground)
                                            .truncate()
                                            .child(name.clone()),
                                    )
                                    .when(selected, |this| {
                                        this.child(chip("Aktif", ChipTone::Amber, cx))
                                    })
                                    .when(instance.is_installing(), |this| {
                                        this.child(chip("Kuruluyor", ChipTone::Info, cx))
                                    }),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(chip(
                                        if instance.version.is_empty() {
                                            "—".to_string()
                                        } else {
                                            instance.version.clone()
                                        },
                                        ChipTone::Neutral,
                                        cx,
                                    ))
                                    .child(chip(instance.loader_label(), ChipTone::Neutral, cx))
                                    .child(chip(
                                        format!("{} mod", instance.mod_count),
                                        ChipTone::Neutral,
                                        cx,
                                    )),
                            )
                            .child(
                                div()
                                    .text_size(TEXT_10)
                                    .font_family("JetBrains Mono")
                                    .text_color(cocoa.text_3)
                                    .child(format!("id: {}", instance.id)),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .flex_shrink_0()
                            .child(
                                Button::new(format!("activate-{id}"))
                                    .small()
                                    .when(selected, |this| this.ghost())
                                    .when(!selected, |this| this.outline())
                                    .label(if selected { "Seçili" } else { "Aktif yap" })
                                    .on_click({
                                        let id = id.clone();
                                        cx.listener(move |this, _, _, cx| {
                                            this.set_active(id.clone(), cx);
                                        })
                                    }),
                            )
                            .child(
                                Button::new(format!("folder-{id}"))
                                    .small()
                                    .ghost()
                                    .label("Klasör")
                                    .icon(IconName::FolderOpen)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_folder("", cx);
                                    })),
                            )
                            .child(
                                Button::new(format!("delete-{id}"))
                                    .small()
                                    .ghost()
                                    .label("Sil")
                                    .icon(IconName::X)
                                    .on_click({
                                        let id = id.clone();
                                        let name = name.clone();
                                        cx.listener(move |this, _, window, cx| {
                                            this.confirm_delete(id.clone(), name.clone(), window, cx)
                                        })
                                    }),
                            ),
                    );

            cards = cards.child(
                card.fade_card(
                    SharedString::from(format!("inst-{id}-{hovered}")),
                    (from_bg, to_bg),
                    (from_border, to_border),
                    120,
                ),
            );
        }

        v_flex()
            .id("instances")
            .size_full()
            .overflow_y_scroll()
            .p(px(20.))
            .gap_4()
            .child(header)
            .child(loader_row)
            .child(cards)
            .into_any_element()
    }

    fn confirm_delete(
        &mut self,
        id: String,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let entity = cx.entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let entity = entity.clone();
            let id = id.clone();
            let name = name.clone();
            alert
                .title(format!("“{name}” profili silinsin mi?"))
                .description("Profil klasörü, modları ve dünyaları diskten kaldırılır. Bu işlem geri alınamaz.")
                .show_cancel(true)
                .ok_text("Sil")
                .cancel_text("Vazgeç")
                .ok_variant(gpui_kit::component::button::ButtonVariant::Danger)
                .width(px(440.))
                .on_ok({
                    let entity = entity.clone();
                    move |_, _, cx| {
                        entity.update(cx, |this, cx| this.delete_instance(id.clone(), name.clone(), cx));
                        true
                    }
                })
        });
    }

    pub(crate) fn create_instance(
        &mut self,
        name: String,
        version: String,
        loader: String,
        cx: &mut Context<Self>,
    ) {
        let name = if name.trim().is_empty() {
            "Yeni Profil".to_string()
        } else {
            name.trim().to_string()
        };
        let version = if version.trim().is_empty() {
            "1.21.4".to_string()
        } else {
            version.trim().to_string()
        };
        let display = name.clone();
        self.request(
            cx,
            move |core| core.create_instance(&name, &version, &loader, ""),
            move |this, result, cx| match result {
                Ok(instance) => {
                    this.set_active(instance.id.clone(), cx);
                    this.toast(format!("“{display}” oluşturuldu."), ToastKind::Success, cx);
                    this.refresh_instances(cx);
                }
                Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
            },
        );
    }
}
