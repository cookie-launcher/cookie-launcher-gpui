//! The launcher core, in Rust — no Python, no localhost HTTP.
//!
//! Owns the `.minecraft` layout (`instances/<id>/instance.json`), the Mojang
//! version manifest, Modrinth downloads, the launch process and the log ring
//! buffer. The UI holds an `Arc<Core>` and calls these methods from
//! `background_spawn`, so nothing here touches GPUI.

use std::collections::VecDeque;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::api::{
    Instance, LaunchRequest, Screenshot, SearchHit, SocialFriend, StatusPayload, World,
};

const USER_AGENT: &str = "CookieLauncher/4.0 (Rust; +https://github.com/cookie-launcher)";
const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

pub struct Core {
    pub mc_dir: PathBuf,
    pub instances_dir: PathBuf,
    pub data_dir: PathBuf,
    agent: ureq::Agent,
    logs: Mutex<VecDeque<String>>,
    identity: std::sync::OnceLock<Identity>,
    share: Mutex<Option<ShareSession>>,
    launch_pid: Mutex<Option<u32>>,
    joined: Mutex<Option<String>>,
    lan_worlds: Arc<Mutex<Vec<LanWorld>>>,
    status: Mutex<StatusPayload>,
    launch_child: Mutex<Option<Child>>,
}

impl Core {
    pub fn new() -> Arc<Self> {
        Core::with_paths(minecraft_dir(), data_dir())
    }

    /// Explicit roots — used by tests so they never touch `~/.minecraft`.
    pub fn with_paths(mc_dir: PathBuf, data_dir: PathBuf) -> Arc<Self> {
        let instances_dir = mc_dir.join("instances");
        let _ = fs::create_dir_all(&instances_dir);
        let _ = fs::create_dir_all(&data_dir);

        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent(USER_AGENT)
            .build();

        let core = Arc::new(Self {
            mc_dir,
            instances_dir,
            data_dir,
            agent: ureq::Agent::new_with_config(config),
            logs: Mutex::new(VecDeque::new()),
            identity: std::sync::OnceLock::new(),
            share: Mutex::new(None),
            launch_pid: Mutex::new(None),
            joined: Mutex::new(None),
            lan_worlds: Arc::new(Mutex::new(Vec::new())),
            status: Mutex::new(StatusPayload {
                status: "Hazır".to_string(),
                ..Default::default()
            }),
            launch_child: Mutex::new(None),
        });
        core.log("Çekirdek hazır (Rust).");
        core
    }

    // -- logging -----------------------------------------------------------

    pub fn log(&self, message: impl AsRef<str>) {
        let stamp = clock();
        let line = format!("[{stamp}] {}", message.as_ref());
        if let Ok(mut logs) = self.logs.lock() {
            logs.push_back(line);
            while logs.len() > 1000 {
                logs.pop_front();
            }
        }
    }

    pub fn logs(&self) -> Vec<String> {
        self.logs
            .lock()
            .map(|logs| logs.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn status(&self) -> StatusPayload {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }

    pub(crate) fn set_status(&self, f: impl FnOnce(&mut StatusPayload)) {
        if let Ok(mut status) = self.status.lock() {
            f(&mut status);
        }
    }

    pub fn clear_error(&self) {
        self.set_status(|status| status.error = None);
    }

    // -- instances ---------------------------------------------------------

    pub fn instances(&self) -> Vec<Instance> {
        let Ok(entries) = fs::read_dir(&self.instances_dir) else {
            return Vec::new();
        };
        let mut out: Vec<Instance> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| read_instance(&entry.path()))
            .collect();
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        out
    }

    pub fn instance_dir(&self, id: &str) -> PathBuf {
        self.instances_dir.join(id)
    }

    fn ensure_unique_id(&self, name: &str) -> String {
        let base = slugify(name);
        let mut candidate = base.clone();
        let mut counter = 2;
        while self.instance_dir(&candidate).exists() {
            candidate = format!("{base}_{counter}");
            counter += 1;
        }
        candidate
    }

    pub fn create_instance(
        &self,
        name: &str,
        version: &str,
        loader: &str,
        icon: &str,
    ) -> Result<Instance> {
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("Geçersiz profil adı."));
        }
        let version = version.trim();
        // Same guard the Python core had: the value becomes a directory name
        // inside `versions/`, so keep it to an id-shaped string.
        if version.is_empty()
            || version.len() > 40
            || !version
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        {
            return Err(anyhow!("Geçersiz Minecraft sürümü: “{version}”."));
        }
        if self.instances().iter().any(|i| i.name.eq_ignore_ascii_case(name)) {
            return Err(anyhow!("Bu isimde bir profil zaten var."));
        }
        let id = self.ensure_unique_id(name);
        let dir = self.instance_dir(&id);
        fs::create_dir_all(dir.join("mods"))?;
        fs::create_dir_all(dir.join("saves"))?;

        let instance = Instance {
            id: id.clone(),
            name: name.to_string(),
            version: version.to_string(),
            loader: loader.to_string(),
            loader_version: String::new(),
            icon: icon.to_string(),
            created_at: now_string(),
            mod_count: 0,
            ..Default::default()
        };
        write_instance(&dir, &instance)?;
        self.log(format!("Profil oluşturuldu: {name} ({id})"));
        Ok(instance)
    }

    pub fn delete_instance(&self, id: &str) -> Result<()> {
        let dir = self.instance_dir(id);
        if !dir.exists() {
            return Err(anyhow!("Profil bulunamadı."));
        }
        if self.status().launch_instance_id.as_deref() == Some(id) {
            return Err(anyhow!("Bu profilin oyunu çalışıyor; önce oyunu kapatın."));
        }
        fs::remove_dir_all(&dir).context("Profil silinemedi")?;
        self.log(format!("Profil silindi: {id}"));
        Ok(())
    }

    pub fn update_instance(&self, id: &str, name: &str, version: &str, loader: &str) -> Result<()> {
        let dir = self.instance_dir(id);
        let mut instance = read_instance(&dir).ok_or_else(|| anyhow!("Profil bulunamadı."))?;
        if !name.trim().is_empty() {
            instance.name = name.trim().to_string();
        }
        if !version.trim().is_empty() {
            instance.version = version.trim().to_string();
        }
        if !loader.trim().is_empty() {
            instance.loader = loader.trim().to_string();
        }
        write_instance(&dir, &instance)
    }

    // -- instance content --------------------------------------------------

    pub fn worlds(&self, id: &str) -> Vec<World> {
        let saves = self.instance_dir(id).join("saves");
        let Ok(entries) = fs::read_dir(&saves) else {
            return Vec::new();
        };
        let mut worlds: Vec<World> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_dir())
            .map(|entry| {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let size = dir_size(&path);
                let last_played = fs::metadata(path.join("level.dat"))
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(format_time)
                    .unwrap_or_default();
                World {
                    name,
                    size_mb: (size as f64 / 1_048_576.0 * 100.0).round() / 100.0,
                    last_played,
                    has_icon: path.join("icon.png").exists(),
                }
            })
            .collect();
        worlds.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        worlds
    }

    pub fn content(&self, id: &str, kind: &str) -> Vec<ContentEntry> {
        let dir = self.instance_dir(id).join(match kind {
            "shader" => "shaderpacks",
            "resourcepack" => "resourcepacks",
            _ => "mods",
        });
        let Ok(entries) = fs::read_dir(&dir) else {
            return Vec::new();
        };
        let mut out: Vec<ContentEntry> = entries
            .filter_map(|entry| entry.ok())
            .map(|entry| {
                let name = entry.file_name().to_string_lossy().to_string();
                ContentEntry {
                    enabled: !name.ends_with(".disabled"),
                    name,
                }
            })
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// Item counts for mods / shaders / resource packs of one profile.
    pub fn content_counts(&self, id: &str) -> [usize; 3] {
        ["mod", "shader", "resourcepack"].map(|kind| self.content(id, kind).len())
    }

    pub fn toggle_content(&self, id: &str, kind: &str, name: &str) -> Result<()> {
        let dir = self.instance_dir(id).join(match kind {
            "shader" => "shaderpacks",
            "resourcepack" => "resourcepacks",
            _ => "mods",
        });
        let from = self.checked_child(&dir, name)?;
        let to = if name.ends_with(".disabled") {
            self.checked_child(&dir, name.trim_end_matches(".disabled"))?
        } else {
            self.checked_child(&dir, &format!("{name}.disabled"))?
        };
        fs::rename(&from, &to).context("İçerik durumu değiştirilemedi")?;
        Ok(())
    }

    pub fn delete_content(&self, id: &str, kind: &str, name: &str) -> Result<()> {
        let dir = self.instance_dir(id).join(match kind {
            "shader" => "shaderpacks",
            "resourcepack" => "resourcepacks",
            _ => "mods",
        });
        let path = self.checked_child(&dir, name)?;
        if path.is_dir() {
            fs::remove_dir_all(&path)?;
        } else if path.exists() {
            fs::remove_file(&path)?;
        } else {
            return Err(anyhow!("İçerik bulunamadı."));
        }
        Ok(())
    }

    // -- screenshots -------------------------------------------------------

    pub fn screenshots(&self) -> Vec<Screenshot> {
        let dir = self.mc_dir.join("screenshots");
        let Ok(entries) = fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut shots: Vec<Screenshot> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("png"))
                    .unwrap_or(false)
            })
            .map(|entry| {
                let meta = entry.metadata().ok();
                Screenshot {
                    name: entry.file_name().to_string_lossy().to_string(),
                    size: meta.as_ref().map(|m| m.len() as i64).unwrap_or(0),
                    mtime: meta
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                        .map(|d| d.as_secs_f64())
                        .unwrap_or(0.0),
                }
            })
            .collect();
        shots.sort_by(|a, b| b.name.cmp(&a.name));
        shots
    }

    pub fn screenshot_bytes(&self, name: &str) -> Result<Vec<u8>> {
        let path = self.checked_child(&self.mc_dir.join("screenshots"), name)?;
        Ok(fs::read(path)?)
    }

    /// Join a user-supplied name under `dir`, rejecting absolute paths and any
    /// `..` component. `Path::starts_with` is component-wise and would happily
    /// accept `dir/../../etc`.
    fn checked_child(&self, dir: &Path, name: &str) -> Result<PathBuf> {
        let relative = Path::new(name);
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(anyhow!("Geçersiz dosya adı."));
        }
        Ok(dir.join(relative))
    }

    pub fn open_folder(&self, sub: &str) -> Result<()> {
        let relative = Path::new(sub);
        // `Path::starts_with` compares components, so `mc/../etc` would pass a
        // naive prefix check. Reject the escape at the source instead.
        if relative.is_absolute()
            || relative
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(anyhow!("Geçersiz klasör yolu."));
        }
        let target = if sub.is_empty() {
            self.mc_dir.clone()
        } else {
            self.mc_dir.join(relative)
        };
        let _ = fs::create_dir_all(&target);
        open_path(&target)?;
        Ok(())
    }

    // -- versions ----------------------------------------------------------

    pub fn versions(&self) -> Result<Vec<VersionEntry>> {
        let cache = self.data_dir.join("version_manifest.json");
        let fresh = fs::metadata(&cache)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .map(|age| age < Duration::from_secs(3600))
            .unwrap_or(false);

        let manifest: Manifest = if fresh {
            serde_json::from_str(&fs::read_to_string(&cache)?)?
        } else {
            match self.fetch_json::<Manifest>(MANIFEST_URL) {
                Ok(manifest) => {
                    let _ = fs::write(&cache, serde_json::to_vec(&manifest)?);
                    manifest
                }
                Err(error) => {
                    if let Ok(text) = fs::read_to_string(&cache) {
                        self.log(format!("Sürüm listesi güncellenemedi, önbellek kullanıldı: {error}"));
                        serde_json::from_str(&text)?
                    } else {
                        return Err(error);
                    }
                }
            }
        };

        let installed: Vec<String> = self
            .mc_dir
            .join("versions")
            .read_dir()
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().join(format!("{}.json", e.file_name().to_string_lossy())).exists())
                    .map(|e| e.file_name().to_string_lossy().to_string())
                    .collect()
            })
            .unwrap_or_default();

        Ok(manifest
            .versions
            .into_iter()
            .map(|version| VersionEntry {
                is_installed: installed.iter().any(|i| i == &version.id),
                id: version.id,
                kind: version.kind,
                release_time: version.release_time,
            })
            .collect())
    }

    // -- modrinth ----------------------------------------------------------

    pub fn search(
        &self,
        query: &str,
        kind: &str,
        sort: &str,
        limit: usize,
        game_version: &str,
        loader: &str,
    ) -> Result<Vec<SearchHit>> {
        if std::env::var("CL_DEBUG_SLOW").is_ok() {
            self.set_status(|s| {
                s.is_installing = true;
                s.percent = 42;
                s.status = "Aranıyor…".to_string();
                s.detail = "Modrinth".to_string();
            });
            std::thread::sleep(Duration::from_secs(6));
            self.set_status(|s| {
                s.is_installing = false;
                s.percent = 0;
                s.status = "Hazır".to_string();
                s.detail = String::new();
            });
        }
        // Facets mirror the Tauri build: project type, the profile's game
        // version and its loader, so a 1.7.10 profile never sees 1.21 mods.
        let mut facets = vec![format!("project_type:{kind}")];
        let version = game_version.trim();
        if !version.is_empty() && version != "Tümü" {
            facets.push(format!("versions:{version}"));
        }
        let loader = loader.trim().to_lowercase();
        if !loader.is_empty()
            && loader != "vanilla"
            && loader != "tümü"
            && matches!(kind, "mod" | "modpack")
        {
            facets.push(format!("categories:{loader}"));
        }
        let facet_json = format!(
            "[{}]",
            facets
                .iter()
                .map(|facet| format!("[\"{facet}\"]"))
                .collect::<Vec<_>>()
                .join(",")
        );
        let url = format!(
            "https://api.modrinth.com/v2/search?query={}&limit={}&index={}&facets={}",
            urlencode(query),
            limit,
            sort,
            urlencode(&facet_json)
        );
        #[derive(Deserialize)]
        struct Response {
            #[serde(default)]
            hits: Vec<Hit>,
        }
        #[derive(Deserialize)]
        struct Hit {
            slug: String,
            #[serde(default)]
            title: String,
            #[serde(default)]
            description: String,
            #[serde(default)]
            author: String,
            #[serde(default)]
            downloads: i64,
            #[serde(default)]
            follows: i64,
            #[serde(default)]
            categories: Vec<String>,
            #[serde(default)]
            project_type: String,
            #[serde(default)]
            icon_url: Option<String>,
        }

        let response: Response = self.fetch_json(&url)?;
        let limit = std::env::var("CL_DEBUG_LIMIT")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(usize::MAX);
        Ok(response
            .hits
            .into_iter()
            .take(limit)
            .map(|hit| SearchHit {
                icon_url: hit.icon_url.unwrap_or_default(),
                slug: hit.slug,
                title: hit.title,
                description: hit.description,
                author: hit.author,
                downloads: hit.downloads,
                follows: hit.follows,
                categories: hit.categories,
                project_type: hit.project_type,
            })
            .collect())
    }

    /// Download the newest matching file for `slug` into the instance's
    /// `mods/`/`shaderpacks/`/`resourcepacks/` folder and resolve one level of
    /// required dependencies.
    pub fn install(&self, slug: &str, instance_id: &str, kind: &str, with_deps: bool) -> Result<Vec<String>> {
        if kind == "modpack" {
            return Err(anyhow!(
                "Modpack kurulumu henüz desteklenmiyor; mod shader ve doku paketi kurabilirsin."
            ));
        }
        let instance = read_instance(&self.instance_dir(instance_id))
            .ok_or_else(|| anyhow!("Profil bulunamadı."))?;
        let target_dir = self.instance_dir(instance_id).join(match kind {
            "shader" => "shaderpacks",
            "resourcepack" => "resourcepacks",
            _ => "mods",
        });
        fs::create_dir_all(&target_dir)?;

        let mut installed = Vec::new();
        let mut queue = VecDeque::from([(slug.to_string(), 0usize)]);
        let mut seen = std::collections::HashSet::new();

        while let Some((slug, depth)) = queue.pop_front() {
            if !seen.insert(slug.clone()) {
                continue;
            }
            let versions = self.project_versions(&slug, &instance, kind)?;
            let Some(version) = versions.first() else {
                self.log(format!("{slug}: uygun sürüm yok"));
                continue;
            };
            for file in &version.files {
                if !file.primary {
                    continue;
                }
                let target = target_dir.join(&file.filename);
                if target.exists() {
                    continue;
                }
                self.log(format!("İndiriliyor: {}", file.filename));
                let bytes = self.fetch_bytes(&file.url)?;
                fs::write(&target, bytes)?;
                installed.push(file.filename.clone());
            }
            if with_deps && depth < 1 {
                for dep in &version.dependencies {
                    if let Some(project) = &dep.project_id {
                        if let Some(slug) = self.project_slug(project) {
                            queue.push_back((slug, depth + 1));
                        }
                    }
                }
            }
        }

        if installed.is_empty() {
            return Err(anyhow!("Kurulacak dosya bulunamadı."));
        }
        self.record_installed_slug(&instance.id, slug, &installed);
        Ok(installed)
    }

    /// Slug -> files map for one profile (`<instance>/modrinth.json`).
    pub fn installed_slugs(&self, instance_id: &str) -> std::collections::HashSet<String> {
        let path = self.instance_dir(instance_id).join("modrinth.json");
        fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<std::collections::HashMap<String, Vec<String>>>(&text).ok())
            .map(|map| map.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn record_installed_slug(&self, instance_id: &str, slug: &str, files: &[String]) {
        let path = self.instance_dir(instance_id).join("modrinth.json");
        let mut map: std::collections::HashMap<String, Vec<String>> = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        map.insert(slug.to_string(), files.to_vec());
        let _ = fs::write(&path, serde_json::to_vec_pretty(&map).unwrap_or_default());
    }

    fn project_slug(&self, project_id: &str) -> Option<String> {
        #[derive(Deserialize)]
        struct Project {
            slug: Option<String>,
        }
        let url = format!("https://api.modrinth.com/v2/project/{project_id}");
        self.fetch_json::<Project>(&url).ok()?.slug
    }

    fn project_versions(
        &self,
        slug: &str,
        instance: &Instance,
        kind: &str,
    ) -> Result<Vec<ProjectVersion>> {
        // Shaders and resource packs are not loader-specific: filtering them by
        // the profile's loader returns nothing.
        let loader = instance.loader.to_lowercase();
        let loaders = if loader == "vanilla" || loader.is_empty() || !matches!(kind, "mod" | "modpack")
        {
            String::new()
        } else {
            format!("&loaders=%5B%22{loader}%22%5D")
        };
        let url = format!(
            "https://api.modrinth.com/v2/project/{slug}/version?game_versions=%5B%22{}%22%5D{loaders}",
            instance.version
        );
        let versions: Vec<ProjectVersion> = self.fetch_json(&url)?;
        Ok(versions)
    }

    // -- http --------------------------------------------------------------

    pub(crate) fn fetch_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        let mut response = self.agent.get(url).call().map_err(|e| anyhow!("{url}: {e}"))?;
        let body = response.body_mut().read_to_string()?;
        serde_json::from_str(&body).map_err(|e| anyhow!("{url}: {e}"))
    }

    pub(crate) fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>> {
        let mut response = self.agent.get(url).call().map_err(|e| anyhow!("{url}: {e}"))?;
        // Client jars are ~30 MB; other downloads are small. Raise the per-body
        // ceiling instead of relying on ureq's 10 MB default.
        let body = response
            .body_mut()
            .with_config()
            .limit(512 * 1024 * 1024)
            .read_to_vec()?;
        Ok(body)
    }

    /// Download straight to a path (used for large jars).
    pub(crate) fn download_to(&self, url: &str, target: &Path) -> Result<u64> {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut response = self.agent.get(url).call().map_err(|e| anyhow!("{url}: {e}"))?;
        let mut file = fs::File::create(target)?;
        let copied = std::io::copy(
            &mut response
                .body_mut()
                .with_config()
                .limit(512 * 1024 * 1024)
                .reader(),
            &mut file,
        )?;
        Ok(copied)
    }

    // -- launch ------------------------------------------------------------

    pub fn launch(self: &Arc<Self>, request: LaunchRequest) -> Result<()> {
        let status = self.status();
        if status.is_installing || status.is_running {
            return Err(anyhow!("Oyun zaten çalışıyor veya hazırlanıyor."));
        }
        let instance = match request.instance_id.clone() {
            Some(id) => read_instance(&self.instance_dir(&id))
                .ok_or_else(|| anyhow!("Profil bulunamadı."))?,
            None => return Err(anyhow!("Profil seçilmedi.")),
        };

        self.log(format!(
            "Başlatma isteği: {} (MC {}, {})",
            instance.name, instance.version, instance.loader
        ));
        self.set_status(|s| {
            s.is_installing = true;
            s.is_running = false;
            s.percent = 2;
            s.status = "Başlatma isteği alındı…".to_string();
            s.detail = format!("Minecraft {}", instance.version);
            s.error = None;
            s.launch_instance_id = Some(instance.id.clone());
            s.minecraft_pid = None;
        });

        let version = match self.effective_version(&instance) {
            Ok(version) => version,
            Err(error) => {
                self.set_status(|s| {
                    s.status = "Başlatma Hatası!".to_string();
                    s.detail = error.to_string();
                });
                self.finish_launch(Some(error.to_string()));
                self.log(format!("HATA: {error}"));
                return Err(error);
            }
        };
        self.log(format!("Etkin sürüm: {version}"));

        let core = self.clone();
        let username = request.username.clone();
        let ram = request.ram.max(2) as u64;
        let quick_play = request.quick_play.clone();
        let optimized = request.cookie_optimized;
        let aikar = request.jvm_preset.eq_ignore_ascii_case("aikar");
        std::thread::spawn(move || {
            let options = crate::launch::LaunchOptions {
                version: &version,
                instance_id: &instance.id,
                username: &username,
                ram_gb: ram,
                quick_play: quick_play.clone(),
                cookie_optimized: optimized,
                aikar: aikar,
            };
            if let Err(error) = core.launch_now(options) {
                core.set_status(|s| {
                    s.status = "Başlatma Hatası!".to_string();
                    s.detail = error.to_string();
                });
                core.finish_launch(Some(error.to_string()));
                core.log(format!("HATA: {error}"));
            }
        });
        Ok(())
    }

    /// The version id that actually launches: the vanilla id, or a
    /// `fabric-loader-<loader>-<mc>` profile resolved from Fabric meta.
    pub fn effective_version(&self, instance: &Instance) -> Result<String> {
        match instance.loader.to_lowercase().as_str() {
            "fabric" | "quilt" => {
                let loader_version = if instance.loader_version.is_empty() {
                    self.recommended_loader_version(&instance.loader, &instance.version)?
                } else {
                    instance.loader_version.clone()
                };
                Ok(format!(
                    "{}-loader-{}-{}",
                    instance.loader.to_lowercase(),
                    loader_version,
                    instance.version
                ))
            }
            "forge" | "neoforge" => self.ensure_forge_family(instance),
            _ => Ok(instance.version.clone()),
        }
    }

    /// Forge/NeoForge: run the official installer once, then launch the profile
    /// it writes (`<mc>-forge-<ver>` / `neoforge-<ver>`).
    pub fn ensure_forge_family(&self, instance: &Instance) -> Result<String> {
        let loader = instance.loader.to_lowercase();
        let mc = instance.version.clone();
        let loader_version = if instance.loader_version.is_empty() {
            self.recommended_forge_version(&loader, &mc)?
        } else {
            instance.loader_version.clone()
        };

        let id = if loader == "neoforge" {
            format!("neoforge-{loader_version}")
        } else {
            format!("{mc}-forge-{loader_version}")
        };
        let json_path = self
            .mc_dir
            .join("versions")
            .join(&id)
            .join(format!("{id}.json"));
        if json_path.exists() {
            return Ok(id);
        }

        let url = if loader == "neoforge" {
            format!(
                "https://maven.neoforged.net/releases/net/neoforged/neoforge/{v}/neoforge-{v}-installer.jar",
                v = loader_version
            )
        } else {
            format!(
                "https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{v}/forge-{mc}-{v}-installer.jar",
                v = loader_version
            )
        };
        let dir = self.data_dir.join("installers");
        let _ = fs::create_dir_all(&dir);
        let installer = dir.join(format!("{id}-installer.jar"));
        self.set_status(|status| {
            status.is_installing = true;
            status.percent = 5;
            status.status = format!("{} kuruluyor…", loader.to_uppercase());
            status.detail = loader_version.clone();
        });
        self.log(format!("{loader} yükleyicisi indiriliyor ({loader_version})…"));
        self.download_to(&url, &installer)?;

        self.log("Yükleyici çalıştırılıyor (java -jar … --installClient)…".to_string());
        let output = Command::new("java")
            .arg("-jar")
            .arg(&installer)
            .arg("--installClient")
            .arg(&self.mc_dir)
            .output()
            .map_err(|e| anyhow!("Yükleyici çalıştırılamadı: {e}"))?;
        if !output.status.success() {
            let tail = String::from_utf8_lossy(&output.stdout);
            let tail = tail.lines().rev().take(3).collect::<Vec<_>>().join(" | ");
            return Err(anyhow!("{loader} kurulumu başarısız: {tail}"));
        }
        if !json_path.exists() {
            return Err(anyhow!(
                "{loader} kurulumu bitti ama {} bulunamadı.",
                json_path.display()
            ));
        }
        self.set_status(|status| {
            status.is_installing = false;
            status.percent = 0;
            status.status = "Hazır".to_string();
            status.detail = String::new();
        });
        self.log(format!("{loader} hazır: {id}"));
        Ok(id)
    }

    fn recommended_forge_version(&self, loader: &str, mc: &str) -> Result<String> {
        if loader == "neoforge" {
            // NeoForge versions track the game: 1.21.4 -> 21.4.x
            let prefix = mc.strip_prefix("1.").unwrap_or(mc).to_string();
            let prefix = format!("{prefix}.");
            #[derive(Deserialize)]
            struct Versions {
                #[serde(default)]
                versions: Vec<String>,
            }
            let data: Versions = self.fetch_json(
                "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge",
            )?;
            let mut matching: Vec<String> = data
                .versions
                .into_iter()
                .filter(|version| version.starts_with(&prefix) && !version.contains("-beta"))
                .collect();
            matching.sort();
            return matching
                .last()
                .cloned()
                .ok_or_else(|| anyhow!("NeoForge {mc} sürümü bulunamadı"));
        }

        #[derive(Deserialize)]
        struct Promotions {
            #[serde(default)]
            promos: std::collections::HashMap<String, String>,
        }
        let data: Promotions =
            self.fetch_json("https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json")?;
        data.promos
            .get(&format!("{mc}-recommended"))
            .or_else(|| data.promos.get(&format!("{mc}-latest")))
            .cloned()
            .ok_or_else(|| anyhow!("Forge {mc} sürümü bulunamadı"))
    }

    fn recommended_loader_version(&self, loader: &str, mc: &str) -> Result<String> {
        let base = if loader.eq_ignore_ascii_case("quilt") {
            "https://meta.quiltmc.org"
        } else {
            "https://meta.fabricmc.net"
        };
        let url = format!("{base}/v2/versions/loader/{mc}");
        // Fabric/Quilt nest the loader under `loader`:
        // [{ "loader": { "version": "0.20.1", "stable": true }, ... }]
        let entries: Vec<serde_json::Value> = self.fetch_json(&url)?;
        entries
            .iter()
            .find(|entry| {
                entry["loader"]["stable"]
                    .as_bool()
                    .unwrap_or(false)
            })
            .or_else(|| entries.first())
            .and_then(|entry| entry["loader"]["version"].as_str())
            .map(|value| value.to_string())
            .ok_or_else(|| anyhow!("{} yükleyici sürümü bulunamadı", loader))
    }

    /// Clear the transient launch state after the game exits or a launch
    /// fails. Without this `launch_instance_id` sticks around and the profile
    /// can never be deleted again.
    pub(crate) fn finish_launch(&self, error: Option<String>) {
        self.clear_launch_pid();
        self.set_status(|status| {
            status.is_installing = false;
            status.is_running = false;
            status.percent = 0;
            status.minecraft_pid = None;
            status.launch_instance_id = None;
            status.current_file.clear();
            match &error {
                Some(message) => {
                    status.status = "Oyun beklenmedik şekilde kapandı.".to_string();
                    status.error = Some(message.clone());
                }
                None => {
                    status.status = "Oyun kapandı.".to_string();
                    status.detail.clear();
                }
            }
        });
    }

    pub fn stop(&self) {
        if let Ok(mut pid) = self.launch_pid.lock() {
            if let Some(pid) = pid.take() {
                #[cfg(unix)]
                let _ = Command::new("kill").arg(pid.to_string()).status();
                #[cfg(windows)]
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .status();
                self.set_status(|s| {
                    s.is_running = false;
                    s.is_installing = false;
                    s.minecraft_pid = None;
                    s.launch_instance_id = None;
                    s.status = "Oyun kapatıldı.".to_string();
                });
            }
        }
    }

    pub(crate) fn set_launch_pid(&self, pid: u32) {
        if let Ok(mut slot) = self.launch_pid.lock() {
            *slot = Some(pid);
        }
    }

    pub(crate) fn clear_launch_pid(&self) {
        if let Ok(mut slot) = self.launch_pid.lock() {
            *slot = None;
        }
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.stop();
    }
}

// ---------------------------------------------------------------------------
// Models used by the core and the UI
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ContentEntry {
    pub name: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Default)]
pub struct VersionEntry {
    pub id: String,
    pub kind: String,
    pub release_time: String,
    pub is_installed: bool,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct Manifest {
    #[serde(default)]
    versions: Vec<ManifestVersion>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ManifestVersion {
    id: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(rename = "releaseTime", default)]
    release_time: String,
}

#[derive(Debug, Deserialize)]
struct ProjectVersion {
    #[serde(default)]
    files: Vec<ProjectFile>,
    #[serde(default)]
    dependencies: Vec<ProjectDependency>,
}

#[derive(Debug, Deserialize)]
struct ProjectFile {
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Deserialize)]
struct ProjectDependency {
    #[serde(default)]
    project_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn minecraft_dir() -> PathBuf {
    if let Ok(explicit) = std::env::var("COOKIE_MINECRAFT_DIR") {
        return PathBuf::from(explicit);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".minecraft")
}

fn data_dir() -> PathBuf {
    if let Ok(explicit) = std::env::var("COOKIE_LAUNCHER_DATA_DIR") {
        return PathBuf::from(explicit);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".cookie_launcher")
}

fn read_instance(dir: &Path) -> Option<Instance> {
    let text = fs::read_to_string(dir.join("instance.json")).ok()?;
    let mut instance: Instance = serde_json::from_str(&text).ok()?;
    instance.id = dir.file_name()?.to_string_lossy().to_string();
    let mods = dir.join("mods");
    instance.mod_count = fs::read_dir(mods)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "jar").unwrap_or(false))
                .count() as i64
        })
        .unwrap_or(0);
    Some(instance)
}

fn write_instance(dir: &Path, instance: &Instance) -> Result<()> {
    fs::create_dir_all(dir)?;
    #[derive(Serialize)]
    struct Stored<'a> {
        name: &'a str,
        version: &'a str,
        loader: &'a str,
        loader_version: &'a str,
        icon: &'a str,
        created_at: &'a str,
    }
    let stored = Stored {
        name: &instance.name,
        version: &instance.version,
        loader: &instance.loader,
        loader_version: &instance.loader_version,
        icon: &instance.icon,
        created_at: &instance.created_at,
    };
    fs::write(
        dir.join("instance.json"),
        serde_json::to_vec_pretty(&stored)?,
    )?;
    Ok(())
}

pub fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.to_lowercase().chars() {
        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('_');
        }
    }
    if out.is_empty() {
        out = format!("profil_{}", now_epoch());
    }
    out.truncate(60);
    out
}

fn dir_size(dir: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            total += dir_size(&path);
        } else if let Ok(meta) = path.metadata() {
            total += meta.len();
        }
    }
    total
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn clock() -> String {
    let secs = now_epoch() % 86_400;
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs % 3600) / 60, secs % 60)
}

fn now_string() -> String {
    // `%Y-%m-%d %H:%M:%S` without pulling a date crate: whole days since epoch.
    let secs = now_epoch();
    let days = secs / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let rem = secs % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn format_time(time: SystemTime) -> Option<String> {
    let secs = time.duration_since(UNIX_EPOCH).ok()?.as_secs();
    let days = secs / 86_400;
    let (year, month, day) = civil_from_days(days as i64);
    let rem = secs % 86_400;
    Some(format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60
    ))
}

/// Howard Hinnant's `civil_from_days`.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn urlencode(value: &str) -> String {
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

fn open_path(path: &Path) -> Result<()> {
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = Command::new("explorer");

    command
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Klasör açılamadı")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Social: remote friends over the bore.pub relay (see `crate::social`)
// ---------------------------------------------------------------------------

use crate::api::{SocialProfile, SocialStateResponse};
use crate::social::{ShareConfig, ShareSession};
use crate::launch::extract_zip;

impl Core {
    fn identity(&self) -> &Identity {
        self.identity.get_or_init(|| Identity::load(&self.data_dir))
    }

    fn friends_path(&self) -> PathBuf {
        self.data_dir.join("friends.json")
    }

    fn load_friends(&self) -> Vec<SocialFriend> {
        fs::read_to_string(self.friends_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save_friends(&self, friends: &[SocialFriend]) -> Result<()> {
        fs::write(self.friends_path(), serde_json::to_vec_pretty(friends)?)?;
        Ok(())
    }

    pub fn social_state(&self) -> Result<SocialStateResponse> {
        let identity = self.identity().clone();
        let friends = self.load_friends();

        let share = self.share.lock().ok().and_then(|share| {
            share.as_ref().map(|session| {
                (
                    session.code.clone(),
                    session.address(),
                    session.invite.expires,
                )
            })
        });

        Ok(SocialStateResponse {
            success: true,
            connected: share.is_some(),
            error: String::new(),
            profile: Some(SocialProfile {
                player_id: Some(identity.player_id.clone()),
                name: Some(identity.name.clone()),
                code: identity.code.clone(),
            }),
            friends,
            invite: share.as_ref().map(|(code, _, _)| crate::api::SocialInvite {
                code: code.clone(),
                deep_link: format!("cl://join/{code}"),
                web_link: format!("https://cookie-launcher.github.io/join/?c={code}"),
            }),
            host_relay: share.as_ref().map(|(_, address, _)| crate::api::HostRelayStatus {
                running: true,
                error: address.clone(),
            }),
            ..Default::default()
        })
    }

    /// Expose `local_port` through bore.pub and publish a CL1 invite code.
    pub fn share_world(&self, local_port: u16, world: &str) -> Result<String> {
        let mut guard = self.share.lock().map_err(|_| anyhow!("kilit hatası"))?;
        if let Some(session) = guard.as_ref() {
            return Ok(session.code.clone());
        }
        let identity = self.identity().clone();
        self.log(format!("bore.pub tüneli açılıyor (yerel port {local_port})…"));
        let session = ShareSession::start(ShareConfig {
            server: std::env::var("COOKIE_BORE_SERVER").unwrap_or_else(|_| "bore.pub".to_string()),
            host_name: identity.name,
            host_code: identity.code,
            world: world.to_string(),
            local_port,
        })?;
        let code = session.code.clone();
        let address = session.address();
        self.log(format!("Dünya paylaşıldı (bore.pub): {address}"));
        self.log(format!("Davet kodu: {code}"));
        *guard = Some(session);
        Ok(code)
    }

    pub fn stop_sharing(&self) {
        if let Ok(mut guard) = self.share.lock() {
            if let Some(session) = guard.take() {
                session.tunnel.close();
                self.log("Paylaşım kapatıldı.");
            }
        }
    }

    pub fn add_friend(&self, code: &str) -> Result<()> {
        let code = code.trim().to_uppercase();
        if code.is_empty() {
            return Err(anyhow!("Arkadaş kodu boş olamaz."));
        }
        if code == self.identity().code.to_uppercase() {
            return Err(anyhow!("Kendi kodunu ekleyemezsin."));
        }
        let mut friends = self.load_friends();
        if friends.iter().any(|f| f.code.eq_ignore_ascii_case(&code)) {
            return Err(anyhow!("Bu arkadaş zaten ekli."));
        }
        friends.push(SocialFriend {
            player_id: format!("code:{code}"),
            name: code.clone(),
            code: code.clone(),
            ..Default::default()
        });
        self.save_friends(&friends)?;
        self.log(format!("Arkadaş eklendi: {code}"));
        Ok(())
    }

    pub fn remove_friend(&self, code: &str) -> Result<()> {
        let mut friends = self.load_friends();
        friends.retain(|f| !f.code.eq_ignore_ascii_case(code) && f.player_id != code);
        self.save_friends(&friends)?;
        Ok(())
    }

    /// Parse a CL1 invite, start the authenticated local proxy and return the
    /// loopback address Minecraft should join (`--quickPlayMultiplayer`).
    pub fn join_invite(&self, text: &str) -> Result<String> {
        let invite = crate::social::parse(text)?;
        let relay = invite
            .relay
            .clone()
            .ok_or_else(|| anyhow!("Davet kodunda relay adresi yok."))?;
        if invite.ticket.is_empty() || invite.token.is_empty() {
            return Err(anyhow!("Davet kodunda tünel anahtarı yok."));
        }
        let ticket = crate::relay::Ticket::from_hex(&invite.ticket, &invite.token)?;
        let proxy = crate::social::LocalProxy::start(relay.host.clone(), relay.port, ticket)?;
        let address = format!("127.0.0.1:{}", proxy.local_port);
        self.log(format!(
            "Davet kabul edildi: {} · {} ({})",
            invite.host_name,
            invite.world,
            format!("{}:{}", relay.host, relay.port)
        ));
        self.set_status(|s| {
            s.detail = format!("Arkadaş dünyası: {address}");
        });
        if let Ok(mut slot) = self.joined.lock() {
            *slot = Some(address.clone());
        }
        Ok(address)
    }

    pub fn joined_address(&self) -> Option<String> {
        self.joined.lock().ok().and_then(|slot| slot.clone())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Identity {
    player_id: String,
    name: String,
    code: String,
}

impl Identity {
    fn load(dir: &Path) -> Identity {
        let path = dir.join("identity.json");
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(identity) = serde_json::from_str::<Identity>(&text) {
                if !identity.player_id.is_empty() {
                    return identity;
                }
            }
        }
        let username = std::env::var("USER").unwrap_or_else(|_| "Oyuncu".to_string());
        let identity = Identity {
            player_id: random_hex(16),
            name: username,
            code: random_code(),
        };
        let _ = fs::write(&path, serde_json::to_vec_pretty(&identity).unwrap_or_default());
        identity
    }
}

impl Core {
    /// Raw bytes for a remote image (Modrinth icons).
    pub fn fetch_icon(&self, url: &str) -> Result<Vec<u8>> {
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(anyhow!("Geçersiz görsel adresi."));
        }
        self.fetch_bytes(url)
    }

    /// Every project gets a picture: the search icon, else the project's own
    /// icon, else its first gallery image, else the project avatar endpoint.
    pub fn project_image(&self, slug: &str, icon_url: &str) -> Option<Vec<u8>> {
        if !icon_url.is_empty() {
            if let Ok(bytes) = self.fetch_icon(icon_url) {
                if !bytes.is_empty() {
                    return Some(bytes);
                }
            }
        }

        #[derive(Deserialize)]
        struct Gallery {
            #[serde(default)]
            url: String,
            #[serde(default)]
            featured: bool,
        }
        #[derive(Deserialize)]
        struct Project {
            #[serde(default)]
            icon_url: Option<String>,
            #[serde(default)]
            gallery: Vec<Gallery>,
        }

        let url = format!("https://api.modrinth.com/v2/project/{slug}");
        if let Ok(project) = self.fetch_json::<Project>(&url) {
            if let Some(icon) = project.icon_url {
                if let Ok(bytes) = self.fetch_icon(&icon) {
                    if !bytes.is_empty() {
                        return Some(bytes);
                    }
                }
            }
            let mut gallery = project.gallery;
            gallery.sort_by_key(|g| !g.featured);
            for entry in gallery {
                if entry.url.is_empty() {
                    continue;
                }
                if let Ok(bytes) = self.fetch_icon(&entry.url) {
                    if !bytes.is_empty() {
                        return Some(bytes);
                    }
                }
            }
        }

        // Last resort: Modrinth's generated project avatar.
        if let Ok(bytes) = self.fetch_icon(&format!("https://api.modrinth.com/v2/project/{slug}/icon")) {
            if !bytes.is_empty() {
                return Some(bytes);
            }
        }
        None
    }
}

fn random_hex(len: usize) -> String {
    let mut out = String::with_capacity(len);
    let mut seed = now_epoch() as u64 ^ ((std::process::id() as u64) << 32);
    while out.len() < len {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        out.push_str(&format!("{:016x}", seed));
    }
    out.truncate(len);
    out
}

fn random_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut seed = now_epoch() ^ ((std::process::id() as u64) << 17);
    let mut out = String::with_capacity(6);
    for _ in 0..6 {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
        out.push(ALPHABET[(seed >> 33) as usize % ALPHABET.len()] as char);
    }
    out
}

// ---------------------------------------------------------------------------
// Modpacks: `.mrpack` (Modrinth format) -> a ready profile
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MrpackIndex {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version_id: String,
    #[serde(default)]
    files: Vec<MrpackFile>,
    #[serde(default)]
    dependencies: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct MrpackFile {
    #[serde(default)]
    path: String,
    #[serde(default)]
    downloads: Vec<String>,
}

impl Core {
    /// Download a Modrinth modpack, unpack its index and turn it into a profile.
    pub fn install_modpack(&self, slug: &str) -> Result<String> {
        let versions: Vec<serde_json::Value> = self.fetch_json(&format!(
            "https://api.modrinth.com/v2/project/{slug}/version"
        ))?;
        let version = versions.first().ok_or_else(|| anyhow!("Paket sürümü yok."))?;
        let file = version["files"]
            .as_array()
            .and_then(|files| {
                files
                    .iter()
                    .find(|file| file["primary"].as_bool().unwrap_or(false))
                    .or_else(|| files.first())
            })
            .ok_or_else(|| anyhow!("Paket dosyası bulunamadı."))?;
        let url = file["url"].as_str().unwrap_or_default();
        let filename = file["filename"].as_str().unwrap_or("pack.mrpack");
        if url.is_empty() {
            return Err(anyhow!("Paket indirme adresi yok."));
        }

        self.log(format!("Modpack indiriliyor: {filename}"));
        let work = self.data_dir.join("modpacks").join(slug);
        let _ = fs::remove_dir_all(&work);
        fs::create_dir_all(&work)?;
        let archive = work.join(filename);
        self.download_to(url, &archive)?;

        let unpacked = work.join("unpacked");
        extract_zip(&archive, &unpacked)?;

        let index_path = unpacked.join("modrinth.index.json");
        let index: MrpackIndex = serde_json::from_str(&fs::read_to_string(&index_path)?)?;

        let mc = index
            .dependencies
            .get("minecraft")
            .cloned()
            .ok_or_else(|| anyhow!("Paket Minecraft sürümü belirtmiyor."))?;
        let (loader, loader_version) = if let Some(v) = index.dependencies.get("fabric-loader") {
            ("fabric".to_string(), v.clone())
        } else if let Some(v) = index.dependencies.get("quilt-loader") {
            ("quilt".to_string(), v.clone())
        } else if let Some(v) = index.dependencies.get("forge") {
            ("forge".to_string(), v.clone())
        } else if let Some(v) = index.dependencies.get("neoforge") {
            ("neoforge".to_string(), v.clone())
        } else {
            ("vanilla".to_string(), String::new())
        };
        let name = if index.name.is_empty() {
            slug.to_string()
        } else {
            index.name.clone()
        };
        let instance = self.create_instance(&name, &mc, &loader, "")?;
        let dir = self.instance_dir(&instance.id);

        // Persist the loader version the pack pinned.
        if !loader_version.is_empty() {
            let mut stored = instance.clone();
            stored.loader_version = loader_version.clone();
            let _ = write_instance(&dir, &stored);
        }

        self.log(format!(
            "Modpack profili: {} (MC {mc}, {loader} {loader_version})",
            instance.name
        ));

        let total = index.files.len().max(1);
        let mut installed = 0usize;
        for (position, file) in index.files.iter().enumerate() {
            if file.path.is_empty() || file.path.contains("..") {
                continue;
            }
            let Some(url) = file.downloads.first() else {
                continue;
            };
            let target = dir.join(&file.path);
            if target.exists() {
                continue;
            }
            if let Some(parent) = target.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match self.download_to(url, &target) {
                Ok(_) => installed += 1,
                Err(error) => self.log(format!("{} indirilemedi: {error}", file.path)),
            }
            if position % 5 == 0 {
                self.set_status(|s| {
                    s.percent = 10 + (position as i64 * 80 / total as i64);
                    s.status = "Modpack kuruluyor…".to_string();
                    s.detail = format!("{position}/{total}");
                });
            }
        }

        for overrides in ["overrides", "client-overrides"] {
            copy_dir(&unpacked.join(overrides), &dir);
        }

        self.record_installed_slug(&instance.id, slug, &[index.name.clone()]);
        let mods = self.content(&instance.id, "mod").len();
        self.log(format!(
            "Modpack hazır: {} · {installed} dosya · {mods} mod",
            instance.name
        ));
        self.set_status(|s| {
            s.is_installing = false;
            s.percent = 0;
            s.status = "Modpack kuruldu.".to_string();
            s.detail = instance.name.clone();
        });
        let _ = fs::remove_dir_all(&work);
        Ok(instance.id)
    }
}

fn copy_dir(from: &Path, to: &Path) {
    let Ok(entries) = fs::read_dir(from) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            let _ = fs::create_dir_all(&target);
            copy_dir(&path, &target);
        } else {
            if let Some(parent) = target.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::copy(&path, &target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn temp_core(tag: &str) -> Arc<Core> {
        let base = std::env::temp_dir().join(format!(
            "cl-test-{tag}-{}-{}",
            std::process::id(),
            now_epoch()
        ));
        let _ = fs::remove_dir_all(&base);
        Core::with_paths(base.join("mc"), base.join("data"))
    }

    #[test]
    fn creates_lists_and_deletes_a_profile() {
        let core = temp_core("instances");
        assert!(core.instances().is_empty());

        let instance = core
            .create_instance("My Pack", "1.21.4", "fabric", "")
            .expect("create");
        assert_eq!(instance.id, "my_pack");
        assert_eq!(instance.name, "My Pack");
        assert_eq!(instance.loader, "fabric");
        assert!(core.instance_dir("my_pack").join("mods").is_dir());
        assert_eq!(core.instances().len(), 1);

        // Duplicate names are rejected rather than silently overwritten.
        assert!(core.create_instance("My Pack", "1.21.4", "fabric", "").is_err());

        core.delete_instance("my_pack").expect("delete");
        assert!(core.instances().is_empty());
        assert!(!core.instance_dir("my_pack").exists());
    }

    #[test]
    fn slugifies_names_and_avoids_collisions() {
        let core = temp_core("slug");
        let first = core.create_instance("Çok Güzel Profil!", "1.21.4", "vanilla", "").unwrap();
        let second = core.create_instance("Çok Güzel Profil!", "1.21.4", "vanilla", "");
        assert!(!first.id.contains(' ') && !first.id.contains('!'));
        assert!(first.id.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-'));
        assert!(second.is_err(), "aynı isim reddedilmeli");
        let third = core.create_instance("Bambaşka", "1.21.4", "vanilla", "").unwrap();
        assert_ne!(first.id, third.id);
    }

    #[test]
    fn finishing_a_launch_unblocks_profile_deletion() {
        let core = temp_core("finish");
        let instance = core.create_instance("Kilitli Profil", "1.21.4", "vanilla", "").unwrap();
        core.set_status(|status| {
            status.is_running = true;
            status.minecraft_pid = Some(1234);
            status.launch_instance_id = Some(instance.id.clone());
        });

        // While the game is considered running the profile is protected.
        assert!(core.delete_instance(&instance.id).is_err());

        // The exit path must clear every transient field…
        core.finish_launch(None);
        let status = core.status();
        assert!(!status.is_running);
        assert!(status.minecraft_pid.is_none());
        assert!(status.launch_instance_id.is_none());

        // …and deletion works again.
        assert!(core.delete_instance(&instance.id).is_ok());
        assert!(core.instances().is_empty());
    }

    #[test]
    fn failed_launch_also_unblocks_deletion() {
        let core = temp_core("finish-err");
        let instance = core.create_instance("Hatalı Profil", "1.21.4", "vanilla", "").unwrap();
        core.set_status(|status| {
            status.is_installing = true;
            status.launch_instance_id = Some(instance.id.clone());
        });
        core.finish_launch(Some("patlama".to_string()));
        let status = core.status();
        assert!(!status.is_installing);
        assert!(status.launch_instance_id.is_none());
        assert_eq!(status.error.as_deref(), Some("patlama"));
        assert!(core.delete_instance(&instance.id).is_ok());
    }

    #[test]
    fn adds_and_removes_friends_persistently() {
        let core = temp_core("friends");
        assert!(core.add_friend("abc123").is_ok(), "küçük harf kabul edilmeli");

        let friends = core.social_state().unwrap().friends;
        assert_eq!(friends.len(), 1);
        assert_eq!(friends[0].code, "ABC123");

        // Persisted next to the data dir, not just in memory.
        let stored = fs::read_to_string(core.data_dir.join("friends.json")).unwrap();
        assert!(stored.contains("ABC123"));

        // Duplicates and your own code are refused.
        assert!(core.add_friend("ABC123").is_err());
        let own = core.identity().code.clone();
        assert!(core.add_friend(&own).is_err());
        assert!(core.add_friend("   ").is_err());

        core.remove_friend("ABC123").unwrap();
        assert!(core.social_state().unwrap().friends.is_empty());
    }

    #[test]
    fn rejects_bad_minecraft_versions() {
        let core = temp_core("versions-guard");
        for bad in ["", "  ", "1.21.4 beta", "../evil", "a/b", &"9".repeat(41)] {
            assert!(
                core.create_instance("Profil", bad, "vanilla", "").is_err(),
                "“{bad}” reddedilmeli"
            );
        }
        assert!(core.create_instance("Profil", "1.21.4", "vanilla", "").is_ok());
    }

    #[test]
    fn accepts_every_supported_loader() {
        let core = temp_core("loaders");
        for (index, loader) in ["vanilla", "fabric", "quilt", "forge", "neoforge"]
            .into_iter()
            .enumerate()
        {
            let created = core.create_instance(
                &format!("Profil {index}"),
                "1.20.1",
                loader,
                "",
            );
            assert!(created.is_ok(), "{loader} profili oluşturulabilmeli");
        }
    }

    #[test]
    fn lists_toggles_and_deletes_installed_content() {
        let core = temp_core("content");
        core.create_instance("Modlu", "1.21.4", "fabric", "").unwrap();
        let mods = core.instance_dir("modlu").join("mods");
        fs::write(mods.join("sodium.jar"), b"jar").unwrap();
        fs::write(mods.join("lithium.jar"), b"jar").unwrap();

        let entries = core.content("modlu", "mod");
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|entry| entry.enabled));
        assert_eq!(core.content_counts("modlu"), [2, 0, 0]);

        // Disabling renames to `.disabled` and keeps the entry listed.
        core.toggle_content("modlu", "mod", "sodium.jar").unwrap();
        let entries = core.content("modlu", "mod");
        assert!(entries
            .iter()
            .any(|entry| entry.name == "sodium.jar.disabled" && !entry.enabled));
        // Re-enabling restores the original name.
        core.toggle_content("modlu", "mod", "sodium.jar.disabled").unwrap();
        assert!(mods.join("sodium.jar").exists());

        core.delete_content("modlu", "mod", "lithium.jar").unwrap();
        assert_eq!(core.content("modlu", "mod").len(), 1);
    }

    #[test]
    fn rejects_path_traversal_in_file_names() {
        let core = temp_core("traversal");
        let root = core.mc_dir.clone();
        fs::create_dir_all(root.join("screenshots")).unwrap();
        fs::write(root.join("secret.txt"), b"top secret").unwrap();

        // Before the guard this resolved to `<mc>/secret.txt`.
        assert!(core.screenshot_bytes("../secret.txt").is_err());
        assert!(core.screenshot_bytes("/etc/passwd").is_err());

        fs::write(root.join("screenshots/shot.png"), b"png").unwrap();
        assert_eq!(core.screenshot_bytes("shot.png").unwrap(), b"png");

        core.create_instance("Dosya Profili", "1.21.4", "vanilla", "").unwrap();
        assert!(core.toggle_content("dosya_profili", "mod", "../../evil.jar").is_err());
        assert!(core.delete_content("dosya_profili", "mod", "../../evil.jar").is_err());
    }

    #[test]
    fn reads_worlds_and_screenshots_from_disk() {
        let core = temp_core("worlds");
        core.create_instance("Dünya Profili", "1.21.4", "vanilla", "").unwrap();
        let saves = core.instance_dir("dunya_profili").join("saves");
        let world = saves.join("Kayıp Şehir");
        fs::create_dir_all(&world).unwrap();
        fs::write(world.join("level.dat"), vec![0u8; 1_600_000]).unwrap();

        let worlds = core.worlds("dunya_profili");
        assert_eq!(worlds.len(), 1);
        assert_eq!(worlds[0].name, "Kayıp Şehir");
        assert!(worlds[0].size_mb > 0.0);
        assert!(!worlds[0].last_played.is_empty());

        let shots_dir = core.mc_dir.join("screenshots");
        fs::create_dir_all(&shots_dir).unwrap();
        fs::write(shots_dir.join("2026-01-01_10.00.00.png"), vec![0u8; 512]).unwrap();
        fs::write(shots_dir.join("not-a-screenshot.txt"), b"x").unwrap();
        let shots = core.screenshots();
        assert_eq!(shots.len(), 1, "sadece png listelenmeli");
        assert_eq!(shots[0].size, 512);
        assert_eq!(core.screenshot_bytes("2026-01-01_10.00.00.png").unwrap().len(), 512);
    }

    #[test]
    fn effective_version_resolves_fabric_and_rejects_forge() {
        let core = temp_core("versions");
        let mut instance = Instance {
            id: "x".into(),
            version: "1.21.4".into(),
            loader: "fabric".into(),
            loader_version: "0.16.9".into(),
            ..Default::default()
        };
        assert_eq!(
            core.effective_version(&instance).unwrap(),
            "fabric-loader-0.16.9-1.21.4"
        );
        instance.loader = "vanilla".into();
        assert_eq!(core.effective_version(&instance).unwrap(), "1.21.4");
        instance.loader = "forge".into();
        assert!(core.effective_version(&instance).is_err());
    }

    #[test]
    fn reads_installed_modrinth_slugs() {
        let core = temp_core("slugs");
        core.create_instance("Slug Profili", "1.21.4", "fabric", "").unwrap();
        assert!(core.installed_slugs("slug_profili").is_empty());
        fs::write(
            core.instance_dir("slug_profili").join("modrinth.json"),
            br#"{"sodium":["sodium.jar"],"fabric-api":["fabric-api.jar"]}"#,
        )
        .unwrap();
        let slugs = core.installed_slugs("slug_profili");
        assert_eq!(slugs.len(), 2);
        assert!(slugs.contains("sodium"));
        assert!(slugs.contains("fabric-api"));
    }

    #[test]
    fn open_folder_rejects_escaping_paths() {
        let core = temp_core("folders");
        assert!(core.open_folder("../etc").is_err());
        assert!(core.open_folder("/etc").is_err());
    }
}

// ---------------------------------------------------------------------------
// LAN worlds: the Minecraft broadcast on UDP 4445 plus offline-mode setup
// ---------------------------------------------------------------------------

use std::net::UdpSocket;

const LAN_PORT: u16 = 4445;
const LAN_TTL: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LanWorld {
    pub motd: String,
    pub port: u16,
    pub address: String,
    pub last_seen: f64,
}

impl LanWorld {
    pub fn label(&self) -> String {
        if self.motd.is_empty() {
            format!("Dünya :{}", self.port)
        } else {
            format!("{} :{}", self.motd, self.port)
        }
    }
}

impl Core {
    /// Worlds the game is currently broadcasting on the LAN.
    pub fn lan_worlds(&self) -> Vec<LanWorld> {
        self.ensure_lan_scanner();
        self.lan_worlds
            .lock()
            .map(|worlds| worlds.clone())
            .unwrap_or_default()
    }

    fn ensure_lan_scanner(&self) {
        static STARTED: std::sync::Once = std::sync::Once::new();
        let worlds = self.lan_worlds.clone();
        STARTED.call_once(move || {
            std::thread::spawn(move || {
                let socket = match bind_lan_socket() {
                    Some(socket) => socket,
                    None => return,
                };
                let _ = socket.set_read_timeout(Some(Duration::from_secs(1)));
                let mut buffer = [0u8; 4096];
                loop {
                    match socket.recv_from(&mut buffer) {
                        Ok((len, addr)) => {
                            let text = String::from_utf8_lossy(&buffer[..len]);
                            if let Some((motd, port)) = parse_lan_packet(&text) {
                                if let Ok(mut worlds) = worlds.lock() {
                                    worlds.retain(|world| world.port != port);
                                    worlds.push(LanWorld {
                                        motd,
                                        port,
                                        address: addr.ip().to_string(),
                                        last_seen: now_epoch() as f64,
                                    });
                                    worlds.sort_by_key(|world| world.port);
                                }
                            }
                        }
                        Err(_) => {
                            // Timeout: drop worlds that stopped broadcasting.
                            let cutoff = now_epoch() as f64 - LAN_TTL.as_secs() as f64;
                            if let Ok(mut worlds) = worlds.lock() {
                                worlds.retain(|world| world.last_seen >= cutoff);
                            }
                        }
                    }
                }
            });
        });
    }

    /// `online-mode=false` in the launcher's own and every profile's
    /// `server.properties`, so offline accounts can join LAN worlds.
    pub fn ensure_offline_properties(&self) -> usize {
        let mut targets = vec![self.mc_dir.join("server.properties")];
        if let Ok(entries) = fs::read_dir(&self.instances_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    targets.push(entry.path().join("server.properties"));
                }
            }
        }
        let mut changed = 0;
        for target in targets {
            if write_offline_properties(&target).is_ok() {
                changed += 1;
            }
        }
        if changed > 0 {
            self.log(format!(
                "Minecraft LAN için online-mode=false ayarlandı ({changed} dosya)."
            ));
        }
        changed
    }
}

fn bind_lan_socket() -> Option<UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).ok()?;
    socket.set_reuse_address(true).ok()?;
    let addr: std::net::SocketAddr = format!("0.0.0.0:{LAN_PORT}").parse().ok()?;
    socket.bind(&addr.into()).ok()?;
    Some(socket.into())
}

/// `[MOTD]Başlık[/MOTD][AD]25565[/AD]`
pub fn parse_lan_packet(text: &str) -> Option<(String, u16)> {
    let motd = text
        .find("[MOTD]")
        .and_then(|start| {
            let rest = &text[start + 6..];
            rest.find("[/MOTD]").map(|end| rest[..end].trim().to_string())
        })
        .unwrap_or_else(|| "Minecraft dünyası".to_string());
    let port = text
        .find("[AD]")
        .and_then(|start| {
            let rest = &text[start + 4..];
            rest.find("[/AD]")
                .and_then(|end| rest[..end].trim().parse::<u16>().ok())
        })?;
    if port == 0 {
        return None;
    }
    Some((motd, port))
}

/// Rewrite `server.properties` keeping every other setting, forcing
/// `online-mode=false` (the launcher only has offline accounts).
fn write_offline_properties(path: &Path) -> Result<()> {
    let mut lines: Vec<String> = fs::read_to_string(path)
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default();
    lines.retain(|line| !line.trim().to_lowercase().starts_with("online-mode"));
    lines.push("online-mode=false".to_string());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, format!("{}\n", lines.join("\n")))?;
    Ok(())
}

#[cfg(test)]
mod lan_tests {
    use super::tests::temp_core;
    use super::*;

    #[test]
    fn parses_the_minecraft_lan_broadcast() {
        let packet = "[MOTD]Balıkçı dünyası[/MOTD][AD]25565[/AD]";
        let (motd, port) = parse_lan_packet(packet).expect("paket");
        assert_eq!(motd, "Balıkçı dünyası");
        assert_eq!(port, 25565);

        // Missing MOTD falls back to a default, missing/garbage port is rejected.
        let (motd, port) = parse_lan_packet("[AD]51234[/AD]").expect("paket");
        assert_eq!(motd, "Minecraft dünyası");
        assert_eq!(port, 51234);
        assert!(parse_lan_packet("[MOTD]sadece başlık[/MOTD]").is_none());
        assert!(parse_lan_packet("[AD]0[/AD]").is_none());
        assert!(parse_lan_packet("çöp").is_none());
    }

    #[test]
    fn forces_offline_mode_and_keeps_other_properties() {
        let core = temp_core("offline");
        let root = core.mc_dir.clone();
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("server.properties"),
            "motd=Benim Dünyam\nonline-mode=true\nmax-players=10\n",
        )
        .unwrap();
        core.create_instance("Bir Profil", "1.21.4", "vanilla", "").unwrap();

        let changed = core.ensure_offline_properties();
        assert!(changed >= 2, "kök ve profil dosyası yazılmalı");

        let text = fs::read_to_string(root.join("server.properties")).unwrap();
        assert!(text.contains("online-mode=false"));
        assert!(!text.contains("online-mode=true"));
        assert!(text.contains("motd=Benim Dünyam"), "diğer ayarlar korunmalı");
        assert!(text.contains("max-players=10"));

        let profile = fs::read_to_string(
            core.instance_dir("bir_profil").join("server.properties"),
        )
        .unwrap();
        assert!(profile.contains("online-mode=false"));

        // Running it again must not duplicate the key.
        core.ensure_offline_properties();
        let text = fs::read_to_string(root.join("server.properties")).unwrap();
        assert_eq!(text.matches("online-mode").count(), 1);
    }
}
