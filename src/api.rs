//! Typed client for the Cookie Launcher Python core (`backend/server.py`).
//!
//! The core binds `127.0.0.1` on the first free port in [`PORTS`] and answers
//! JSON. A native client must not send an `Origin` header and must send
//! `Content-Type: application/json` on POST; both rules come from the core's
//! loopback same-port guard.

use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub const API_VERSION: u64 = 19;
pub const PORTS: [u16; 6] = [18420, 18421, 18422, 18423, 18424, 18425];

/// Envelope field shared by almost every logical response.
#[derive(Debug, Clone, Deserialize)]
pub struct Envelope {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct ApiClient {
    agent: ureq::Agent,
    port: u16,
}

impl ApiClient {
    pub fn new(port: u16) -> Self {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(25)))
            .build();
        Self {
            agent: ureq::Agent::new_with_config(config),
            port,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }

    pub fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let mut response = self.agent.get(&self.url(path)).call()?;
        let body = response.body_mut().read_to_string()?;
        serde_json::from_str(&body).map_err(|e| anyhow!("{path}: {e}"))
    }

    /// GET a JSON document that also carries `success`/`error`, surfacing the
    /// core's Turkish error text instead of a deserialization failure.
    pub fn get_checked<T: DeserializeOwned + HasEnvelope>(&self, path: &str) -> Result<T> {
        let value: T = self.get(path)?;
        if let Some(message) = value.envelope_error() {
            return Err(anyhow!(message));
        }
        Ok(value)
    }

    pub fn post<T: DeserializeOwned, B: Serialize>(&self, path: &str, body: &B) -> Result<T> {
        let payload = serde_json::to_vec(body)?;
        let mut response = self
            .agent
            .post(&self.url(path))
            .header("Content-Type", "application/json")
            .send(payload)?;
        let raw = response.body_mut().read_to_string()?;
        serde_json::from_str(&raw).map_err(|e| anyhow!("{path}: {e}"))
    }

    pub fn post_checked<T: HasEnvelope + DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let value: T = self.post(path, body)?;
        if let Some(message) = value.envelope_error() {
            return Err(anyhow!(message));
        }
        Ok(value)
    }

    /// Fetch a binary endpoint (screenshots, content icons).
    pub fn get_bytes(&self, path: &str) -> Result<Vec<u8>> {
        let mut response = self.agent.get(&self.url(path)).call()?;
        Ok(response.body_mut().read_to_vec()?)
    }

    pub fn health(&self) -> Result<Health> {
        self.get("/api/health")
    }
}

/// A response type that reports logical failure through `success`/`error`.
pub trait HasEnvelope {
    fn envelope_error(&self) -> Option<String>;
}

// ---------------------------------------------------------------------------
// Health / status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct Health {
    #[serde(default)]
    pub api_version: u64,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub app: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct StatusPayload {
    #[serde(default)]
    pub is_installing: bool,
    #[serde(default)]
    pub is_running: bool,
    #[serde(default)]
    pub percent: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub current_file: String,
    #[serde(default)]
    pub downloaded_mb: f64,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub minecraft_pid: Option<i64>,
    #[serde(default)]
    pub launch_instance_id: Option<String>,
}

impl StatusPayload {
    pub fn is_busy(&self) -> bool {
        self.is_installing || self.is_running
    }
}

// ---------------------------------------------------------------------------
// Instances
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Instance {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub loader: String,
    #[serde(default)]
    pub loader_version: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub mod_count: i64,
    #[serde(default)]
    pub install_status: Option<String>,
    #[serde(default)]
    pub install_progress: Option<i64>,
    #[serde(default)]
    pub install_detail: Option<String>,
    /// Set when the profile came from a Modrinth modpack.
    #[serde(default)]
    pub pack_slug: String,
    #[serde(default)]
    pub pack_version: String,
    #[serde(default)]
    pub pack_version_id: String,
    /// Files the pack installed, so an update can remove what disappeared.
    #[serde(default)]
    pub pack_files: Vec<String>,
}

impl Instance {
    pub fn loader_label(&self) -> &'static str {
        loader_label(&self.loader)
    }

    pub fn is_installing(&self) -> bool {
        self.install_status.as_deref() == Some("installing")
    }
}

pub fn loader_label(loader: &str) -> &'static str {
    match loader.to_ascii_lowercase().as_str() {
        "fabric" => "Fabric",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "quilt" => "Quilt",
        _ => "Vanilla",
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstancesResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub instances: Vec<Instance>,
    #[serde(default)]
    pub error: Option<String>,
}

impl HasEnvelope for InstancesResponse {
    fn envelope_error(&self) -> Option<String> {
        if self.success {
            None
        } else {
            self.error.clone()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstanceResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub instance: Option<Instance>,
    #[serde(default)]
    pub error: Option<String>,
}

impl HasEnvelope for InstanceResponse {
    fn envelope_error(&self) -> Option<String> {
        if self.success {
            None
        } else {
            self.error.clone()
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CreateInstanceRequest<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub loader: &'a str,
    pub loader_version: &'a str,
    pub icon: &'a str,
}

#[derive(Debug, Serialize)]
pub struct IdRequest<'a> {
    pub id: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OkResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl HasEnvelope for OkResponse {
    fn envelope_error(&self) -> Option<String> {
        if self.success {
            None
        } else {
            self.error.clone()
        }
    }
}

// ---------------------------------------------------------------------------
// Versions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct MinecraftVersion {
    pub id: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(rename = "releaseTime", default)]
    pub release_time: String,
    #[serde(default)]
    pub is_installed: bool,
}

impl MinecraftVersion {
    pub fn year(&self) -> &str {
        self.release_time.get(0..4).unwrap_or("")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionsResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub versions: Vec<MinecraftVersion>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoaderVersions {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub recommended: Option<String>,
    #[serde(default)]
    pub versions: Vec<String>,
    #[serde(default)]
    pub stable_only: Vec<String>,
    #[serde(default)]
    pub supported: bool,
    #[serde(default)]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Launch / logs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct LaunchRequest {
    pub instance_id: Option<String>,
    pub username: String,
    pub ram: i64,
    pub cookie_optimized: bool,
    pub jvm_preset: String,
    #[serde(default)]
    pub quick_play: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LaunchResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

impl HasEnvelope for LaunchResponse {
    fn envelope_error(&self) -> Option<String> {
        if self.success {
            None
        } else {
            self.error.clone()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LogsResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub logs: Vec<String>,
}

// ---------------------------------------------------------------------------
// Modrinth
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchHit {
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: i64,
    #[serde(default)]
    pub follows: i64,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub icon_url: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub hits: Vec<SearchHit>,
    #[serde(default)]
    pub total_hits: i64,
    #[serde(default)]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InstallRequest {
    pub slug: String,
    pub instance_id: String,
    pub project_type: String,
    pub force: bool,
    pub install_dependencies: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InstallResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub installed: Option<Vec<String>>,
}

impl HasEnvelope for InstallResponse {
    fn envelope_error(&self) -> Option<String> {
        if self.success {
            None
        } else {
            self.error.clone()
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct World {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size_mb: f64,
    #[serde(default)]
    pub last_played: String,
    #[serde(default)]
    pub has_icon: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WorldsResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub worlds: Vec<World>,
    #[serde(default)]
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Screenshots
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Screenshot {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub mtime: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScreenshotsResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub screenshots: Vec<Screenshot>,
}

// ---------------------------------------------------------------------------
// Social
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SocialFriend {
    #[serde(default)]
    pub player_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub online: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SocialProfile {
    #[serde(default)]
    pub player_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub code: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SocialRequests {
    #[serde(default, rename = "in")]
    pub incoming: Vec<SocialFriend>,
    #[serde(default, rename = "out")]
    pub outgoing: Vec<SocialFriend>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SocialInvite {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub web_link: String,
    #[serde(default)]
    pub deep_link: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HostRelayStatus {
    #[serde(default)]
    pub running: bool,
    #[serde(default)]
    pub error: String,
}

/// `GET /api/social/state` — the whole social snapshot. Every field is
/// optional: while the social service is unavailable the core omits most of
/// them, so the client must render an honest empty state rather than fail.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SocialStateResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub profile: Option<SocialProfile>,
    #[serde(default)]
    pub friends: Vec<SocialFriend>,
    #[serde(default)]
    pub requests: SocialRequests,
    #[serde(default)]
    pub invite: Option<SocialInvite>,
    #[serde(default)]
    pub host_relay: Option<HostRelayStatus>,
}

impl SocialStateResponse {
    pub fn display_name(&self) -> String {
        self.profile
            .as_ref()
            .and_then(|p| p.name.clone())
            .unwrap_or_default()
    }

    pub fn code(&self) -> String {
        self.profile
            .as_ref()
            .map(|p| p.code.clone())
            .unwrap_or_default()
    }

    pub fn pending_incoming(&self) -> usize {
        self.requests.incoming.len()
    }
}

#[derive(Debug, Serialize)]
pub struct FriendCodeRequest {
    pub code: String,
}

#[derive(Debug, Serialize)]
pub struct PlayerIdRequest {
    pub player_id: String,
}

#[derive(Debug, Serialize)]
pub struct FriendRespondRequest {
    pub player_id: String,
    pub accept: bool,
}

/// Result of comparing a profile's pack with the newest published version.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PackStatus {
    pub slug: String,
    pub current: String,
    pub latest: String,
    pub update_available: bool,
}
