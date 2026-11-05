//! CLT1 — the binary HMAC challenge/response that guards the relay tunnel,
//! ported from `backend/relay_handshake.py`.
//!
//! Wire format (big-endian, no padding):
//! ```text
//! CLIENT  -> SERVER : "CLT1" u8 version u8 type=1 tid[8]
//! SERVER  -> CLIENT : "CLT1" u8 version u8 type=2 tid[8] u32 expires nonce[16]
//! CLIENT  -> SERVER : "CLT1" u8 version u8 type=3 tid[8] u8 role nonce_c[16] mac[32]
//! SERVER  -> CLIENT : "CLT1" u8 version u8 type=4 u8 status
//! SERVER  -> CLIENT : "CLT1" u8 version u8 type=5   (both sides ready)
//! ```
//! The tunnel token never crosses the wire; only the HMAC of a one-shot nonce
//! does, and no Minecraft byte moves until authentication finishes.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub const MAGIC: [u8; 4] = *b"CLT1";
pub const VERSION: u8 = 1;

pub const CLIENT_HELLO: u8 = 1;
pub const SERVER_HELLO: u8 = 2;
pub const CLIENT_AUTH: u8 = 3;
pub const RESULT: u8 = 4;
pub const READY: u8 = 5;

pub const ROLE_HOST: u8 = 0;
pub const ROLE_GUEST: u8 = 1;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
const READY_TIMEOUT: Duration = Duration::from_secs(20);

pub const HELLO_LEN: usize = 4 + 1 + 1 + 8;
pub const SERVER_HELLO_LEN: usize = 4 + 1 + 1 + 8 + 4 + 16;
pub const AUTH_LEN: usize = 4 + 1 + 1 + 8 + 1 + 16 + 32;
pub const RESULT_LEN: usize = 4 + 1 + 1 + 1;
pub const READY_LEN: usize = 4 + 1 + 1;

#[derive(Clone, Debug)]
pub struct Ticket {
    pub id: [u8; 8],
    pub token: [u8; 32],
    pub created: u64,
    pub used_host: bool,
    pub used_guest: bool,
}

impl Ticket {
    pub fn new() -> Ticket {
        let mut id = [0u8; 8];
        let mut token = [0u8; 32];
        fill_random(&mut id);
        fill_random(&mut token);
        Ticket {
            id,
            token,
            created: now(),
            used_host: false,
            used_guest: false,
        }
    }

    pub fn id_hex(&self) -> String {
        hex(&self.id)
    }

    pub fn token_hex(&self) -> String {
        hex(&self.token)
    }

    pub fn from_hex(id: &str, token: &str) -> Result<Ticket> {
        let id = unhex(id).ok_or_else(|| anyhow!("ticket kimliği geçersiz"))?;
        let token = unhex(token).ok_or_else(|| anyhow!("ticket anahtarı geçersiz"))?;
        if id.len() != 8 || token.len() != 32 {
            return Err(anyhow!("ticket uzunluğu geçersiz"));
        }
        let mut id_bytes = [0u8; 8];
        let mut token_bytes = [0u8; 32];
        id_bytes.copy_from_slice(&id);
        token_bytes.copy_from_slice(&token);
        Ok(Ticket {
            id: id_bytes,
            token: token_bytes,
            created: now(),
            used_host: false,
            used_guest: false,
        })
    }
}

fn mac(token: &[u8; 32], tid: &[u8; 8], role: u8, server_nonce: &[u8; 16], client_nonce: &[u8; 16]) -> [u8; 32] {
    let mut message = Vec::with_capacity(4 + 2 + 8 + 1 + 16 + 16);
    message.extend_from_slice(&MAGIC);
    message.push(VERSION);
    message.push(CLIENT_AUTH);
    message.extend_from_slice(tid);
    message.push(role);
    message.extend_from_slice(server_nonce);
    message.extend_from_slice(client_nonce);

    let mut hasher = Hmac::<Sha256>::new_from_slice(token).expect("hmac key");
    hasher.update(&message);
    let out = hasher.finalize().into_bytes();
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&out);
    digest
}

fn fill_random(buffer: &mut [u8]) {
    // No rand crate: hash a monotonic counter with the process id.
    use sha2::Digest;
    let mut counter = 0u64;
    let mut filled = 0;
    while filled < buffer.len() {
        let mut hasher = Sha256::new();
        hasher.update(now().to_be_bytes());
        hasher.update(std::process::id().to_be_bytes());
        hasher.update(counter.to_be_bytes());
        let digest = hasher.finalize();
        let take = (buffer.len() - filled).min(digest.len());
        buffer[filled..filled + take].copy_from_slice(&digest[..take]);
        filled += take;
        counter += 1;
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

fn read_exact(stream: &mut TcpStream, len: usize) -> Result<Vec<u8>> {
    let mut buffer = vec![0u8; len];
    stream
        .read_exact(&mut buffer)
        .map_err(|_| anyhow!("handshake: bağlantı kapandı"))?;
    Ok(buffer)
}

/// Client side of CLT1 (host or guest).
pub fn client_handshake(
    stream: &mut TcpStream,
    ticket: &Ticket,
    role: u8,
) -> Result<()> {
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;

    let mut hello = Vec::with_capacity(HELLO_LEN);
    hello.extend_from_slice(&MAGIC);
    hello.push(VERSION);
    hello.push(CLIENT_HELLO);
    hello.extend_from_slice(&ticket.id);
    stream.write_all(&hello)?;
    stream.flush()?;

    let reply = read_exact(stream, SERVER_HELLO_LEN)?;
    if reply[0..4] != MAGIC || reply[4] != VERSION || reply[5] != SERVER_HELLO {
        return Err(anyhow!("handshake: sunucu yanıtı geçersiz"));
    }
    if reply[6..14] != ticket.id {
        return Err(anyhow!("handshake: ticket uyuşmadı"));
    }
    let expires = u32::from_be_bytes([reply[14], reply[15], reply[16], reply[17]]) as u64;
    if expires != 0 && expires < now() {
        return Err(anyhow!("handshake: ticket süresi doldu"));
    }
    let mut server_nonce = [0u8; 16];
    server_nonce.copy_from_slice(&reply[18..34]);

    let mut client_nonce = [0u8; 16];
    fill_random(&mut client_nonce);
    let digest = mac(&ticket.token, &ticket.id, role, &server_nonce, &client_nonce);

    let mut auth = Vec::with_capacity(AUTH_LEN);
    auth.extend_from_slice(&MAGIC);
    auth.push(VERSION);
    auth.push(CLIENT_AUTH);
    auth.extend_from_slice(&ticket.id);
    auth.push(role);
    auth.extend_from_slice(&client_nonce);
    auth.extend_from_slice(&digest);
    stream.write_all(&auth)?;
    stream.flush()?;

    let result = read_exact(stream, RESULT_LEN)?;
    if result[0..4] != MAGIC || result[4] != VERSION || result[5] != RESULT || result[6] != 0 {
        return Err(anyhow!("handshake: kimlik doğrulama reddedildi"));
    }

    stream.set_read_timeout(Some(READY_TIMEOUT))?;
    let ready = read_exact(stream, READY_LEN)?;
    if ready[0..4] != MAGIC || ready[4] != VERSION || ready[5] != READY {
        return Err(anyhow!("handshake: eşleşme onayı gelmedi"));
    }
    stream.set_read_timeout(None)?;
    Ok(())
}

/// Server side of CLT1: authenticates one connection against `ticket`.
pub fn server_handshake(stream: &mut TcpStream, ticket: &Arc<Mutex<Ticket>>) -> Result<u8> {
    stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT))?;
    stream.set_write_timeout(Some(HANDSHAKE_TIMEOUT))?;

    let hello = read_exact(stream, HELLO_LEN)?;
    if hello[0..4] != MAGIC || hello[4] != VERSION || hello[5] != CLIENT_HELLO {
        return Err(anyhow!("handshake: hello geçersiz"));
    }
    let mut tid = [0u8; 8];
    tid.copy_from_slice(&hello[6..14]);

    let (token, expires, host_used, guest_used) = {
        let ticket = ticket.lock().map_err(|_| anyhow!("kilit"))?;
        if ticket.id != tid {
            return Err(anyhow!("handshake: bilinmeyen ticket"));
        }
        (
            ticket.token,
            ticket.created + 90,
            ticket.used_host,
            ticket.used_guest,
        )
    };
    if expires < now() {
        return Err(anyhow!("handshake: ticket süresi doldu"));
    }

    let mut nonce = [0u8; 16];
    fill_random(&mut nonce);
    let mut reply = Vec::with_capacity(SERVER_HELLO_LEN);
    reply.extend_from_slice(&MAGIC);
    reply.push(VERSION);
    reply.push(SERVER_HELLO);
    reply.extend_from_slice(&tid);
    reply.extend_from_slice(&(expires as u32).to_be_bytes());
    reply.extend_from_slice(&nonce);
    stream.write_all(&reply)?;
    stream.flush()?;

    let auth = read_exact(stream, AUTH_LEN)?;
    if auth[0..4] != MAGIC || auth[4] != VERSION || auth[5] != CLIENT_AUTH || auth[6..14] != tid {
        return Err(anyhow!("handshake: auth geçersiz"));
    }
    let role = auth[14];
    if role != ROLE_HOST && role != ROLE_GUEST {
        return Err(anyhow!("handshake: rol geçersiz"));
    }
    let mut client_nonce = [0u8; 16];
    client_nonce.copy_from_slice(&auth[15..31]);
    let mut got = [0u8; 32];
    got.copy_from_slice(&auth[31..63]);

    let expected = mac(&token, &tid, role, &nonce, &client_nonce);
    if !constant_time_eq(&got, &expected) {
        return Err(anyhow!("handshake: kimlik doğrulama reddedildi"));
    }

    {
        let mut ticket = ticket.lock().map_err(|_| anyhow!("kilit"))?;
        let used = if role == ROLE_HOST { host_used } else { guest_used };
        if used {
            return Err(anyhow!("handshake: bu rol daha önce kullanıldı"));
        }
        if role == ROLE_HOST {
            ticket.used_host = true;
        } else {
            ticket.used_guest = true;
        }
    }

    let mut result = Vec::with_capacity(RESULT_LEN);
    result.extend_from_slice(&MAGIC);
    result.push(VERSION);
    result.push(RESULT);
    result.push(0);
    stream.write_all(&result)?;
    stream.flush()?;

    // Both roles authenticated: tell the client it may start speaking.
    let mut ready = Vec::with_capacity(READY_LEN);
    ready.extend_from_slice(&MAGIC);
    ready.push(VERSION);
    ready.push(READY);
    stream.write_all(&ready)?;
    stream.flush()?;
    stream.set_read_timeout(None)?;
    Ok(role)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Host side: listens for the guest, authenticates it, then bridges the
/// connection to the local Minecraft port.
pub struct RelayServer {
    pub listen_port: u16,
    pub ticket: Arc<Mutex<Ticket>>,
    pub connected: Arc<Mutex<bool>>,
}

impl RelayServer {
    pub fn start(target_port: u16, ticket: Ticket) -> Result<RelayServer> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let listen_port = listener.local_addr()?.port();
        let ticket = Arc::new(Mutex::new(ticket));
        let connected = Arc::new(Mutex::new(false));

        let shared_ticket = ticket.clone();
        let shared_state = connected.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let ticket = shared_ticket.clone();
                let state = shared_state.clone();
                std::thread::spawn(move || {
                    let mut guest = stream;
                    if server_handshake(&mut guest, &ticket).is_err() {
                        return;
                    }
                    let Ok(mut target) = TcpStream::connect(("127.0.0.1", target_port)) else {
                        return;
                    };
                    if let Ok(mut flag) = state.lock() {
                        *flag = true;
                    }
                    bridge(&mut guest, &mut target);
                });
            }
        });

        Ok(RelayServer {
            listen_port,
            ticket,
            connected,
        })
    }

    pub fn guest_connected(&self) -> bool {
        self.connected.lock().map(|flag| *flag).unwrap_or(false)
    }
}

/// Guest side: a local proxy that MC connects to; it dials the public relay,
/// authenticates, then splices.
pub struct LocalProxy {
    pub local_port: u16,
}

impl LocalProxy {
    pub fn start(relay_host: String, relay_port: u16, ticket: Ticket) -> Result<LocalProxy> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let local_port = listener.local_addr()?.port();
        let ticket = Arc::new(ticket);

        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let ticket = ticket.clone();
                let host = relay_host.clone();
                std::thread::spawn(move || {
                    let Ok(mut remote) = TcpStream::connect((host.as_str(), relay_port)) else {
                        return;
                    };
                    if client_handshake(&mut remote, &ticket, ROLE_GUEST).is_err() {
                        return;
                    }
                    let mut local = stream;
                    bridge(&mut local, &mut remote);
                });
            }
        });

        Ok(LocalProxy { local_port })
    }
}

fn bridge(a: &mut TcpStream, b: &mut TcpStream) {
    let Ok(mut a_read) = a.try_clone() else {
        return;
    };
    let Ok(mut b_write) = b.try_clone() else {
        return;
    };
    let upstream = std::thread::spawn(move || {
        let _ = std::io::copy(&mut a_read, &mut b_write);
        let _ = b_write.shutdown(std::net::Shutdown::Write);
    });
    let _ = std::io::copy(b, a);
    let _ = b.shutdown(std::net::Shutdown::Write);
    let _ = upstream.join();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mac_is_deterministic_and_role_bound() {
        let ticket = Ticket::new();
        let nonce_s = [1u8; 16];
        let nonce_c = [2u8; 16];
        let host = mac(&ticket.token, &ticket.id, ROLE_HOST, &nonce_s, &nonce_c);
        let guest = mac(&ticket.token, &ticket.id, ROLE_GUEST, &nonce_s, &nonce_c);
        assert_eq!(host, mac(&ticket.token, &ticket.id, ROLE_HOST, &nonce_s, &nonce_c));
        assert_ne!(host, guest);
    }

    #[test]
    fn hex_roundtrip() {
        let ticket = Ticket::new();
        let clone = Ticket::from_hex(&ticket.id_hex(), &ticket.token_hex()).expect("ticket");
        assert_eq!(ticket.id, clone.id);
        assert_eq!(ticket.token, clone.token);
    }

    #[test]
    fn handshake_accepts_guest_and_rejects_replay() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("listener");
        let port = listener.local_addr().expect("addr").port();
        let ticket = Arc::new(Mutex::new(Ticket::new()));
        let server_ticket = ticket.clone();

        let server = std::thread::spawn(move || {
            let mut roles = Vec::new();
            for stream in listener.incoming().take(2).flatten() {
                let mut stream = stream;
                roles.push(server_handshake(&mut stream, &server_ticket).is_ok());
            }
            roles
        });

        // Lock once: the guard in a struct literal would live to the end of the
        // statement and the second `lock()` would deadlock.
        let (id, token) = {
            let guard = ticket.lock().expect("lock");
            (guard.id, guard.token)
        };
        let client_ticket = Ticket {
            id,
            token,
            created: now(),
            used_host: false,
            used_guest: false,
        };
        let mut first = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        assert!(client_handshake(&mut first, &client_ticket, ROLE_GUEST).is_ok());

        let mut second = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        assert!(client_handshake(&mut second, &client_ticket, ROLE_GUEST).is_err());

        let roles = server.join().expect("join");
        assert_eq!(roles, vec![true, false]);
    }
}
