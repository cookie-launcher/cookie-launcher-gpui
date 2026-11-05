//! Friends: the social snapshot the core maintains. While the social service is
//! unavailable the screen says so instead of pretending to be empty.

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::Input,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::assets::IconName;
use gpui_kit::*;

use crate::api::SocialFriend;
use crate::app::{AppView, ToastKind};
use crate::theme::{Cocoa, RADIUS_SURFACE};
use crate::widgets::*;

impl AppView {
    pub(crate) fn render_friends(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let social = self.social.clone();
        let connected = social.connected;
        let name = social.display_name();
        let code = social.code();

        let header = page_header(
            "Arkadaşlar",
            "Dünya paylaşımı relay üzerinden; oyun trafiği host'ta kalır.",
            h_flex()
                .items_center()
                .gap_2()
                .child(chip(
                    if connected { "Paylaşım açık" } else { "Paylaşım kapalı" },
                    if connected { ChipTone::Amber } else { ChipTone::Neutral },
                    cx,
                ))
                .child(
                    Button::new("friends-refresh")
                        .ghost()
                        .small()
                        .label("Yenile")
                        .icon(IconName::RefreshCw)
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_social(cx))),
                ),
            cx,
        );

        let identity = h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_4()
            .p_4()
            .rounded(px(RADIUS_SURFACE))
            .bg(cocoa.surface_1)
            .border_1()
            .border_color(cocoa.line)
            .child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(10.))
                            .font_family("JetBrains Mono")
                            .text_color(cocoa.text_3)
                            .child("SENİN ARKADAŞ KODUN"),
                    )
                    .child(
                        div()
                            .text_size(px(22.))
                            .font_family("JetBrains Mono")
                            .font_weight(FontWeight::BOLD)
                            .text_color(if code.is_empty() { cocoa.text_3 } else { cocoa.amber })
                            .child(if code.is_empty() {
                                "—".to_string()
                            } else {
                                code.clone()
                            }),
                    )
                    .child(
                        div()
                            .text_size(TEXT_11)
                            .text_color(cocoa.text_3)
                            .child(if name.is_empty() {
                                "Sosyal servis henüz hazır değil.".to_string()
                            } else {
                                format!("{name} olarak bağlısın.")
                            }),
                    ),
            )
            .child(
                Button::new("copy-code")
                    .outline()
                    .small()
                    .label("Kodu kopyala")
                    .icon(IconName::Copy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let code = this.social.code();
                        cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
                        this.toast("Arkadaş kodu kopyalandı.", ToastKind::Success, cx);
                    })),
            );

        let sharing = social.host_relay.as_ref().map(|r| r.running).unwrap_or(false);
        let share_panel = {
            let mut panel = h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap_4()
                .p_4()
                .rounded(px(RADIUS_SURFACE))
                .bg(cocoa.surface_1)
                .border_1()
                .border_color(cocoa.line);
            panel = panel.child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(14.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("Dünyayı paylaş"),
                    )
                    .child(
                        div()
                            .text_size(TEXT_11)
                            .text_color(cocoa.text_3)
                            .child(if sharing {
                                "bore.pub tüneli açık. Kodu arkadaşına gönder."
                            } else {
                                "Tek tıkla bore.pub tüneli açılır; arkadaşın kod ile uzaktan bağlanır."
                            }),
                    )
                    .when(sharing, |this| {
                        this.child(
                            div()
                                .pt_1()
                                .text_size(px(13.))
                                .font_family("JetBrains Mono")
                                .text_color(cocoa.amber)
                                .child(
                                    social
                                        .invite
                                        .as_ref()
                                        .map(|i| i.code.clone())
                                        .unwrap_or_default(),
                                ),
                        )
                    })
                    .when(sharing, |this| {
                        this.child(
                            div()
                                .text_size(TEXT_10)
                                .font_family("JetBrains Mono")
                                .text_color(cocoa.text_3)
                                .child(
                                    social
                                        .host_relay
                                        .as_ref()
                                        .map(|r| r.error.clone())
                                        .unwrap_or_default(),
                                ),
                        )
                    }),
            );
            panel = panel.child(if sharing {
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("share-copy")
                            .outline()
                            .small()
                            .label("Kodu kopyala")
                            .icon(IconName::Copy)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let code = this
                                    .social
                                    .invite
                                    .as_ref()
                                    .map(|i| i.code.clone())
                                    .unwrap_or_default();
                                cx.write_to_clipboard(ClipboardItem::new_string(code));
                                this.toast("Davet kodu kopyalandı.", ToastKind::Success, cx);
                            })),
                    )
                    .child(
                        Button::new("share-stop")
                            .ghost()
                            .small()
                            .label("Kapat")
                            .on_click(cx.listener(|this, _, _, cx| this.stop_sharing(cx))),
                    )
                    .into_any_element()
            } else {
                Button::new("share-start")
                    .primary()
                    .small()
                    .label("Paylaş")
                    .icon(IconName::Share2)
                    .on_click(cx.listener(|this, _, _, cx| this.start_sharing(cx)))
                    .into_any_element()
            });
            panel
        };

        let add_row = h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(div().flex_1().min_w_0().child(Input::new(&self.friend_input)))
            .child(
                Button::new("add-friend")
                    .primary()
                    .label("Katıl")
                    .icon(IconName::Play)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let code = this.friend_input.read(cx).value().to_string();
                        this.join_invite(code, cx);
                    })),
            );

        let mut body = v_flex().w_full().gap_3();

        if !social.requests.incoming.is_empty() {
            let mut list = v_flex().w_full().gap_2();
            for request in social.requests.incoming.clone() {
                let player_id = request.player_id.clone();
                let label = if request.name.is_empty() {
                    request.player_id.clone()
                } else {
                    request.name.clone()
                };
                list = list.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .p_3()
                        .rounded(px(6.))
                        .bg(cocoa.surface_2)
                        .border_1()
                        .border_color(Hsla::from(rgba(0xe6a05252)))
                        .child(
                            v_flex()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(TEXT_12)
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(format!("{label} istek gönderdi")),
                                )
                                .child(
                                    div()
                                        .text_size(TEXT_10)
                                        .text_color(cocoa.text_3)
                                        .child("Karşılıklı arkadaş olmak için kabul et."),
                                ),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Button::new(format!("accept-{player_id}"))
                                        .small()
                                        .primary()
                                        .label("Kabul et")
                                        .on_click(cx.listener({
                                            let player_id = player_id.clone();
                                            move |this, _, _, cx| {
                                                this.respond_friend(player_id.clone(), true, cx)
                                            }
                                        })),
                                )
                                .child(
                                    Button::new(format!("decline-{player_id}"))
                                        .small()
                                        .ghost()
                                        .label("Yok say")
                                        .on_click(cx.listener({
                                            let player_id = player_id.clone();
                                            move |this, _, _, cx| {
                                                this.respond_friend(player_id.clone(), false, cx)
                                            }
                                        })),
                                ),
                        ),
                );
            }
            body = body.child(section_title("Bekleyen istekler", cx)).child(list);
        }

        let friends = social.friends.clone();
        body = body.child(section_title(
            if friends.is_empty() { "Arkadaşlar" } else { "Çevrimiçi arkadaşlar" },
            cx,
        ));

        if friends.is_empty() {
            body = body.child(empty_state(
                "Henüz arkadaş yok",
                "Arkadaş kodunu paylaş ya da birinden gelen kodu yukarıdaki alana yaz.",
                "☺",
                cx,
            ));
        } else {
            let mut list = v_flex().w_full().gap_1();
            for friend in &friends {
                list = list.child(friend_row(friend, cx));
            }
            body = body.child(list);
        }

        v_flex()
            .id("friends")
            .size_full()
            .overflow_y_scroll()
            .p(px(20.))
            .gap_4()
            .child(header)
            .child(identity)
            .child(share_panel)
            .child(add_row)
            .child(body)
            .into_any_element()
    }

    pub(crate) fn start_sharing(&mut self, cx: &mut Context<Self>) {
        let port = 25565u16;
        let world = self
            .worlds
            .first()
            .map(|w| w.name.clone())
            .unwrap_or_else(|| "Dünya".to_string());
        self.request(
            cx,
            move |core| core.share_world(port, &world),
            |this, result, cx| {
                match result {
                    Ok(code) => {
                        this.toast("Dünya paylaşıldı.", ToastKind::Success, cx);
                        this.logs.push(format!("Davet kodu: {code}"));
                    }
                    Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
                }
                this.refresh_social(cx);
            },
        );
    }

    pub(crate) fn stop_sharing(&mut self, cx: &mut Context<Self>) {
        self.core.stop_sharing();
        self.toast("Paylaşım kapatıldı.", ToastKind::Info, cx);
        self.refresh_social(cx);
    }

    pub(crate) fn join_invite(&mut self, code: String, cx: &mut Context<Self>) {
        if code.trim().is_empty() {
            self.toast("Davet kodu boş olamaz.", ToastKind::Info, cx);
            return;
        }
        self.request(
            cx,
            move |core| core.join_invite(&code),
            |this, result, cx| match result {
                Ok(address) => {
                    this.toast(
                        format!("Tünel hazır · {address}"),
                        ToastKind::Success,
                        cx,
                    );
                    this.launch_into(address, cx);
                }
                Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
            },
        );
    }

    pub(crate) fn add_friend(&mut self, code: String, cx: &mut Context<Self>) {
        let code = code.trim().to_string();
        if code.is_empty() {
            self.toast("Arkadaş kodu boş olamaz.", ToastKind::Info, cx);
            return;
        }
        self.request(
            cx,
            move |core| core.add_friend(&code),
            move |this, result, cx| match result {
                Ok(()) => {
                    this.toast("Arkadaş eklendi.", ToastKind::Success, cx);
                    this.refresh_social(cx);
                }
                Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
            },
        );
    }

    pub(crate) fn respond_friend(
        &mut self,
        player_id: String,
        accept: bool,
        cx: &mut Context<Self>,
    ) {
        self.request(
            cx,
            move |core| if accept { core.add_friend(&player_id) } else { core.remove_friend(&player_id) },
            |this, result, cx| match result {
                Ok(()) => {
                    this.toast("Liste güncellendi.", ToastKind::Success, cx);
                    this.refresh_social(cx);
                }
                Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
            },
        );
    }
}

fn section_title(text: &'static str, cx: &App) -> AnyElement {
    let cocoa = Cocoa::get(cx);
    div()
        .pt_2()
        .text_size(px(10.))
        .font_family("JetBrains Mono")
        .text_color(cocoa.text_3)
        .child(text)
        .into_any_element()
}

fn friend_row(friend: &SocialFriend, cx: &App) -> AnyElement {
    let cocoa = Cocoa::get(cx);
    let initial = friend
        .name
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_else(|| "•".into());
    let label = if friend.name.is_empty() {
        friend.player_id.clone()
    } else {
        friend.name.clone()
    };

    h_flex()
        .w_full()
        .items_center()
        .gap_3()
        .p_3()
        .rounded(px(6.))
        .bg(cocoa.surface_1)
        .border_1()
        .border_color(cocoa.line)
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(px(32.))
                .h(px(32.))
                .flex_shrink_0()
                .rounded(px(5.))
                .bg(cocoa.surface_3)
                .text_size(TEXT_12)
                .font_weight(FontWeight::BOLD)
                .text_color(cocoa.amber)
                .child(initial),
        )
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap(px(1.))
                .child(
                    div()
                        .text_size(TEXT_12)
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .truncate()
                        .child(label),
                )
                .child(
                    div()
                        .text_size(TEXT_10)
                        .text_color(cocoa.text_3)
                        .child(if friend.detail.is_empty() {
                            friend.status.clone()
                        } else {
                            friend.detail.clone()
                        }),
                ),
        )
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(status_dot(
                    if friend.online { cocoa.amber } else { cocoa.line_strong },
                    None,
                ))
                .child(
                    div()
                        .text_size(TEXT_10)
                        .text_color(if friend.online { cocoa.amber } else { cocoa.text_3 })
                        .child(if friend.online { "Çevrimiçi" } else { "Çevrimdışı" }),
                ),
        )
        .when(friend.online, |this| {
            this.child(
                h_flex()
                    .items_center()
                    .gap_1()
                    .text_size(TEXT_10)
                    .text_color(cocoa.text_3)
                    .child(Icon::new(IconName::Play).size(px(13.)).text_color(cocoa.text_3))
                    .child("Katıl"),
            )
        })
        .into_any_element()
}
