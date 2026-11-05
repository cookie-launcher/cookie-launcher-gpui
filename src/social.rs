//! Remote friends: the CL1 invite codec and the bore.pub tunnel client,
//! ported from `backend/invite_code.py` and `backend/share_relay.py`.
//!
//! Flow (same as the Tauri build):
//!   1. host opens a bore tunnel to its local relay/Minecraft port,
//!   2. host publishes a `CL1-…` code carrying room + session + endpoint,
//!   3. guest connects to the public relay port, splices to its game.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

pub use crate::relay::LocalProxy;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

pub const MAGIC: &str = "CL1";

// ---------------------------------------------------------------------------
// Invite codes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Invite {
    pub version: u32,
    pub room_id: String,
    pub session_id: String,
    pub expires: u64,
    #[serde(default)]
    pub relay: Option<RelayEndpoint>,
    #[serde(default)]
    pub host_name: String,
    #[serde(default)]
    pub host_code: String,
    #[serde(default)]
    pub noise_pub: String,
    #[serde(default)]
    pub world: String,
    /// CLT1 ticket id (hex) — only in long codes; never in the short form.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ticket: String,
    /// CLT1 token (hex) — the tunnel secret.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    #[serde(skip)]
    pub raw: String,
    #[serde(skip)]
    pub short: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelayEndpoint {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub tls: bool,
}

impl Invite {
    pub fn expired(&self) -> bool {
        self.expires != 0 && self.expires + 60 < now()
    }

    pub fn web_link(&self) -> String {
        format!(
            "https://cookie-launcher.github.io/join/?c={}",
            urlencode(&self.raw)
        )
    }

    pub fn deep_link(&self) -> String {
        format!("cl://join/{}", self.raw)
    }
}

/// `CL1-<room13>-<session7>-<exp8hex>[-<hostcode>]`
pub fn make_short(room_id: &str, session_id: &str, expires: u64, host_code: &str) -> Result<String> {
    let room = room_id.trim().to_uppercase();
    let session = session_id.trim().to_uppercase();
    if room.len() != 13 || !room.chars().all(|c| "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".contains(c)) {
        return Err(anyhow!("room_id 13 karakter base32 olmalı"));
    }
    if session.len() != 7 || !session.chars().all(|c| "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567".contains(c)) {
        return Err(anyhow!("session_id 7 karakter base32 olmalı"));
    }
    let mut code = format!(
        "{MAGIC}-{room}-{session}-{:08X}",
        (expires & 0xFFFF_FFFF) as u32
    );
    let host: String = host_code
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase();
    if !host.is_empty() {
        code.push('-');
        code.push_str(&host);
    }
    Ok(code)
}

/// `CL1-<base64url(json)>` — endpoint embedded, no rendezvous needed.
pub fn make_long(invite: &Invite) -> String {
    let payload = serde_json::json!({
        "v": 1,
        "rid": invite.room_id,
        "sid": invite.session_id,
        "exp": invite.expires,
        "relay": invite.relay,
        "host_name": invite.host_name,
        "host_code": invite.host_code,
        "noise_pub": invite.noise_pub,
        "world": invite.world,
        "tid": invite.ticket,
        "tok": invite.token,
    });
    format!(
        "{MAGIC}-{}",
        base64url_encode(payload.to_string().as_bytes())
    )
}

pub fn normalize(text: &str) -> String {
    let mut value = text.trim().trim_matches(|c| "<>()[]{}\"'".contains(c)).to_string();

    // `…/join/?c=CL1-…` wins over the plain path form.
    if let Some(idx) = value.find("?c=") {
        let rest = &value[idx + 3..];
        value = rest.split(['&', '#', ' ', '\n']).next().unwrap_or(rest).to_string();
    } else if let Some(idx) = value.find("/join/") {
        let rest = &value[idx + 6..];
        value = rest.split(['?', '#', ' ', '\n']).next().unwrap_or(rest).to_string();
    }

    let value = urldecode(&value);
    let value = value.trim().trim_matches(|c| "<>()[]{}\"'".contains(c));
    if value.to_lowercase().starts_with("cl1-") {
        format!("{MAGIC}-{}", &value[4..])
    } else {
        value.to_string()
    }
}

pub fn parse(text: &str) -> Result<Invite> {
    let code = normalize(text);
    if code.is_empty() {
        return Err(anyhow!("Boş davet kodu"));
    }

    let short: Vec<&str> = code.split('-').collect();
    if short.len() >= 4 && short[1].len() == 13 && short[2].len() == 7 {
        let expires = u64::from_str_radix(short[3], 16).unwrap_or(0);
        let invite = Invite {
            version: 1,
            room_id: short[1].to_string(),
            session_id: short[2].to_string(),
            expires,
            host_code: short.get(4).copied().unwrap_or("").to_string(),
            raw: code.clone(),
            short: true,
            ..Default::default()
        };
        if invite.expired() {
            return Err(anyhow!("Davet kodunun süresi dolmuş"));
        }
        return Ok(invite);
    }

    if code.len() > 24 {
        let body = &code[4..];
        if let Some(bytes) = base64url_decode(body) {
            #[derive(Deserialize)]
            struct Payload {
                #[serde(default)]
                v: u32,
                #[serde(default)]
                rid: String,
                #[serde(default)]
                sid: String,
                #[serde(default)]
                exp: u64,
                #[serde(default)]
                relay: Option<RelayEndpoint>,
                #[serde(default)]
                host_name: String,
                #[serde(default)]
                host_code: String,
                #[serde(default)]
                noise_pub: String,
                #[serde(default)]
                world: String,
                #[serde(default)]
                tid: String,
                #[serde(default)]
                tok: String,
            }
            if let Ok(payload) = serde_json::from_slice::<Payload>(&bytes) {
                if payload.v != 1 {
                    return Err(anyhow!("Desteklenmeyen davet sürümü"));
                }
                let invite = Invite {
                    version: 1,
                    room_id: payload.rid,
                    session_id: payload.sid,
                    expires: payload.exp,
                    relay: payload.relay,
                    host_name: payload.host_name,
                    host_code: payload.host_code,
                    noise_pub: payload.noise_pub,
                    world: payload.world,
                    ticket: payload.tid,
                    token: payload.tok,
                    raw: code,
                    short: false,
                };
                if invite.expired() {
                    return Err(anyhow!("Davet kodunun süresi dolmuş"));
                }
                return Ok(invite);
            }
        }
    }

    Err(anyhow!("Geçersiz CL1 davet kodu"))
}

// ---------------------------------------------------------------------------
// bore.pub tunnel
// ---------------------------------------------------------------------------

/// A live bore tunnel: `public_port` on `server` forwards to `local_port`.
pub struct BoreTunnel {
    pub server: String,
    pub public_port: u16,
    local_port: u16,
    stop: Arc<AtomicBool>,
    control: Arc<Mutex<Option<TcpStream>>>,
}

impl BoreTunnel {
    pub fn open(server: &str, local_port: u16) -> Result<BoreTunnel> {
        let server = if server.trim().is_empty() {
            "bore.pub".to_string()
        } else {
            server.trim().to_string()
        };
        let mut control = TcpStream::connect((server.as_str(), 7835))
            .map_err(|e| anyhow!("bore bağlantısı kurulamadı: {e}"))?;
        control.set_read_timeout(Some(Duration::from_secs(15)))?;

        send_message(&mut control, &serde_json::json!({ "Hello": 0 }))?;
        let hello = read_message(&mut control)?
            .ok_or_else(|| anyhow!("bore.pub hello yanıtı gelmedi"))?;
        let public_port = hello
            .get("Hello")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| anyhow!("bore.pub public port vermedi"))?;
        if public_port == 0 {
            return Err(anyhow!("bore.pub public port vermedi"));
        }

        control.set_read_timeout(None)?;
        let stop = Arc::new(AtomicBool::new(false));
        let control = Arc::new(Mutex::new(Some(control)));

        // Control loop: every `{"Connection": uuid}` opens a data socket that
        // is spliced to the local service.
        let worker_stop = stop.clone();
        let worker_control = control.clone();
        let worker_server = server.clone();
        std::thread::spawn(move || {
            loop {
                if worker_stop.load(Ordering::Relaxed) {
                    break;
                }
                let message = {
                    let mut guard = match worker_control.lock() {
                        Ok(guard) => guard,
                        Err(_) => break,
                    };
                    match guard.as_mut() {
                        Some(stream) => read_message(stream),
                        None => break,
                    }
                };
                let Some(message) = message.ok().flatten() else {
                    break;
                };
                let Some(connection) = message
                    .get("Connection")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string())
                else {
                    continue;
                };
                let server = worker_server.clone();
                std::thread::spawn(move || {
                    if let Err(error) = splice_connection(&server, &connection, local_port) {
                        eprintln!("bore splice: {error}");
                    }
                });
            }
        });

        Ok(BoreTunnel {
            server,
            public_port: public_port as u16,
            local_port,
            stop,
            control,
        })
    }

    pub fn endpoint(&self) -> RelayEndpoint {
        RelayEndpoint {
            host: self.server.clone(),
            port: self.public_port,
            tls: false,
        }
    }

    pub fn close(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut guard) = self.control.lock() {
            if let Some(stream) = guard.as_mut() {
                let _ = stream.shutdown(std::net::Shutdown::Both);
            }
            *guard = None;
        }
    }
}

fn splice_connection(server: &str, connection: &str, local_port: u16) -> Result<()> {
    let mut data = TcpStream::connect((server, 7835))?;
    send_message(&mut data, &serde_json::json!({ "Accept": connection }))?;
    let mut local = TcpStream::connect(("127.0.0.1", local_port))?;

    let mut data_read = data.try_clone()?;
    let mut local_write = local.try_clone()?;
    let up = std::thread::spawn(move || {
        let _ = std::io::copy(&mut data_read, &mut local_write);
        let _ = local_write.shutdown(std::net::Shutdown::Write);
    });
    let mut local_read = local;
    let _ = std::io::copy(&mut local_read, &mut data);
    let _ = data.shutdown(std::net::Shutdown::Write);
    let _ = up.join();
    Ok(())
}

fn send_message(stream: &mut TcpStream, value: &serde_json::Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(value)?;
    bytes.push(0);
    stream.write_all(&bytes)?;
    stream.flush()?;
    Ok(())
}

fn read_message(stream: &mut TcpStream) -> Result<Option<serde_json::Value>> {
    let mut buffer = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => return Ok(None),
            Ok(_) => {
                if byte[0] == 0 {
                    return Ok(serde_json::from_slice(&buffer).ok());
                }
                buffer.push(byte[0]);
                if buffer.len() > 8192 {
                    return Ok(None);
                }
            }
            Err(_) => return Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// Sharing
// ---------------------------------------------------------------------------

/// A running share: local port exposed through bore, with its invite code.
pub struct ShareSession {
    pub tunnel: BoreTunnel,
    pub relay: crate::relay::RelayServer,
    pub invite: Invite,
    pub code: String,
}

pub struct ShareConfig {
    pub server: String,
    pub host_name: String,
    pub host_code: String,
    pub world: String,
    pub local_port: u16,
}

impl ShareSession {
    pub fn start(config: ShareConfig) -> Result<ShareSession> {
        // The relay authenticates the guest before touching the game socket,
        // then bore exposes only the relay's port to the internet.
        let ticket = crate::relay::Ticket::new();
        let relay = crate::relay::RelayServer::start(config.local_port, ticket.clone())?;
        let tunnel = BoreTunnel::open(&config.server, relay.listen_port)?;
        let expires = now() + 1800;
        let mut invite = Invite {
            version: 1,
            room_id: random_base32(13),
            session_id: random_base32(7),
            expires,
            relay: Some(tunnel.endpoint()),
            host_name: config.host_name,
            host_code: config.host_code,
            world: config.world,
            ticket: ticket.id_hex(),
            token: ticket.token_hex(),
            ..Default::default()
        };
        let code = make_long(&invite);
        invite.raw = code.clone();
        Ok(ShareSession {
            tunnel,
            relay,
            invite,
            code,
        })
    }

    pub fn address(&self) -> String {
        format!("{}:{}", self.tunnel.server, self.tunnel.public_port)
    }
}

// ---------------------------------------------------------------------------
// Small encoders
// ---------------------------------------------------------------------------

pub fn base64url_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[n as usize & 63] as char);
        }
    }
    out
}

pub fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    fn value(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'-' | b'+' => Some(62),
            b'_' | b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for c in input.bytes() {
        if c == b'=' {
            break;
        }
        let v = value(c)?;
        buffer = (buffer << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

pub fn urlencode(value: &str) -> String {
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

pub fn urldecode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&value[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn random_base32(len: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut seed = now() as u64 ^ ((std::process::id() as u64) << 32);
    let mut out = String::with_capacity(len);
    for _ in 0..len {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        out.push(ALPHABET[(seed >> 33) as usize % ALPHABET.len()] as char);
    }
    out
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Bind a local TCP listener that forwards to `target_port`; used to give the
/// tunnel something to talk to when no Minecraft world is open yet.
pub fn local_echo() -> Result<(u16, Arc<AtomicU16>)> {
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let counter = Arc::new(AtomicU16::new(0));
    let worker = counter.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            worker.fetch_add(1, Ordering::Relaxed);
            let mut stream = stream;
            let _ = stream.write_all(b"CookieLauncher relay\n");
        }
    });
    Ok((port, counter))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_code_roundtrip() {
        let expires = 1_800_000_000u64;
        let code = make_short("ABCDEFGHIJKLM", "ABCDEFG", expires, "HOST01").unwrap();
        let invite = parse(&code).unwrap();
        assert_eq!(invite.room_id, "ABCDEFGHIJKLM");
        assert_eq!(invite.session_id, "ABCDEFG");
        assert_eq!(invite.expires, expires);
        assert_eq!(invite.host_code, "HOST01");
    }

    #[test]
    fn long_code_roundtrip() {
        let invite = Invite {
            version: 1,
            room_id: "ABCDEFGHIJKLM".into(),
            session_id: "ABCDEFG".into(),
            expires: now() + 600,
            relay: Some(RelayEndpoint {
                host: "bore.pub".into(),
                port: 54321,
                tls: false,
            }),
            host_name: "Emre".into(),
            host_code: "HOST01".into(),
            world: "Dünya".into(),
            ..Default::default()
        };
        let code = make_long(&invite);
        let parsed = parse(&code).unwrap();
        assert_eq!(parsed.relay.unwrap().port, 54321);
        assert_eq!(parsed.host_name, "Emre");
        assert_eq!(parsed.world, "Dünya");
    }

    #[test]
    fn transport_links_are_normalized() {
        let code = make_short("ABCDEFGHIJKLM", "ABCDEFG", now() + 600, "").unwrap();
        let link = format!("cl://join/{code}");
        assert_eq!(parse(&link).unwrap().room_id, "ABCDEFGHIJKLM");
        let web = format!("https://cookie-launcher.github.io/join/?c={code}");
        assert_eq!(parse(&web).unwrap().session_id, "ABCDEFG");
    }
}
