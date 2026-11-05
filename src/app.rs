//! Application shell: owns the Rust core, the polled engine state, and the
//! window chrome (brand bar, navigation rail, status bar). Feature screens
//! render from that state in `views/`.

use std::time::{Duration, Instant};

use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    button::{Button, ButtonVariants as _},
    input::InputState,
    slider::{SliderEvent, SliderState},
    WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::assets::IconName;
use gpui_kit::*;

use crate::api::*;
use crate::core::Core;
use crate::theme::Cocoa;
use crate::widgets::*;

pub mod views;

actions!(
    launcher,
    [
        GoPlay,
        GoInstances,
        GoModrinth,
        GoGallery,
        GoConsole,
        GoFriends,
        FocusSearch,
        LaunchGame,
        ShowShortcuts
    ]
);

/// Bound once at startup: the same chords the status bar advertises.
pub fn init_keybindings(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("ctrl-1", GoPlay, None),
        KeyBinding::new("ctrl-2", GoInstances, None),
        KeyBinding::new("ctrl-3", GoModrinth, None),
        KeyBinding::new("ctrl-4", GoGallery, None),
        KeyBinding::new("ctrl-5", GoConsole, None),
        KeyBinding::new("ctrl-6", GoFriends, None),
        KeyBinding::new("ctrl-k", FocusSearch, None),
        KeyBinding::new("ctrl-enter", LaunchGame, None),
        KeyBinding::new("?", ShowShortcuts, None),
    ]);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tab {
    Play,
    Instances,
    Modrinth,
    Gallery,
    Console,
    Friends,
}

impl Tab {
    pub const ALL: [Tab; 6] = [
        Tab::Play,
        Tab::Instances,
        Tab::Modrinth,
        Tab::Gallery,
        Tab::Console,
        Tab::Friends,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Play => "Oyna",
            Tab::Instances => "Profiller",
            Tab::Modrinth => "Modlar",
            Tab::Gallery => "Galeri",
            Tab::Console => "Konsol",
            Tab::Friends => "Arkadaşlar",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Tab::Play => IconName::Play,
            Tab::Instances => IconName::Folder,
            Tab::Modrinth => IconName::Blocks,
            Tab::Gallery => IconName::Image,
            Tab::Console => IconName::Terminal,
            Tab::Friends => IconName::Users,
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Tab::Play => "1",
            Tab::Instances => "2",
            Tab::Modrinth => "3",
            Tab::Gallery => "4",
            Tab::Console => "5",
            Tab::Friends => "6",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Connection {
    Connecting,
    Ready(u16),
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Error,
}

pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    pub shown_at: Instant,
}

pub struct AppView {
    pub core: std::sync::Arc<Core>,
    pub connection: Connection,
    pub tab: Tab,

    pub status: StatusPayload,
    pub instances: Vec<Instance>,
    pub active_id: Option<String>,
    pub versions: Vec<MinecraftVersion>,
    pub logs: Vec<String>,
    pub social: SocialStateResponse,
    pub screenshots: Vec<Screenshot>,
    /// Decoded screenshot thumbnails, keyed by file name.
    pub thumbs: std::collections::HashMap<String, std::sync::Arc<Image>>,
    pub worlds: Vec<World>,
    /// Decoded Modrinth project icons, keyed by slug.
    pub icons: std::collections::HashMap<String, std::sync::Arc<Image>>,
    /// Which nav row the pointer is over, for the 120 ms background fade.
    pub hover_nav: Option<usize>,
    /// Which content card (profile / modrinth result) the pointer is over.
    pub hover_card: Option<String>,
    /// Which quick-action card the pointer is over.
    pub hover_quick: Option<&'static str>,
    /// Caramel button is held down (80 ms press offset).
    pub launch_pressed: bool,

    pub hits: Vec<SearchHit>,
    pub search_total: i64,
    pub search_busy: bool,
    pub search_kind: SearchKind,
    pub search_sort: String,
    pub hide_downloaded: bool,
    pub install_dependencies: bool,
    /// Which installed-content category the Modrinth panel shows.
    pub content_kind: String,
    pub content_items: Vec<crate::core::ContentEntry>,
    pub content_counts: [usize; 3],
    /// Scroll position of the content centre, reset when results change.
    pub modrinth_scroll: ScrollHandle,
    pub root_focus: FocusHandle,
    pub installing_slug: Option<String>,

    pub username: Entity<InputState>,
    pub search_input: Entity<InputState>,
    pub friend_input: Entity<InputState>,
    pub ram_gb: u32,
    pub ram_slider: Entity<SliderState>,
    pub optimize_engine: bool,
    /// Loader used by the next profile created from the Instances screen.
    pub draft_loader: String,

    pub toast: Option<Toast>,
    pub tick: u64,
    pub launch_accepted_at: Option<Instant>,
    pub error_shown: Option<String>,
    pub booted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    Mod,
    Modpack,
    Shader,
    ResourcePack,
}

impl SearchKind {
    pub const ALL: [SearchKind; 4] = [
        SearchKind::Mod,
        SearchKind::Modpack,
        SearchKind::Shader,
        SearchKind::ResourcePack,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SearchKind::Mod => "Modlar",
            SearchKind::Modpack => "Modpackler",
            SearchKind::Shader => "Shaderlar",
            SearchKind::ResourcePack => "Doku paketleri",
        }
    }

    pub fn api_value(self) -> &'static str {
        match self {
            SearchKind::Mod => "mod",
            SearchKind::Modpack => "modpack",
            SearchKind::Shader => "shader",
            SearchKind::ResourcePack => "resourcepack",
        }
    }
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let core = Core::new();
        let username = cx.new(|cx| InputState::new(window, cx).placeholder("Oyuncu adı").default_value("Steve"));
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Mod, modpack veya shader ara…"));
        let friend_input = cx.new(|cx| InputState::new(window, cx).placeholder("Arkadaş kodu (CL1-…)"));
        username.focus_handle(cx).focus(window, cx);

        let ram_slider = cx.new(|_| {
            SliderState::new()
                .min(2.)
                .max(16.)
                .step(1.)
                .default_value(4.)
        });
        cx.subscribe_in(
            &ram_slider,
            window,
            |this, _, event: &SliderEvent, _, cx| {
                if let SliderEvent::Change(value) = event {
                    this.ram_gb = value.start().round() as u32;
                    cx.notify();
                }
            },
        )
        .detach();

        let view = Self {
            core,
            connection: Connection::Connecting,
            tab: std::env::var("CL_TAB")
                .ok()
                .and_then(|name| match name.as_str() {
                    "instances" => Some(Tab::Instances),
                    "modrinth" => Some(Tab::Modrinth),
                    "gallery" => Some(Tab::Gallery),
                    "console" => Some(Tab::Console),
                    "friends" => Some(Tab::Friends),
                    _ => None,
                })
                .unwrap_or(Tab::Play),
            status: StatusPayload::default(),
            instances: Vec::new(),
            active_id: None,
            versions: Vec::new(),
            logs: Vec::new(),
            social: SocialStateResponse::default(),
            screenshots: Vec::new(),
            thumbs: std::collections::HashMap::new(),
            worlds: Vec::new(),
            icons: std::collections::HashMap::new(),
            hover_nav: None,
            hover_quick: None,
            launch_pressed: false,
            hover_card: None,
            hits: Vec::new(),
            search_total: 0,
            search_busy: false,
            search_kind: SearchKind::Mod,
            search_sort: "downloads".to_string(),
            hide_downloaded: false,
            install_dependencies: true,
            content_kind: "mod".to_string(),
            content_items: Vec::new(),
            content_counts: [0; 3],
            modrinth_scroll: ScrollHandle::new(),
            root_focus: cx.focus_handle(),
            installing_slug: None,
            username,
            search_input,
            friend_input,
            ram_gb: 4,
            ram_slider,
            optimize_engine: true,
            draft_loader: "fabric".to_string(),
            toast: None,
            tick: 0,
            launch_accepted_at: None,
            error_shown: None,
            booted: false,
        };
        view
    }

    /// Test constructor: skips the network boot so a headless window only
    /// exercises the interaction paths.
    #[cfg(test)]
    pub fn test_view(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut view = Self::new(window, cx);
        view.booted = true;
        view.connection = Connection::Ready(0);
        view
    }

    // -- lifecycle ---------------------------------------------------------

    pub fn boot(&mut self, cx: &mut Context<Self>) {
        self.connection = Connection::Ready(0);
        self.refresh_instances(cx);
        self.refresh_versions(cx);
        self.refresh_worlds(cx);
        self.refresh_social(cx);
        self.start_polling(cx);
        if let Ok(spec) = std::env::var("CL_SWITCH_AFTER") {
            if let Some((secs, id)) = spec.split_once(':') {
                if let Ok(secs) = secs.parse::<u64>() {
                    let id = id.to_string();
                    cx.spawn(async move |this, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(secs * 1000))
                            .await;
                        let _ = this.update(cx, |this, cx| this.set_active(id.clone(), cx));
                    })
                    .detach();
                }
            }
        }

        // Debug: dispatch an action through the real handler path so the
        // action wiring can be verified without a keyboard.
        if let Ok(action) = std::env::var("CL_ACTION") {
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(2500))
                    .await;
                let _ = this.update_in(cx, |this, window, cx| {
                    for name in action.split(',') {
                        match name.trim() {
                            "play" => window.dispatch_action(Box::new(GoPlay), cx),
                            "instances" => window.dispatch_action(Box::new(GoInstances), cx),
                            "modrinth" => window.dispatch_action(Box::new(GoModrinth), cx),
                            "gallery" => window.dispatch_action(Box::new(GoGallery), cx),
                            "console" => window.dispatch_action(Box::new(GoConsole), cx),
                            "friends" => window.dispatch_action(Box::new(GoFriends), cx),
                            "search" => window.dispatch_action(Box::new(FocusSearch), cx),
                            "shortcuts" => window.dispatch_action(Box::new(ShowShortcuts), cx),
                            _ => {}
                        }
                    }
                    cx.notify();
                    let _ = this;
                });
            })
            .detach();
        }

        if let Ok(slug) = std::env::var("CL_INSTALL_PACK") {
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(2000))
                    .await;
                let _ = this.update(cx, |this, cx| {
                    this.search_kind = SearchKind::Modpack;
                    this.install_hit(slug.clone(), cx);
                });
            })
            .detach();
        }

        if std::env::var("CL_AUTOLAUNCH").is_ok() {
            // Wait for the first instance list, then launch (debug affordance).
            cx.spawn(async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(2500))
                    .await;
                let _ = this.update(cx, |this, cx| this.launch(cx));
            })
            .detach();
        }
        match self.tab {
            Tab::Modrinth => {}
            Tab::Gallery => self.refresh_screenshots(cx),
            Tab::Console => self.load_logs(cx),
            _ => {}
        }
        cx.notify();
    }

    fn start_polling(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(700))
                    .await;

                let tick = match this.update(cx, |this, _| {
                    this.tick += 1;
                    this.tick
                }) {
                    Ok(tick) => tick,
                    Err(_) => break,
                };

                let want_logs = tick % 3 == 0;
                let want_social = tick % 5 == 0;
                let want_instances = tick % 2 == 0;

                let tasks = match this.update(cx, |this, cx| {
                    let core = this.core.clone();
                    let status = core.status();
                    this.apply_status(status, cx);
                    let instances = if want_instances && this.instances.iter().any(|i| i.is_installing()) {
                        Some(cx.background_spawn({
                            let core = core.clone();
                            async move { core.instances() }
                        }))
                    } else {
                        None
                    };
                    let logs = if want_logs {
                        Some(cx.background_spawn({
                            let core = core.clone();
                            async move { core.logs() }
                        }))
                    } else {
                        None
                    };
                    let social = if want_social && this.tab == Tab::Friends {
                        Some(cx.background_spawn({
                            let core = core.clone();
                            async move { core.social_state().ok() }
                        }))
                    } else {
                        None
                    };
                    (instances, logs, social)
                }) {
                    Ok(tasks) => tasks,
                    Err(_) => break,
                };

                if let Some(task) = tasks.1 {
                    let logs = task.await;
                    if this.update(cx, |this, cx| {
                        this.logs = logs;
                        cx.notify();
                    }).is_err() {
                        break;
                    }
                }
                if let Some(task) = tasks.2 {
                    let social = task.await;
                    if this.update(cx, |this, cx| {
                        if let Some(social) = social {
                            this.social = social;
                        }
                        cx.notify();
                    }).is_err() {
                        break;
                    }
                }
                if let Some(task) = tasks.0 {
                    let instances = task.await;
                    if this.update(cx, |this, cx| {
                        this.instances = instances;
                        cx.notify();
                    }).is_err() {
                        break;
                    }
                }
            }
        })
        .detach();
    }

    fn apply_status(&mut self, status: StatusPayload, cx: &mut Context<Self>) {
        // A stale "idle" snapshot right after a launch must not cancel the
        // progress UI; mirror the web client's grace window.
        let in_grace = self
            .launch_accepted_at
            .map(|at| at.elapsed() < Duration::from_millis(2500))
            .unwrap_or(false);
        if in_grace && !status.is_busy() && status.percent == 0 {
            cx.notify();
            return;
        }

        self.status = status;

        if let Some(message) = self.status.error.clone() {
            if self.error_shown.as_deref() != Some(message.as_str()) {
                self.toast(message.clone(), ToastKind::Error, cx);
                self.error_shown = Some(message);
                self.clear_error();
            }
        }

        if let Some(toast) = &self.toast {
            if toast.shown_at.elapsed() > Duration::from_secs(6) {
                self.toast = None;
            }
        }
        cx.notify();
    }

    pub fn toast(&mut self, text: impl Into<String>, kind: ToastKind, cx: &mut Context<Self>) {
        self.toast = Some(Toast {
            text: text.into(),
            kind,
            shown_at: Instant::now(),
        });
        cx.notify();
    }

    // -- data --------------------------------------------------------------

    fn request<R, F, A>(&mut self, cx: &mut Context<Self>, work: F, apply: A)
    where
        R: Send + 'static,
        F: FnOnce(std::sync::Arc<Core>) -> R + Send + 'static,
        A: FnOnce(&mut Self, R, &mut Context<Self>) + 'static,
    {
        let core = self.core.clone();
        let task = cx.background_spawn(async move { work(core) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| apply(this, result, cx));
        })
        .detach();
    }

    pub fn refresh_instances(&mut self, cx: &mut Context<Self>) {
        self.request(
            cx,
            |core| core.instances(),
            |this, result, cx| {
                {
                    this.instances = result;
                    let previous = this.active_id.clone();
                    if let Ok(wanted) = std::env::var("CL_ACTIVE") {
                        if this.active_id.as_deref() != Some(wanted.as_str())
                            && this.instances.iter().any(|i| i.id == wanted)
                        {
                            let wanted = wanted.clone();
                            this.set_active(wanted, cx);
                        }
                    }
                    let stale = this
                        .active_id
                        .as_ref()
                        .map(|id| !this.instances.iter().any(|i| &i.id == id))
                        .unwrap_or(true);
                    if stale {
                        if let Some(first) = this.instances.first().map(|i| i.id.clone()) {
                            this.set_active(first, cx);
                        }
                    } else if previous != this.active_id {
                        this.refresh_worlds(cx);
                        this.refresh_content(cx);
                        if this.tab == Tab::Modrinth {
                            this.search_modrinth(cx);
                        }
                    }
                }
            },
        );
    }

    /// Change the active profile and refresh everything that depends on it:
    /// worlds, installed content and the Modrinth search (version+loader
    /// filtered). Every call site goes through here so no view stays stale.
    pub fn set_active(&mut self, id: String, cx: &mut Context<Self>) {
        if self.active_id.as_deref() == Some(id.as_str()) {
            return;
        }
        self.active_id = Some(id);
        self.refresh_worlds(cx);
        self.refresh_content(cx);
        if self.tab == Tab::Modrinth {
            self.search_modrinth(cx);
        }
        cx.notify();
    }

    pub fn refresh_worlds(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.active_id.clone() else {
            self.worlds.clear();
            return;
        };
        self.request(
            cx,
            move |core| core.worlds(&id),
            |this, result, cx| {
                this.worlds = result;
                cx.notify();
            },
        );
    }

    pub fn refresh_versions(&mut self, cx: &mut Context<Self>) {
        self.request(
            cx,
            |core| core.versions(),
            |this, result, cx| match result {
                Ok(versions) => {
                    this.versions = versions
                        .into_iter()
                        .filter(|version| version.kind == "release")
                        .map(|v| MinecraftVersion {
                            id: v.id,
                            kind: v.kind,
                            release_time: v.release_time,
                            is_installed: v.is_installed,
                        })
                        .collect();
                }
                Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
            },
        );
    }

    pub fn refresh_social(&mut self, cx: &mut Context<Self>) {
        self.request(
            cx,
            |core| core.social_state(),
            |this, result, cx| {
                if let Ok(social) = result {
                    this.social = social;
                }
                cx.notify();
            },
        );
    }

    pub fn refresh_screenshots(&mut self, cx: &mut Context<Self>) {
        self.request(
            cx,
            |core| core.screenshots(),
            |this, result, cx| {
                this.screenshots = result;
                this.load_thumbnails(cx);
                cx.notify();
            },
        );
    }

    /// Screenshot bytes are served by the core; decode the first few so the
    /// gallery shows the actual images instead of placeholders.
    fn load_thumbnails(&mut self, cx: &mut Context<Self>) {
        let core = self.core.clone();
        let names: Vec<String> = self
            .screenshots
            .iter()
            .take(24)
            .map(|shot| shot.name.clone())
            .filter(|name| !self.thumbs.contains_key(name))
            .collect();
        if names.is_empty() {
            return;
        }
        let task = cx.background_spawn(async move {
            let mut out: Vec<(String, Vec<u8>)> = Vec::new();
            for name in names {
                if let Ok(bytes) = core.screenshot_bytes(&name) {
                    if !bytes.is_empty() {
                        out.push((name, bytes));
                    }
                }
            }
            out
        });
        cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let _ = this.update(cx, |this, cx| {
                for (name, bytes) in loaded {
                    this.thumbs
                        .insert(name, std::sync::Arc::new(Image::from_bytes(ImageFormat::Png, bytes)));
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Installed content for the active profile in the selected category.
    pub fn refresh_content(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.active_id.clone() else {
            self.content_items.clear();
            return;
        };
        let kind = self.content_kind.clone();
        self.request(
            cx,
            move |core| (core.content(&id, &kind), core.content_counts(&id)),
            |this, (items, counts), cx| {
                this.content_items = items;
                this.content_counts = counts;
                cx.notify();
            },
        );
    }

    pub fn toggle_content(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(id) = self.active_id.clone() else {
            return;
        };
        let kind = self.content_kind.clone();
        self.request(
            cx,
            move |core| core.toggle_content(&id, &kind, &name),
            |this, result, cx| {
                if let Err(error) = result {
                    this.toast(error.to_string(), ToastKind::Error, cx);
                }
                this.refresh_content(cx);
            },
        );
    }

    pub fn delete_content(&mut self, name: String, cx: &mut Context<Self>) {
        let Some(id) = self.active_id.clone() else {
            return;
        };
        let kind = self.content_kind.clone();
        self.request(
            cx,
            move |core| core.delete_content(&id, &kind, &name),
            |this, result, cx| {
                match result {
                    Ok(()) => this.toast("İçerik silindi.", ToastKind::Success, cx),
                    Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
                }
                this.refresh_content(cx);
            },
        );
    }

    pub fn load_logs(&mut self, cx: &mut Context<Self>) {
        self.request(
            cx,
            |core| core.logs(),
            |this, result, cx| {
                this.logs = result;
                cx.notify();
            },
        );
    }

    // -- actions -----------------------------------------------------------

    pub fn active_instance(&self) -> Option<&Instance> {
        let id = self.active_id.as_ref()?;
        self.instances.iter().find(|instance| &instance.id == id)
    }

    pub fn launch_mode(&self) -> LaunchMode {
        if self.status.is_running {
            LaunchMode::Running
        } else if self.status.is_installing {
            LaunchMode::Installing
        } else if self.active_instance().is_none() {
            LaunchMode::NoProfile
        } else {
            LaunchMode::Idle
        }
    }

    pub fn launch(&mut self, cx: &mut Context<Self>) {
        if self.status.is_busy() {
            self.toast("Oyun zaten hazırlanıyor.", ToastKind::Info, cx);
            return;
        }
        let Some(instance) = self.active_instance().cloned() else {
            self.toast("Önce bir profil oluşturun.", ToastKind::Info, cx);
            self.tab = Tab::Instances;
            cx.notify();
            return;
        };
        let username = self.username.read(cx).value().to_string();
        let request = LaunchRequest {
            instance_id: Some(instance.id.clone()),
            username: if username.trim().is_empty() {
                "Steve".to_string()
            } else {
                username
            },
            ram: self.ram_gb as i64,
            cookie_optimized: true,
            jvm_preset: "aikar".to_string(),
            quick_play: None,
        };

        self.request(
            cx,
            move |core| core.launch(request.clone()),
            |this, result, cx| match result {
                Ok(()) => {
                    this.launch_accepted_at = Some(Instant::now());
                    this.toast("Başlatma isteği alındı.", ToastKind::Success, cx);
                }
                Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
            },
        );
    }

    /// Launch the active profile straight into a friend's world
    /// (`--quickPlayMultiplayer`).
    pub fn launch_into(&mut self, address: String, cx: &mut Context<Self>) {
        if self.status.is_busy() {
            self.toast("Oyun zaten hazırlanıyor.", ToastKind::Info, cx);
            return;
        }
        let Some(instance) = self.active_instance().cloned() else {
            self.toast("Önce bir profil oluşturun.", ToastKind::Info, cx);
            return;
        };
        let username = self.username.read(cx).value().to_string();
        let request = LaunchRequest {
            instance_id: Some(instance.id),
            username: if username.trim().is_empty() {
                "Steve".to_string()
            } else {
                username
            },
            ram: self.ram_gb as i64,
            cookie_optimized: true,
            jvm_preset: "aikar".to_string(),
            quick_play: Some(address.clone()),
        };
        self.request(
            cx,
            move |core| core.launch(request),
            move |this, result, cx| match result {
                Ok(()) => {
                    this.launch_accepted_at = Some(Instant::now());
                    this.toast(
                        format!("Arkadaş dünyasına bağlanılıyor: {address}"),
                        ToastKind::Success,
                        cx,
                    );
                }
                Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
            },
        );
    }

    pub fn delete_instance(&mut self, id: String, name: String, cx: &mut Context<Self>) {
        let target = id.clone();
        self.request(
            cx,
            move |core| core.delete_instance(&target),
            move |this, result, cx| match result {
                Ok(_) => {
                    this.toast(format!("“{name}” silindi."), ToastKind::Success, cx);
                    this.refresh_instances(cx);
                }
                Err(error) => this.toast(error.to_string(), ToastKind::Error, cx),
            },
        );
    }

    pub fn open_folder(&mut self, folder: &str, cx: &mut Context<Self>) {
        let folder = folder.to_string();
        self.request(
            cx,
            move |core| core.open_folder(&folder),
            |this, result, cx| {
                if let Err(error) = result {
                    this.toast(error.to_string(), ToastKind::Error, cx);
                }
            },
        );
    }

    fn clear_error(&mut self) {
        self.core.clear_error();
    }

    // -- chrome ------------------------------------------------------------

    fn go_to(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        match tab {
            Tab::Modrinth => {
                if self.hits.is_empty() {
                    self.search_modrinth(cx);
                }
                self.refresh_content(cx);
            }
            Tab::Gallery if self.screenshots.is_empty() => self.refresh_screenshots(cx),
            Tab::Console => self.load_logs(cx),
            Tab::Friends => self.refresh_social(cx),
            _ => {}
        }
        cx.notify();
    }

    fn shortcuts_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        const SHORTCUTS: [(&str, &str); 9] = [
            ("Ctrl+1", "Oyna"),
            ("Ctrl+2", "Profiller"),
            ("Ctrl+3", "Modlar"),
            ("Ctrl+4", "Galeri"),
            ("Ctrl+5", "Konsol"),
            ("Ctrl+6", "Arkadaşlar"),
            ("Ctrl+K", "Aramaya odaklan"),
            ("Ctrl+Enter", "Oyunu başlat"),
            ("?", "Bu pencere"),
        ];
        window.open_dialog(cx, |dialog, _, app| {
            let cocoa = Cocoa::get(app);
            let mut list = v_flex().gap_1();
            for (keys, action) in SHORTCUTS {
                list = list.child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .justify_between()
                        .gap_4()
                        .py_1()
                        .child(
                            div()
                                .text_size(TEXT_12)
                                .text_color(cocoa.text_2)
                                .child(action),
                        )
                        .child(
                            div()
                                .px_2()
                                .py(px(1.))
                                .rounded(px(4.))
                                .border_1()
                                .border_color(cocoa.line_strong)
                                .text_size(px(10.))
                                .font_family("JetBrains Mono")
                                .text_color(cocoa.text_3)
                                .child(keys),
                        ),
                );
            }
            dialog.title("Kısayollar").w(px(380.)).child(list).footer(
                gpui_kit::component::dialog::DialogFooter::new().child(
                    Button::new("shortcuts-close")
                        .label("Kapat")
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
        });
    }

    fn render_brand_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();

        h_flex()
            .w_full()
            .h(px(44.))
            .flex_shrink_0()
            .items_center()
            .pl(px(16.))
            .pr(px(8.))
            .gap_3()
            .bg(cocoa.chrome)
            .border_b_1()
            .border_color(cocoa.line)
            .child(wordmark(px(13.), cx))
            .child(tag("Java Edition", cx))
            .child(
                Button::new("folders")
                    .ghost()
                    .xsmall()
                    .label("Klasörler")
                    .icon(IconName::ChevronDown)
                    .on_click(cx.listener(|this, _, window, cx| this.folder_dialog(window, cx))),
            )
            .child(
                div()
                    .id("drag-region")
                    .flex_1()
                    .h_full()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move()),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap(px(2.))
                    .child(window_control("win-min", IconName::Minus, false, |window| {
                        window.minimize_window()
                    }))
                    .child(window_control("win-max", IconName::Square, false, |window| {
                        window.zoom_window()
                    }))
                    .child(window_control("win-close", IconName::X, true, |window| {
                        window.remove_window()
                    })),
            )
            .into_any_element()
    }

    fn folder_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        const FOLDERS: [(&str, &str); 6] = [
            ("", ".minecraft ana klasörü"),
            ("mods", "mods klasörü"),
            ("saves", "saves (dünyalar)"),
            ("screenshots", "screenshots klasörü"),
            ("resourcepacks", "resourcepacks klasörü"),
            ("shaderpacks", "shaderpacks klasörü"),
        ];
        let entity = cx.entity();
        window.open_dialog(cx, move |dialog, _, _| {
            let entity = entity.clone();
            let mut list = v_flex().gap_1();
            for (path, label) in FOLDERS {
                list = list.child(
                    Button::new(format!("folder-{path}"))
                        .ghost()
                        .w_full()
                        .justify_start()
                        .label(label)
                        .on_click({
                            let entity = entity.clone();
                            move |_, _, cx| {
                                entity.update(cx, |this, cx| this.open_folder(path, cx));
                            }
                        }),
                );
            }
            dialog
                .title("Klasörler")
                .w(px(380.))
                .child(list)
                .footer(
                    gpui_kit::component::dialog::DialogFooter::new().child(
                        Button::new("folders-close").label("Kapat").on_click(
                            |_, window, cx| window.close_dialog(cx),
                        ),
                    ),
                )
        });
    }

    fn render_rail(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let foreground = cx.theme().foreground;
        let active_instance = self.active_instance().cloned();

        let mut menu = v_flex().gap(px(3.)).px(px(10.)).py(px(6.)).w_full();

        for tab in Tab::ALL {
            let selected = self.tab == tab;
            let icon_color = if selected { cocoa.amber } else { cocoa.text_3 };
            let label_color = if selected { foreground } else { cocoa.text_3 };
            let entity = cx.entity();

            let hovered = self.hover_nav == Some(tab as usize);
            let base_bg = if selected {
                cocoa.surface_2
            } else {
                Hsla::from(rgba(0x00000000))
            };
            let hover_bg = if selected {
                cocoa.surface_2
            } else {
                Hsla::from(rgba(0xffebd209))
            };
            let (from, to) = if hovered {
                (base_bg, hover_bg)
            } else {
                (hover_bg, base_bg)
            };

            let row = div()
                .id(("nav", tab as usize))
                .relative()
                .flex()
                .items_center()
                .gap_3()
                .w_full()
                .h(px(44.))
                .px(px(12.))
                .rounded(px(5.))
                .cursor_pointer()
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(label_color)
                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                    let next = if *hovered { Some(tab as usize) } else { None };
                    if this.hover_nav != next {
                        this.hover_nav = next;
                        cx.notify();
                    }
                }))
                .on_click(move |_, _, cx| {
                    entity.update(cx, |this, cx| {
                        this.tab = tab;
                        match tab {
                            Tab::Gallery if this.screenshots.is_empty() => this.refresh_screenshots(cx),
                            Tab::Console => this.load_logs(cx),
                            Tab::Friends => this.refresh_social(cx),
                            Tab::Modrinth if this.hits.is_empty() => this.search_modrinth(cx),
                            _ => {}
                        }
                        cx.notify();
                    });
                })
                .when(selected, |this| {
                    this.child(
                        div()
                            .absolute()
                            .left(px(-2.))
                            .top(px(11.))
                            .bottom(px(11.))
                            .w(px(3.))
                            .rounded_r(px(2.))
                            .bg(cocoa.amber),
                    )
                })
                .child(Icon::new(tab.icon()).size(px(18.)).text_color(icon_color))
                .child(div().flex_1().child(tab.label()))
                .child(
                    div()
                        .text_size(px(9.))
                        .font_family("JetBrains Mono")
                        .text_color(if selected { cocoa.text_3 } else { cocoa.chrome })
                        .child(tab.shortcut()),
                );

            // CSS `.nav-tab { transition: background .12s ease }`
            menu = menu.child(row.fade_bg(SharedString::from(format!("nav-bg-{}-{hovered}", tab as usize)), from, to, 120));
        }

        let (profile_name, profile_version, profile_loader, profile_initial, has_profile) =
            match &active_instance {
                Some(instance) => (
                    instance.name.clone(),
                    instance.version.clone(),
                    instance.loader_label().to_string(),
                    instance
                        .name
                        .chars()
                        .next()
                        .map(|c| c.to_uppercase().to_string())
                        .unwrap_or_else(|| "?".into()),
                    true,
                ),
                None => (
                    "Profil yok".to_string(),
                    "MC • —".to_string(),
                    String::new(),
                    "?".to_string(),
                    false,
                ),
            };

        let (state_color, state_text) = if self.status.is_running {
            (cocoa.info, "Oyun çalışıyor")
        } else if self.status.is_installing {
            (cocoa.amber, "Hazırlanıyor…")
        } else if matches!(self.connection, Connection::Ready(_)) {
            (cocoa.amber, "Core hazır")
        } else {
            (cocoa.danger, "Core bekleniyor...")
        };

        v_flex()
            .id("rail")
            .w(px(218.))
            .h_full()
            .flex_shrink_0()
            .bg(cocoa.chrome)
            .border_r_1()
            .border_color(cocoa.line)
            .child(
                h_flex()
                    .items_center()
                    .gap(px(11.))
                    .px(px(16.))
                    .pt(px(18.))
                    .pb(px(16.))
                    .child(cookie_mark(26., cocoa.amber))
                    .child(wordmark_compact(px(10.), cx)),
            )
            .child(menu)
            .child(div().flex_1())
            .child(
                v_flex()
                    .gap(px(10.))
                    .px_3()
                    .pt_3()
                    .pb(px(14.))
                    .border_t_1()
                    .border_color(cocoa.line)
                    .child(
                        h_flex()
                            .items_center()
                            .gap(px(10.))
                            .p(px(10.))
                            .rounded(px(6.))
                            .bg(cocoa.surface_1)
                            .border_1()
                            .border_color(cocoa.line)
                            .when(!has_profile, |this| this.opacity(0.65))
                            .child(icon_tile(34., 5., profile_initial, cx))
                            .child(
                                v_flex()
                                    .min_w_0()
                                    .flex_1()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(TEXT_12)
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(foreground)
                                            .truncate()
                                            .child(profile_name),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .font_family("JetBrains Mono")
                                            .text_color(cocoa.text_3)
                                            .truncate()
                                            .child(profile_version),
                                    )
                                    .when(!profile_loader.is_empty(), |this| {
                                        this.child(
                                            div()
                                                .text_size(px(10.))
                                                .text_color(cocoa.text_3)
                                                .truncate()
                                                .child(profile_loader),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .px(px(2.))
                            .child(if self.status.is_busy() {
                                status_dot_busy(state_color).into_any_element()
                            } else {
                                status_dot(
                                    state_color,
                                    Some(if self.status.is_running {
                                        Hsla::from(rgba(0x8fb0cf1f))
                                    } else if matches!(self.connection, Connection::Ready(_)) {
                                        Hsla::from(rgba(0xe6a0521a))
                                    } else {
                                        Hsla::from(rgba(0xd76a5b1a))
                                    }),
                                )
                                .into_any_element()
                            })
                            .child(
                                div()
                                    .text_size(TEXT_10)
                                    .text_color(cocoa.text_3)
                                    .child(state_text),
                            ),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .px(px(2.))
                            .child(
                                div()
                                    .text_size(TEXT_10)
                                    .text_color(cocoa.text_3)
                                    .child("Kısayollar"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(21.))
                                    .h(px(21.))
                                    .rounded(px(4.))
                                    .border_1()
                                    .border_color(cocoa.line_strong)
                                    .text_size(px(10.))
                                    .text_color(cocoa.text_3)
                                    .child("?"),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_status_bar(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let cocoa = Cocoa::get(cx).clone_shades();
        let status = &self.status;
        let (dot, left) = match &self.connection {
            Connection::Ready(_) => (
                cocoa.amber,
                if status.is_busy() {
                    format!("{} · Rust çekirdek", status.status)
                } else if status.is_running {
                    "Oyun açık · Rust çekirdek".to_string()
                } else {
                    "Core hazır · Rust".to_string()
                },
            ),
            Connection::Connecting => (cocoa.amber, "Core bekleniyor...".to_string()),
            Connection::Failed(_) => (cocoa.danger, "Core bekleniyor...".to_string()),
        };

        v_flex()
            .w_full()
            .flex_shrink_0()
            .bg(cocoa.chrome)
            .border_t_1()
            .border_color(cocoa.line)
            .when(status.is_busy(), |this| {
                this.child(
                    div().w_full().h(px(3.)).bg(cocoa.track).child(
                        div()
                            .h_full()
                            .w(relative((status.percent as f32 / 100.).clamp(0., 1.)))
                            .bg(cocoa.amber),
                    ),
                )
            })
            .child(
                h_flex()
                    .h(px(30.))
                    .items_center()
                    .justify_between()
                    .px(px(14.))
                    .gap_4()
                    .child(
                        h_flex()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .child(status_dot(dot, None))
                            .child(
                                div()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(TEXT_10)
                                    .text_color(cocoa.text_3)
                                    .child(left),
                            ),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(TEXT_10)
                            .text_color(cocoa.text_3)
                            .child("Ctrl+1-6 sekmeler · Ctrl+K ara · ? kısayollar"),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(TEXT_10)
                            .font_family("JetBrains Mono")
                            .text_color(cocoa.text_3)
                            .child("v4.0.0 · Java Edition"),
                    ),
            )
            .into_any_element()
    }

    fn render_toast(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let toast = self.toast.as_ref()?;
        let toast_id = toast.text.clone();
        let cocoa = Cocoa::get(cx).clone_shades();
        let accent = match toast.kind {
            ToastKind::Info => cocoa.info,
            ToastKind::Success => cocoa.amber,
            ToastKind::Error => cocoa.danger,
        };
        Some(
            div()
                .absolute()
                .bottom(px(46.))
                .right(px(20.))
                .max_w(px(420.))
                .px(px(14.))
                .py(px(10.))
                .rounded(px(6.))
                .bg(cocoa.surface_2)
                .border_1()
                .border_color(cocoa.line_strong)
                .border_l_2()
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(div().w(px(3.)).h(px(14.)).rounded(px(2.)).bg(accent))
                        .child(
                            div()
                                .text_size(TEXT_12)
                                .text_color(cocoa.text_2)
                                .child(toast.text.clone()),
                        ),
                )
                .with_animation(
                    SharedString::from(format!("toast-{toast_id}")),
                    Animation::new(Duration::from_millis(160))
                        .with_easing(|delta| 1.0 - (1.0 - delta).powi(5)),
                    |this, delta| div().opacity(delta).mt(px((1.0 - delta) * 10.)).child(this),
                )
                .into_any_element(),
        )
    }
}

fn percent_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Small helper so borrowed `Cocoa` values can be moved into closures.
impl Cocoa {
    pub fn clone_shades(&self) -> Cocoa {
        Cocoa {
            amber: self.amber,
            amber_hover: self.amber_hover,
            amber_ink: self.amber_ink,
            amber_soft: self.amber_soft,
            paper: self.paper,
            paper_hover: self.paper_hover,
            paper_ink: self.paper_ink,
            chrome: self.chrome,
            surface_1: self.surface_1,
            surface_2: self.surface_2,
            surface_3: self.surface_3,
            track: self.track,
            line: self.line,
            line_strong: self.line_strong,
            text_2: self.text_2,
            text_3: self.text_3,
            launch_top: self.launch_top,
            launch_bottom: self.launch_bottom,
            launch_edge: self.launch_edge,
            danger: self.danger,
            info: self.info,
        }
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.booted {
            // Kick the supervisor once, the first time the window renders.
            self.boot(cx);
            self.booted = true;
        }

        let theme = cx.theme().clone();
        let background = theme.background;
        let foreground = theme.foreground;
        let font = theme.font_family.clone();
        let size = theme.font_size;

        let content = match self.tab {
            Tab::Play => self.render_play(window, cx),
            Tab::Instances => self.render_instances(window, cx),
            Tab::Modrinth => self.render_modrinth(window, cx),
            Tab::Gallery => self.render_gallery(window, cx),
            Tab::Console => self.render_console(window, cx),
            Tab::Friends => self.render_friends(window, cx),
        };

        // 140 ms fade, same rhythm as the web build's tab switch.
        let content = content
            .with_animation(
                ("tab", self.tab as usize),
                Animation::new(Duration::from_millis(140))
                    .with_easing(|delta| 1.0 - (1.0 - delta).powi(5)),
                |element, delta| div().opacity(delta).size_full().child(element).into_any_element(),
            )
            .into_any_element();

        let toast = self.render_toast(cx);

        v_flex()
            .id("launcher-root")
            .key_context("Launcher")
            .track_focus(&self.root_focus)
            .on_action(cx.listener(|this, _: &GoPlay, _, cx| this.go_to(Tab::Play, cx)))
            .on_action(cx.listener(|this, _: &GoInstances, _, cx| this.go_to(Tab::Instances, cx)))
            .on_action(cx.listener(|this, _: &GoModrinth, _, cx| this.go_to(Tab::Modrinth, cx)))
            .on_action(cx.listener(|this, _: &GoGallery, _, cx| this.go_to(Tab::Gallery, cx)))
            .on_action(cx.listener(|this, _: &GoConsole, _, cx| this.go_to(Tab::Console, cx)))
            .on_action(cx.listener(|this, _: &GoFriends, _, cx| this.go_to(Tab::Friends, cx)))
            .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                this.tab = Tab::Modrinth;
                this.search_input.focus_handle(cx).focus(window, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &LaunchGame, _, cx| this.launch(cx)))
            .on_action(cx.listener(|this, _: &ShowShortcuts, window, cx| {
                this.shortcuts_dialog(window, cx)
            }))
            .size_full()
            .bg(background)
            .text_color(foreground)
            .font_family(font)
            .text_size(size)
            .child(self.render_brand_bar(cx))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .items_stretch()
                    .child(self.render_rail(cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .child(content),
                    ),
            )
            .child(self.render_status_bar(cx))
            .when_some(toast, |this, toast| this.child(toast))
            .into_any_element()
    }
}


impl AppView {
    /// Fetch Modrinth icons for the current result set (one round-trip per
    /// project) and decode them into the texture cache.
    pub(crate) fn load_mod_icons(&mut self, cx: &mut Context<Self>) {
        let missing: Vec<(String, String)> = self
            .hits
            .iter()
            .filter(|hit| !self.icons.contains_key(&hit.slug))
            .map(|hit| (hit.slug.clone(), hit.icon_url.clone()))
            .collect();
        if missing.is_empty() {
            return;
        }
        let core = self.core.clone();
        let task = cx.background_spawn(async move {
            let mut out: Vec<(String, Vec<u8>)> = Vec::new();
            for (slug, url) in missing {
                if let Some(bytes) = core.project_image(&slug, &url) {
                    out.push((slug, bytes));
                }
            }
            out
        });
        cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let _ = this.update(cx, |this, cx| {
                for (slug, bytes) in loaded {
                    if let Some(image) = decode_image(bytes) {
                        this.icons.insert(slug, image);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn decode_image(bytes: Vec<u8>) -> Option<std::sync::Arc<Image>> {
    let format = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        ImageFormat::Png
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        ImageFormat::Jpeg
    } else if bytes.starts_with(b"RIFF") {
        ImageFormat::Webp
    } else {
        ImageFormat::Png
    };
    Some(std::sync::Arc::new(Image::from_bytes(format, bytes)))
}
