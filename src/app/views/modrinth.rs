//! İçerik merkezi: search Modrinth and install into the active profile.
//! Mirrors the Tauri layout: category pills, a sort bar, a target bar, a
//! two-column result grid, and the installed-content panel underneath.

use gpui_kit::assets::IconName;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    checkbox::Checkbox,
    input::Input,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::api::SearchHit;
use crate::app::{AppView, SearchKind, ToastKind};
use crate::theme::Cocoa;
use crate::widgets::*;

impl AppView {
    pub(crate) fn render_modrinth(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let foreground = cx.theme().foreground;
        let active_name = self
            .active_instance()
            .map(|instance| instance.name.clone())
            .unwrap_or_else(|| "profil yok".to_string());
        let target_detail = match self.active_instance() {
            Some(instance) => format!(
                "“{}” profiline kurulacak • MC {} • {} • {} mevcut mod",
                instance.name,
                instance.version,
                instance.loader_label(),
                instance.mod_count
            ),
            None => "Önce bir profil seç.".to_string(),
        };

        let header = h_flex()
            .w_full()
            .items_start()
            .justify_between()
            .gap_4()
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(27.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(foreground)
                            .child("İçerik merkezi"),
                    )
                    .child(
                        div()
                            .text_size(TEXT_12)
                            .text_color(cocoa.text_3)
                            .child("Yüz binlerce mod, modpack, shader ve doku paketini tek tıkla kurun."),
                    ),
            )
            .child(
                h_flex()
                    .w(px(340.))
                    .flex_shrink_0()
                    .items_center()
                    .child(div().flex_1().min_w_0().child(Input::new(&self.search_input)))
                    .child(
                        div()
                            .id("search-go")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(40.))
                            .h(px(38.))
                            .rounded_r(px(5.))
                            .bg(cocoa.paper)
                            .border_1()
                            .border_color(rgb(0xfff1dc))
                            .cursor_pointer()
                            .hover(|this| this.bg(cocoa.paper_hover))
                            .on_click(cx.listener(|this, _, _, cx| this.search_modrinth(cx)))
                            .child(
                                if self.search_busy {
                                    Icon::new(IconName::LoaderCircle)
                                } else {
                                    Icon::new(IconName::Search)
                                }
                                .size(px(15.))
                                .text_color(cocoa.paper_ink),
                            ),
                    ),
            );

        let mut kinds = h_flex().gap_2();
        for kind in SearchKind::ALL {
            let selected = self.search_kind == kind;
            kinds = kinds.child(
                div()
                    .id(("kind", kind as usize))
                    .flex()
                    .items_center()
                    .h(px(32.))
                    .px(px(12.))
                    .rounded(px(5.))
                    .border_1()
                    .border_color(if selected { cocoa.line_strong } else { cocoa.line })
                    .bg(if selected { cocoa.surface_2 } else { cocoa.surface_1 })
                    .text_size(TEXT_12)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(if selected { foreground } else { cocoa.text_3 })
                    .cursor_pointer()
                    .hover(|this| this.bg(cocoa.surface_2))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if this.search_kind != kind {
                            this.search_kind = kind;
                            this.search_modrinth(cx);
                        }
                    }))
                    .child(kind.label()),
            );
        }

        let sort_label = match self.search_sort.as_str() {
            "relevance" => "İlgi düzeyi",
            "follows" => "En çok takip",
            "newest" => "En yeni",
            "updated" => "Son güncellenen",
            _ => "En çok indirilen",
        };
        let sort_bar = h_flex()
            .w_full()
            .items_center()
            .gap_4()
            .px_3()
            .py_2()
            .rounded(px(6.))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .child(
                div()
                    .text_size(TEXT_11)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cocoa.text_3)
                    .child("Sıralama"),
            )
            .child(
                h_flex()
                    .id("sort-select")
                    .items_center()
                    .gap_2()
                    .h(px(30.))
                    .px_3()
                    .rounded(px(5.))
                    .bg(cocoa.surface_2)
                    .border_1()
                    .border_color(cocoa.line_strong)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.cycle_sort(cx)))
                    .child(
                        div()
                            .text_size(TEXT_12)
                            .text_color(cocoa.text_2)
                            .child(sort_label),
                    )
                    .child(Icon::new(IconName::ChevronDown).size(px(13.)).text_color(cocoa.text_3)),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(Checkbox::new("hide-downloaded").checked(self.hide_downloaded))
                    .child(
                        div()
                            .text_size(TEXT_11)
                            .text_color(cocoa.text_3)
                            .child("İndirilenleri gizle"),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(Checkbox::new("install-deps").checked(self.install_dependencies))
                    .child(
                        div()
                            .text_size(TEXT_11)
                            .text_color(cocoa.text_3)
                            .child("Bağımlılıkları da kur"),
                    ),
            );

        let target_bar = h_flex()
            .w_full()
            .items_center()
            .gap_3()
            .px_3()
            .py_2()
            .rounded(px(6.))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(TEXT_11)
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cocoa.text_3)
                    .child("Kurulum hedefi"),
            )
            .child(
                h_flex()
                    .id("target-select")
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .h(px(34.))
                    .w(px(288.))
                    .px_3()
                    .rounded(px(6.))
                    .bg(cocoa.surface_1)
                    .border_1()
                    .border_color(cocoa.line_strong)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, window, cx| this.profile_picker(window, cx)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(TEXT_12)
                            .text_color(cocoa.text_2)
                            .child(match self.active_instance() {
                                Some(instance) => format!(
                                    "{} — {} ({})",
                                    instance.name,
                                    instance.version,
                                    instance.loader_label().to_lowercase()
                                ),
                                None => "Profil seç".to_string(),
                            }),
                    )
                    .child(Icon::new(IconName::ChevronDown).size(px(13.)).text_color(cocoa.text_3)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_size(TEXT_11)
                    .text_color(cocoa.text_3)
                    .child(target_detail),
            );

        let mut results = v_flex().w_full().gap_3();
        if self.hits.is_empty() {
            results = results.child(if self.search_busy {
                {
                    let mut skeleton = v_flex().w_full().gap_2();
                    for index in 0..3 {
                        skeleton = skeleton.child(
                            div()
                                .h(px(84.))
                                .w_full()
                                .rounded(px(6.))
                                .bg(cocoa.surface_2)
                                .with_animation(
                                    SharedString::from(format!("skeleton-row-{index}")),
                                    Animation::new(std::time::Duration::from_millis(1400))
                                        .repeat()
                                        .with_easing(|t: f32| (t * std::f32::consts::PI).sin().abs()),
                                    |el: Div, t: f32| el.opacity(0.55 + 0.45 * t),
                                ),
                        );
                    }
                    skeleton.into_any_element()
                }
            } else {
                empty_state(
                    "Sonuç yok",
                    "Aramayı değiştir ya da başka bir içerik türü seç.",
                    "⌕",
                    cx,
                )
                .into_any_element()
            });
        }

        for pair in self.hits.clone().chunks(2) {
            let mut row = h_flex().w_full().items_stretch().gap_3();
            for hit in pair {
                row = row.child(self.render_hit_card(hit.clone(), cx));
            }
            if pair.len() == 1 {
                row = row.child(div().flex_1());
            }
            results = results.child(row);
        }

        // -- installed content ------------------------------------------------
        let counts = [
            ("mod", "Modlar", self.content_counts[0]),
            ("shader", "Shaderlar", self.content_counts[1]),
            ("resourcepack", "Doku paketleri", self.content_counts[2]),
        ];
        let mut tabs = h_flex().gap_2();
        for (kind, label, count) in counts {
            let selected = self.content_kind == kind;
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("content-tab-{kind}")))
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(30.))
                    .px(px(10.))
                    .rounded(px(5.))
                    .bg(if selected { cocoa.surface_2 } else { cocoa.surface_1 })
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

        let mut rows = v_flex().w_full().gap_1();
        if self.content_items.is_empty() {
            let empty_label = match self.content_kind.as_str() {
                "shader" => ("Bu profilde henüz shader yok", "Shaderlar sekmesinden içerik kurarak başlayın."),
                "resourcepack" => ("Bu profilde henüz doku paketi yok", "Doku paketleri sekmesinden içerik kurarak başlayın."),
                _ => ("Bu profilde henüz mod yok", "Modrinth sekmesinden içerik kurarak başlayın."),
            };
            rows = rows.child(
                v_flex()
                    .w_full()
                    .min_h(px(130.))
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(cocoa.line)
                    .child(icon_tile(46., 6., "M", cx))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(foreground)
                            .child(empty_label.0),
                    )
                    .child(
                        div()
                            .text_size(TEXT_12)
                            .text_color(cocoa.text_3)
                            .child(empty_label.1),
                    ),
            );
        }
        for item in self.content_items.clone() {
            let name = item.name.clone();
            rows = rows.child(
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
                        Button::new(SharedString::from(format!("toggle-{name}")))
                            .xsmall()
                            .ghost()
                            .label(if item.enabled { "Devre dışı" } else { "Etkinleştir" })
                            .on_click(cx.listener({
                                let name = name.clone();
                                move |this, _, _, cx| this.toggle_content(name.clone(), cx)
                            })),
                    )
                    .child(
                        Button::new(SharedString::from(format!("remove-{name}")))
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

        let installed = v_flex()
            .w_full()
            .gap_3()
            .p_4()
            .rounded(px(8.))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(14.5))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(foreground)
                                    .child("Yüklü içerikler"),
                            )
                            .child(
                                div()
                                    .text_size(TEXT_11)
                                    .text_color(cocoa.text_3)
                                    .child(format!("— {active_name}")),
                            ),
                    )
                    .child(tabs),
            )
            .child(rows);

        let _ = window;
        v_flex()
            .id("modrinth")
            .size_full()
            .overflow_y_scroll()
            .px(px(18.))
            .pt(px(14.))
            .pb(px(16.))
            .gap_3()
            .track_scroll(&self.modrinth_scroll)
            .child(header)
            .child(kinds)
            .child(sort_bar)
            .child(target_bar)
            .child(results)
            .child(installed)
            .into_any_element()
    }

    fn render_hit_card(&mut self, hit: SearchHit, cx: &mut Context<Self>) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let foreground = cx.theme().foreground;
        let slug = hit.slug.clone();
        let installing = self.installing_slug.as_deref() == Some(slug.as_str());
        let initial = hit
            .title
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "•".into());
        let icon = self.icons.get(&hit.slug).cloned();

        let hovered = self.hover_card.as_deref() == Some(hit.slug.as_str());
        let (from_border, to_border) = if hovered {
            (cocoa.line, cocoa.line_strong)
        } else {
            (cocoa.line_strong, cocoa.line)
        };
        let slug_for_hover = hit.slug.clone();
        let card = h_flex()
            .id(SharedString::from(format!("hit-{}", hit.slug)))
            .flex_1()
            .min_w(px(0.))
            .items_start()
            .gap_3()
            .p(px(10.))
            .rounded(px(6.))
            .bg(cocoa.surface_1)
            .border_1()
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                let next = if *hovered {
                    Some(slug_for_hover.clone())
                } else {
                    None
                };
                if this.hover_card != next {
                    this.hover_card = next;
                    cx.notify();
                }
            }))
            .child(match icon {
                Some(image) => img(ImageSource::Image(image))
                    .flex_shrink_0()
                    .w(px(56.))
                    .h(px(56.))
                    .rounded(px(6.))
                    .object_fit(ObjectFit::Cover)
                    .into_any_element(),
                None => div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .w(px(56.))
                    .h(px(56.))
                    .rounded(px(6.))
                    .bg(cocoa.surface_3)
                    .text_size(px(22.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(cocoa.amber)
                    .child(initial)
                    .into_any_element(),
            })
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .child(
                        div()
                            .text_size(TEXT_13)
                            .font_weight(FontWeight::BOLD)
                            .text_color(foreground)
                            .truncate()
                            .child(if hit.title.is_empty() {
                                hit.slug.clone()
                            } else {
                                hit.title.clone()
                            }),
                    )
                    .child(
                        div()
                            .text_size(TEXT_11)
                            .text_color(cocoa.text_3)
                            .truncate()
                            .child(hit.description.clone()),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(TEXT_10)
                                    .font_family("JetBrains Mono")
                                    .text_color(cocoa.text_3)
                                    .child(format!("↓{}", compact(hit.downloads))),
                            )
                            .child(chip(hit.author.clone(), ChipTone::Neutral, cx)),
                    ),
            )
            .child(
                Button::new(SharedString::from(format!("install-{slug}")))
                    .small()
                    .outline()
                    .label(if installing { "…" } else { "Kur" })
                    .loading(installing)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.install_hit(slug.clone(), cx)
                    })),
            );

        card.fade_border(
            SharedString::from(format!("hit-border-{}-{hovered}", hit.slug)),
            from_border,
            to_border,
            120,
        )
    }

    fn cycle_sort(&mut self, cx: &mut Context<Self>) {
        self.search_sort = match self.search_sort.as_str() {
            "downloads" => "relevance".to_string(),
            "relevance" => "follows".to_string(),
            "follows" => "newest".to_string(),
            _ => "downloads".to_string(),
        };
        self.search_modrinth(cx);
    }

    pub(crate) fn search_modrinth(&mut self, cx: &mut Context<Self>) {
        self.search_busy = true;
        let query = self.search_input.read(cx).value().to_string();
        let kind = self.search_kind.api_value().to_string();
        let sort = self.search_sort.clone();
        let (version, loader) = match self.active_instance() {
            Some(instance) => (instance.version.clone(), instance.loader.clone()),
            None => (String::new(), String::new()),
        };
        self.modrinth_scroll.set_offset(point(px(0.), px(0.)));
        self.request(
            cx,
            move |core| core.search(&query, &kind, &sort, 20, &version, &loader),
            |this, result, cx| {
                this.search_busy = false;
                match result {
                    Ok(hits) => {

                        this.search_total = hits.len() as i64;
                        this.hits = hits;
                        this.load_mod_icons(cx);
                    }
                    Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
                }
                cx.notify();
            },
        );
    }

    pub(crate) fn install_hit(&mut self, slug: String, cx: &mut Context<Self>) {
        // A modpack becomes a profile of its own, not a file in mods/.
        if self.search_kind == SearchKind::Modpack {
            self.installing_slug = Some(slug.clone());
            cx.notify();
            self.request(
                cx,
                move |core| core.install_modpack(&slug),
                |this, result, cx| {
                    this.installing_slug = None;
                    match result {
                        Ok(id) => {
                            this.toast("Modpack kuruldu.", ToastKind::Success, cx);
                            this.set_active(id, cx);
                            this.refresh_instances(cx);
                        }
                        Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
                    }
                    cx.notify();
                },
            );
            return;
        }

        let Some(instance) = self.active_instance().cloned() else {
            self.toast("Önce bir profil seç.", ToastKind::Info, cx);
            return;
        };
        self.installing_slug = Some(slug.clone());
        cx.notify();

        let project_type = self.search_kind.api_value().to_string();
        let deps = self.install_dependencies;
        let instance_id = instance.id.clone();
        self.request(
            cx,
            move |core| core.install(&slug, &instance_id, &project_type, deps),
            |this, result, cx| {
                this.installing_slug = None;
                match result {
                    Ok(installed) => {
                        this.toast(installed.join(", "), ToastKind::Success, cx);
                        this.refresh_instances(cx);
                    }
                    Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
                }
                cx.notify();
            },
        );
    }
}

fn compact(value: i64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.)
    } else if value >= 1_000 {
        format!("{:.1}k", value as f64 / 1_000.)
    } else {
        value.to_string()
    }
}

fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
