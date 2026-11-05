//! Vanilla Minecraft launch pipeline, ported from the Python core.
//!
//! `prepare` resolves the version JSON, downloads the client jar, libraries,
//! natives and asset objects, then `spawn` starts the JVM with the same
//! command line the Tauri build used. All progress is reported through the
//! core's status + log ring.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

use crate::core::Core;

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

pub struct Prepared {
    pub main_class: String,
    pub classpath: String,
    pub assets_dir: PathBuf,
    pub asset_index: String,
    pub natives_dir: PathBuf,
    pub classpath_sep: char,
}

#[derive(Deserialize, serde::Serialize)]
struct VersionJson {
    #[serde(default)]
    id: String,
    #[serde(rename = "mainClass", default)]
    main_class: String,
    #[serde(rename = "inheritsFrom", default)]
    inherits_from: Option<String>,
    #[serde(default)]
    libraries: Vec<Library>,
    #[serde(rename = "assetIndex", default)]
    asset_index: Option<AssetIndexRef>,
    #[serde(default)]
    downloads: Option<Downloads>,
    #[serde(rename = "javaVersion", default)]
    java_version: Option<JavaVersion>,
}

#[derive(Deserialize, Serialize, Default)]
struct Downloads {
    #[serde(default)]
    client: Option<Download>,
}

#[derive(Deserialize, Serialize, Clone)]
struct Download {
    #[serde(default)]
    url: String,
    #[serde(default)]
    sha1: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    size: u64,
}

#[derive(Deserialize, Serialize, Default)]
struct AssetIndexRef {
    #[serde(default)]
    id: String,
    #[serde(default)]
    url: String,
}

#[derive(Deserialize, Serialize, Default)]
struct JavaVersion {
    #[serde(rename = "majorVersion", default)]
    major: u32,
}

#[derive(Deserialize, serde::Serialize)]
struct Library {
    #[serde(default)]
    name: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    downloads: Option<LibraryDownloads>,
    #[serde(default)]
    natives: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Deserialize, Serialize, Default)]
struct LibraryDownloads {
    #[serde(default)]
    artifact: Option<Download>,
    #[serde(default)]
    classifiers: Option<std::collections::HashMap<String, Download>>,
}

#[derive(Deserialize, serde::Serialize)]
struct Rule {
    #[serde(default)]
    action: String,
    #[serde(default)]
    os: Option<RuleOs>,
}

#[derive(Deserialize, serde::Serialize)]
struct RuleOs {
    #[serde(default)]
    name: String,
    #[serde(default)]
    arch: Option<String>,
}

#[derive(Deserialize)]
struct AssetIndexFile {
    #[serde(default)]
    objects: std::collections::HashMap<String, AssetObject>,
}

#[derive(Deserialize)]
struct AssetObject {
    #[serde(default)]
    hash: String,
    #[serde(default)]
    size: u64,
}

pub struct LaunchOptions<'a> {
    pub version: &'a str,
    pub instance_id: &'a str,
    pub username: &'a str,
    pub ram_gb: u64,
    pub quick_play: Option<String>,
}

impl Core {
    pub fn prepare(&self, version_id: &str) -> Result<Prepared> {
        let version_json_path = self.mc_dir.join("versions").join(version_id).join(format!("{version_id}.json"));
        let version_dir = self.mc_dir.join("versions").join(version_id);
        fs::create_dir_all(&version_dir)?;

        let mut jar_version = version_id.to_string();
        let json: VersionJson = if let Some((loader, base_id)) = loader_profile(self, version_id)? {
            jar_version = base_id;
            loader
        } else if version_json_path.exists() {
            serde_json::from_str(&fs::read_to_string(&version_json_path)?)?
        } else {
            let manifest: serde_json::Value = self.fetch_json(MANIFEST_URL)?;
            let url = manifest["versions"]
                .as_array()
                .and_then(|versions| {
                    versions.iter().find(|entry| {
                        entry.get("id").and_then(|id| id.as_str()) == Some(version_id)
                    })
                })
                .and_then(|entry| entry.get("url"))
                .and_then(|url| url.as_str())
                .ok_or_else(|| anyhow!("Sürüm bulunamadı: {version_id}"))?;
            let parsed: VersionJson = self.fetch_json(url)?;
            fs::write(&version_json_path, serde_json::to_vec_pretty(&parsed)?)?;
            parsed
        };

        self.set_progress(15, "Minecraft indiriliyor…", version_id);

        // Client jar lives with the version that owns it (for a loader profile
        // that is the vanilla version it inherits from).
        if let Some(client) = json.downloads.as_ref().and_then(|d| d.client.clone()) {
            let jar_dir = self.mc_dir.join("versions").join(&jar_version);
            let jar = jar_dir.join(format!("{jar_version}.jar"));
            if !jar.exists() {
                self.download_to(&client.url, &jar)?;
            }
        }

        // Libraries + natives
        let lib_dir = self.mc_dir.join("libraries");
        let natives_dir = version_dir.join("natives");
        fs::create_dir_all(&natives_dir)?;
        let mut classpath: Vec<String> = Vec::new();

        let os_name = std::env::consts::OS; // "linux" | "macos" | "windows"
        let mut done = 0usize;
        let total = json.libraries.len().max(1);

        for library in &json.libraries {
            if !rules_allow(&library.rules, os_name) {
                continue;
            }
            done += 1;
            if done % 8 == 0 {
                self.set_progress(
                    15 + (done as u32 * 45 / total as u32),
                    "Kütüphaneler indiriliyor…",
                    &library.name,
                );
            }

            if let Some(artifact) = library.downloads.as_ref().and_then(|d| d.artifact.clone()) {
                if artifact.path.is_empty() {
                    continue;
                }
                let target = lib_dir.join(&artifact.path);
                if !target.exists() {
                    if let Ok(bytes) = self.fetch_bytes(&artifact.url) {
                        if let Some(parent) = target.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(&target, bytes)?;
                    }
                }
                classpath.push(target.to_string_lossy().to_string());
            } else if let (Some(base), Some(path)) = (&library.url, maven_path(&library.name)) {
                // Fabric/Quilt profiles name their libraries by Maven
                // coordinates and give only the repository root.
                let target = lib_dir.join(&path);
                if !target.exists() {
                    let url = format!("{}{}", base.trim_end_matches('/'), format!("/{path}"));
                    if let Ok(bytes) = self.fetch_bytes(&url) {
                        if let Some(parent) = target.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(&target, bytes)?;
                    }
                }
                if target.exists() {
                    classpath.push(target.to_string_lossy().to_string());
                }
            }

            // Natives: download the classifier and unzip into natives/.
            if let Some(natives) = &library.natives {
                let key = match os_name {
                    "macos" => "natives-osx",
                    "windows" => "natives-windows",
                    _ => "natives-linux",
                };
                if let Some(classifier) = natives
                    .get(key)
                    .or_else(|| natives.get("natives-linux"))
                    .or_else(|| natives.values().next())
                {
                    if let Some(download) = library
                        .downloads
                        .as_ref()
                        .and_then(|d| d.classifiers.as_ref())
                        .and_then(|c| c.get(classifier).cloned())
                    {
                        let jar = lib_dir.join(format!("natives-{}.jar", download.path.replace('/', "-")));
                        if !jar.exists() {
                            if let Ok(bytes) = self.fetch_bytes(&download.url) {
                                if let Some(parent) = jar.parent() {
                                    fs::create_dir_all(parent)?;
                                }
                                fs::write(&jar, bytes)?;
                            }
                        }
                        let _ = extract_zip(&jar, &natives_dir);
                    }
                }
            }
        }

        // Assets
        self.set_progress(62, "Varlıklar indiriliyor…", "");
        let assets_dir = self.mc_dir.join("assets");
        let index_dir = assets_dir.join("indexes");
        let objects_dir = assets_dir.join("objects");
        fs::create_dir_all(&index_dir)?;
        fs::create_dir_all(&objects_dir)?;

        let mut asset_index_id = String::new();
        if let Some(index) = &json.asset_index {
            asset_index_id = index.id.clone();
            let index_path = index_dir.join(format!("{}.json", index.id));
            if !index_path.exists() {
                if let Ok(bytes) = self.fetch_bytes(&index.url) {
                    fs::write(&index_path, &bytes)?;
                }
            }
            if let Ok(text) = fs::read_to_string(&index_path) {
                if let Ok(parsed) = serde_json::from_str::<AssetIndexFile>(&text) {
                    let total = parsed.objects.len().max(1);
                    for (count, object) in parsed.objects.values().enumerate() {
                        if object.hash.len() < 2 {
                            continue;
                        }
                        let target = objects_dir.join(&object.hash[..2]).join(&object.hash);
                        if target.exists() {
                            continue;
                        }
                        let url = format!(
                            "{RESOURCES_URL}/{}/{}",
                            &object.hash[..2],
                            object.hash
                        );
                        if let Ok(bytes) = self.fetch_bytes(&url) {
                            if let Some(parent) = target.parent() {
                                let _ = fs::create_dir_all(parent);
                            }
                            let _ = fs::write(&target, bytes);
                        }
                        if count % 100 == 0 {
                            self.set_progress(
                                62 + (count as u32 * 30 / total as u32),
                                "Varlıklar indiriliyor…",
                                &format!("{count}/{total}"),
                            );
                        }
                    }
                }
            }
        }

        let _ = json.java_version.map(|j| j.major);
        let client_jar = self
            .mc_dir
            .join("versions")
            .join(&jar_version)
            .join(format!("{jar_version}.jar"));
        classpath.push(client_jar.to_string_lossy().to_string());

        Ok(Prepared {
            main_class: if json.main_class.is_empty() {
                "net.minecraft.client.main.Main".to_string()
            } else {
                json.main_class
            },
            classpath: classpath.join(":"),
            assets_dir,
            asset_index: asset_index_id,
            natives_dir,
            classpath_sep: ':' as char,
        })
    }

    fn set_progress(&self, percent: u32, status: &str, detail: &str) {
        self.set_status(|s: &mut crate::api::StatusPayload| {
            s.is_installing = percent < 100;
            s.percent = percent as i64;
            s.status = status.to_string();
            s.detail = detail.to_string();
        });
        self.log(format!("{status} {detail}"));
    }
}

/// Runs `prepare` then the JVM, reporting through the core.
impl Core {
    pub fn launch_now(self: &std::sync::Arc<Core>, options: LaunchOptions<'_>) -> Result<()> {
        self.log(format!(
            "Hazırlanıyor: {} ({}, {} GB)",
            options.version, options.instance_id, options.ram_gb
        ));
        let prepared = self.prepare(options.version)?;

        let instance_dir = self.instances_dir.join(options.instance_id);
        fs::create_dir_all(&instance_dir)?;

        let ram = options.ram_gb.clamp(2, 64);
        let mut command = Command::new(java_binary());
        command
            .arg(format!("-Xmx{ram}G"))
            .arg(format!("-Xms{}G", (ram / 2).max(1)))
            .arg(format!("-Djava.library.path={}", prepared.natives_dir.display()))
            .arg("-cp")
            .arg(&prepared.classpath)
            .arg(&prepared.main_class)
            .arg("--username")
            .arg(options.username)
            .arg("--version")
            .arg(options.version)
            .arg("--gameDir")
            .arg(&instance_dir)
            .arg("--assetsDir")
            .arg(&prepared.assets_dir)
            .arg("--assetIndex")
            .arg(&prepared.asset_index)
            .arg("--uuid")
            .arg(offline_uuid(options.username))
            .arg("--accessToken")
            .arg("0")
            .arg("--userType")
            .arg("legacy");

        // Joining a friend's world: point the client straight at the local
        // authenticated proxy instead of the main menu.
        if let Some(address) = &options.quick_play {
            command.arg("--quickPlayMultiplayer").arg(address);
        }

        command
            .current_dir(&instance_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.log(format!("Java başlatılıyor: {ram} GB, {}", options.username));
        let mut child = command.spawn().context("Java başlatılamadı")?;
        let pid = child.id();
        self.set_launch_pid(pid);

        self.set_status(|s| {
            s.is_installing = false;
            s.is_running = true;
            s.percent = 100;
            s.status = "✨ Oyun Başarıyla Açıldı!".to_string();
            s.detail = "İyi oyunlar!".to_string();
            s.minecraft_pid = Some(pid as i64);
        });

        if let Some(stdout) = child.stdout.take() {
            let core = self.clone();
            std::thread::spawn(move || {
                pump(stdout, core);
            });
        }
        if let Some(stderr) = child.stderr.take() {
            let core = self.clone();
            std::thread::spawn(move || {
                pump(stderr, core);
            });
        }

        let core = self.clone();
        std::thread::spawn(move || {
            let status = child.wait();
            core.clear_launch_pid();
            let code = status.as_ref().ok().and_then(|s| s.code()).unwrap_or(-1);
            core.set_status(|s| {
                s.is_installing = false;
                s.is_running = false;
                s.percent = 0;
                s.minecraft_pid = None;
                if code == 0 {
                    s.status = "Oyun kapandı.".to_string();
                    s.detail = String::new();
                } else {
                    s.status = "Oyun beklenmedik şekilde kapandı.".to_string();
                    s.detail = format!("Çıkış kodu: {code}");
                    s.error = Some(format!("Oyun çıkış kodu {code} ile kapandı."));
                }
            });
            core.log(format!("Oyun kapandı (kod {code})."));
        });

        Ok(())
    }
}

fn pump(reader: impl Read + Send + 'static, core: std::sync::Arc<Core>) {
    use std::io::{BufRead, BufReader};
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    while reader.read_line(&mut line).unwrap_or(0) > 0 {
        let trimmed = line.trim_end();
        if !trimmed.is_empty() {
            core.log(trimmed);
        }
        line.clear();
    }
}

fn rules_allow(rules: &[Rule], os_name: &str) -> bool {
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules {
        let matches = match &rule.os {
            None => true,
            Some(os) => {
                let name_matches = os.name.is_empty() || os.name == os_name
                    || (os.name == "osx" && os_name == "macos");
                let arch_matches = os
                    .arch
                    .as_ref()
                    .map(|arch| match arch.as_str() {
                        "x86" => cfg!(target_arch = "x86"),
                        "arm64" => cfg!(target_arch = "aarch64"),
                        _ => true,
                    })
                    .unwrap_or(true);
                name_matches && arch_matches
            }
        };
        if matches {
            allowed = rule.action == "allow";
        }
    }
    allowed
}

fn java_binary() -> String {
    if let Ok(explicit) = std::env::var("COOKIE_JAVA") {
        return explicit;
    }
    "java".to_string()
}

/// `OfflinePlayer:<name>` UUID v3, the same value the Python core sends.
fn offline_uuid(username: &str) -> String {
    let digest = compute_md5(format!("OfflinePlayer:{username}").as_bytes());
    let mut bytes = digest;
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

pub(crate) fn extract_zip(archive: &Path, target: &Path) -> Result<()> {
    let status = Command::new("unzip")
        .arg("-q")
        .arg("-o")
        .arg(archive)
        .arg("-d")
        .arg(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if status.map(|s| s.success()).unwrap_or(false) {
        return Ok(());
    }
    let _ = Command::new("jar")
        .arg("xf")
        .arg(archive)
        .current_dir(target)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

fn compute_md5(bytes: &[u8]) -> [u8; 16] {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(bytes);
    let out = hasher.finalize();
    let mut digest = [0u8; 16];
    digest.copy_from_slice(&out);
    digest
}

/// `group:artifact:version[:classifier]` -> `group:artifact:classifier`
fn library_key(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    Some(format!(
        "{}:{}:{}",
        parts[0],
        parts[1],
        parts.get(3).copied().unwrap_or("")
    ))
}

/// `net.fabricmc:fabric-loader:0.16.9` -> `net/fabricmc/fabric-loader/0.16.9/fabric-loader-0.16.9.jar`
fn maven_path(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(':').collect();
    if parts.len() < 3 {
        return None;
    }
    let group = parts[0].replace('.', "/");
    let artifact = parts[1];
    let version = parts[2];
    let classifier = parts
        .get(3)
        .map(|value| format!("-{value}"))
        .unwrap_or_default();
    Some(format!(
        "{group}/{artifact}/{version}/{artifact}-{version}{classifier}.jar"
    ))
}

/// Fabric (and Quilt) publish a launch profile that `inheritsFrom` the vanilla
/// version. Resolving it means merging its libraries with the base version and
/// keeping its main class.
fn loader_profile(core: &Core, version_id: &str) -> Result<Option<(VersionJson, String)>> {
    let is_fabric = version_id.starts_with("fabric-loader-");
    let is_quilt = version_id.starts_with("quilt-loader-");
    if !is_fabric && !is_quilt {
        return Ok(None);
    }

    let dir = core.mc_dir.join("versions").join(version_id);
    fs::create_dir_all(&dir)?;
    let cache = dir.join(format!("{version_id}.json"));

    let profile: VersionJson = if cache.exists() {
        serde_json::from_str(&fs::read_to_string(&cache)?)?
    } else {
        // fabric-loader-<loader>-<mc>  /  quilt-loader-<loader>-<mc>
        let rest = version_id
            .trim_start_matches("fabric-loader-")
            .trim_start_matches("quilt-loader-");
        let mut split = rest.rsplitn(2, '-');
        let mc = split.next().unwrap_or("");
        let loader = split.next().unwrap_or("");
        let url = if is_fabric {
            format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{loader}/profile/json")
        } else {
            format!("https://meta.quiltmc.org/v3/versions/loader/{mc}/{loader}/profile/json")
        };
        let parsed: VersionJson = core.fetch_json(&url)?;
        fs::write(&cache, serde_json::to_vec_pretty(&parsed)?)?;
        parsed
    };

    let base_id = profile
        .inherits_from
        .clone()
        .unwrap_or_else(|| version_id.to_string());
    let base_path = core
        .mc_dir
        .join("versions")
        .join(&base_id)
        .join(format!("{base_id}.json"));
    let base: VersionJson = if base_path.exists() {
        serde_json::from_str(&fs::read_to_string(&base_path)?)?
    } else {
        let manifest: serde_json::Value = core.fetch_json(MANIFEST_URL)?;
        let url = manifest["versions"]
            .as_array()
            .and_then(|versions| {
                versions
                    .iter()
                    .find(|entry| entry.get("id").and_then(|id| id.as_str()) == Some(base_id.as_str()))
            })
            .and_then(|entry| entry.get("url"))
            .and_then(|url| url.as_str())
            .ok_or_else(|| anyhow!("Temel sürüm bulunamadı: {base_id}"))?;
        let parsed: VersionJson = core.fetch_json(url)?;
        if let Some(parent) = base_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&base_path, serde_json::to_vec_pretty(&parsed)?)?;
        parsed
    };

    // Loader libraries first, then the base version. Fabric refuses to start
    // when the same artifact appears twice with different versions (ASM), so
    // the loader's copy wins and the base duplicate is dropped.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut libraries: Vec<Library> = Vec::new();
    for library in profile.libraries.into_iter().chain(base.libraries) {
        if let Some(key) = library_key(&library.name) {
            if !seen.insert(key) {
                continue;
            }
        }
        libraries.push(library);
    }

    Ok(Some((VersionJson {
        id: version_id.to_string(),
        main_class: if profile.main_class.is_empty() {
            base.main_class
        } else {
            profile.main_class
        },
        inherits_from: None,
        libraries,
        asset_index: base.asset_index,
        downloads: base.downloads,
        java_version: base.java_version,
    }, base_id)))
}
