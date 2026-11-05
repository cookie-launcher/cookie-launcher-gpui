//! Cookie Launcher — native GPUI client.
//!
//! The UI is Rust; downloads, installs and the game process stay in the Python
//! core, which this binary supervises the same way the Tauri build did.

mod api;
mod assets;
mod app;
mod core;
mod launch;
mod relay;
mod social;
mod theme;
mod widgets;

use gpui_kit::*;

use crate::app::AppView;

fn main() {
    // `--social-probe` prints the LAN social snapshot and exits; it lets the
    // discovery protocol be verified without a window.
    // `--launch-probe <version>` runs the whole prepare + JVM pipeline and
    // reports whether the process reached the game loop.
    if let Some(version) = std::env::args()
        .skip_while(|arg| arg != "--launch-probe")
        .nth(1)
    {
        let core = core::Core::new();
        let options = launch::LaunchOptions {
            version: &version,
            instance_id: "probe",
            username: "Probe",
            ram_gb: 2,
            quick_play: None,
            cookie_optimized: true,
            aikar: true,
        };
        let core_arc = core.clone();
        let prepared = core_arc.prepare(&version);
        match prepared {
            Ok(prepared) => {
                println!("PREPARED mainClass={}", prepared.main_class);
                println!("classpath entries: {}", prepared.classpath.split(':').count());
                println!("assets: {}", prepared.asset_index);
                match core_arc.launch_now(options) {
                    Ok(()) => {
                        std::thread::sleep(std::time::Duration::from_secs(25));
                        let status = core_arc.status();
                        println!("STATUS running={} pid={:?}", status.is_running, status.minecraft_pid);
                        for line in core_arc.logs().iter().rev().take(12).rev() {
                            println!("LOG {line}");
                        }
                        core_arc.stop();
                    }
                    Err(error) => println!("LAUNCH FAILED: {error}"),
                }
            }
            Err(error) => println!("PREPARE FAILED: {error}"),
        }
        return;
    }

    // `--relay-probe` exercises the whole friend path locally: a relay guards
    // an echo server, bore exposes it, and the guest proxy authenticates and
    // receives the payload.
    if std::env::args().any(|arg| arg == "--relay-probe") {
        use std::io::{Read, Write};
        let core = core::Core::new();
        let (echo_port, _hits) = social::local_echo().expect("echo");
        println!("echo on {echo_port}");
        let code = match core.share_world(echo_port, "Probe Dünyası") {
            Ok(code) => {
                println!("share code: {} (len {})", &code[..code.len().min(48)], code.len());
                code
            }
            Err(error) => {
                println!("SHARE FAILED: {error}");
                return;
            }
        };
        std::thread::sleep(std::time::Duration::from_secs(2));
        match core.join_invite(&code) {
            Ok(address) => {
                println!("guest proxy at {address}");
                std::thread::sleep(std::time::Duration::from_secs(1));
                match std::net::TcpStream::connect(&address) {
                    Ok(mut stream) => {
                        stream.set_read_timeout(Some(std::time::Duration::from_secs(15))).ok();
                        let mut buffer = [0u8; 128];
                        match stream.read(&mut buffer) {
                            Ok(n) if n > 0 => println!(
                                "JOIN OK: {}",
                                String::from_utf8_lossy(&buffer[..n]).trim()
                            ),
                            _ => println!("JOIN EMPTY"),
                        }
                        let _ = stream.write_all(b"ping\n");
                    }
                    Err(error) => println!("JOIN CONNECT FAILED: {error}"),
                }
                core.stop_sharing();
            }
            Err(error) => println!("JOIN FAILED: {error}"),
        }
        return;
    }

    if let Some(mc) = std::env::args()
        .skip_while(|arg| arg != "--loader-probe")
        .nth(1)
    {
        let loader = std::env::args()
            .skip_while(|arg| arg != "--loader-probe")
            .nth(2)
            .unwrap_or_else(|| "fabric".to_string());
        let core = core::Core::new();
        let instance = api::Instance {
            id: "probe".to_string(),
            name: "Probe".to_string(),
            version: mc.clone(),
            loader: loader.clone(),
            ..Default::default()
        };
        match core.effective_version(&instance) {
            Ok(version) => {
                println!("effective version: {version}");
                match core.prepare(&version) {
                    Ok(prepared) => {
                        println!("mainClass: {}", prepared.main_class);
                        println!("classpath entries: {}", prepared.classpath.split(':').count());
                        println!("asset index: {}", prepared.asset_index);
                    }
                    Err(error) => println!("PREPARE FAILED: {error}"),
                }
            }
            Err(error) => println!("RESOLVE FAILED: {error}"),
        }
        return;
    }

    // `--search-probe <mc> <loader> <query>`: verify the Modrinth facets.
    // `--install-probe <slug> [mod|shader|resourcepack]` downloads a project
    // and its dependencies into a throwaway profile.
    // `--seed-profile <name> <slug>...` creates a profile and installs the
    // given projects into it, so the UI filter can be inspected with real data.
    if let Some(name) = std::env::args()
        .skip_while(|arg| arg != "--seed-profile")
        .nth(1)
    {
        let mut slugs: Vec<String> = Vec::new();
        let mut collecting = false;
        for arg in std::env::args() {
            if collecting {
                slugs.push(arg);
            } else if arg == "--seed-profile" {
                collecting = true;
            }
        }
        let core = core::Core::new();
        let instance = core
            .create_instance(&name, "1.21.4", "fabric", "")
            .expect("profile");
        for slug in slugs.iter().skip(1) {
            match core.install(slug, &instance.id, "mod", true) {
                Ok(files) => println!("installed {slug}: {}", files.len()),
                Err(error) => println!("failed {slug}: {error}"),
            }
        }
        println!("profile id: {}", instance.id);
        return;
    }

    if std::env::args().any(|arg| arg == "--install-probe") {
        let args: Vec<String> = std::env::args().collect();
        let pos = args.iter().position(|a| a == "--install-probe").unwrap_or(0);
        let slug = args.get(pos + 1).cloned().unwrap_or_default();
        let kind = args.get(pos + 2).cloned().unwrap_or_else(|| "mod".to_string());
        let core = core::Core::new();
        let instance = core
            .create_instance("Install Probe", "1.21.4", "fabric", "")
            .expect("profile");
        match core.install(&slug, &instance.id, &kind, true) {
            Ok(files) => {
                let mods = core.content(&instance.id, "mod");
                println!("INSTALLED {slug}: {} dosya, {} mod", files.len(), mods.len());
                for file in mods.iter().take(6) {
                    println!("  {} enabled={}", file.name, file.enabled);
                }
        let _ = core.delete_instance(&instance.id);
            }
            Err(error) => println!("INSTALL FAILED: {error}"),
        }
        return;
    }

    // `--lan-share-probe` proves the whole chain: discover a LAN world, share
    // that exact port through bore, join it back and read the payload.
    if std::env::args().any(|arg| arg == "--lan-share-probe") {
        use std::io::{Read, Write};
        let core = core::Core::new();
        let (echo_port, _) = social::local_echo().expect("echo");
        println!("sahte dünya sunucusu: 127.0.0.1:{echo_port}");

        // Start the scanner before broadcasting, exactly like the app does at
        // boot; otherwise the packet arrives before anything is listening.
        let _ = core.lan_worlds();
        let udp = std::net::UdpSocket::bind("127.0.0.1:0").expect("udp");
        let packet = format!("[MOTD]Probe Dünyası[/MOTD][AD]{echo_port}[/AD]");
        let _ = udp.send_to(packet.as_bytes(), ("127.0.0.1", 4445));
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let worlds = core.lan_worlds();
        println!("keşfedilen dünya: {} adet", worlds.len());
        let Some(world) = worlds.first().cloned() else {
            println!("LAN DÜNYASI BULUNAMADI");
            return;
        };
        println!("hedef port: {}", world.port);

        match core.share_world(world.port, &world.motd) {
            Ok(code) => {
                println!("kod üretildi (len {})", code.len());
                std::thread::sleep(std::time::Duration::from_secs(2));
                match core.join_invite(&code) {
                    Ok(address) => {
                        std::thread::sleep(std::time::Duration::from_millis(800));
                        match std::net::TcpStream::connect(&address) {
                            Ok(mut stream) => {
                                stream
                                    .set_read_timeout(Some(std::time::Duration::from_secs(15)))
                                    .ok();
                                let mut buffer = [0u8; 128];
                                match stream.read(&mut buffer) {
                                    Ok(n) if n > 0 => println!(
                                        "LAN PAYLAŞIMI OK: {}",
                                        String::from_utf8_lossy(&buffer[..n]).trim()
                                    ),
                                    _ => println!("BOŞ YANIT"),
                                }
                                let _ = stream.write_all(b"ping\n");
                            }
                            Err(error) => println!("BAĞLANTI HATASI: {error}"),
                        }
                    }
                    Err(error) => println!("KATILMA HATASI: {error}"),
                }
                core.stop_sharing();
            }
            Err(error) => println!("PAYLAŞIM HATASI: {error}"),
        }
        return;
    }

    // `--lan-beacon` keeps broadcasting a fake LAN world for a minute, so the
    // UI can be inspected with a world present.
    if std::env::args().any(|arg| arg == "--lan-beacon") {
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("udp");
        let packet = b"[MOTD]Arkada\xc5\x9f\xc4\xb1n D\xc3\xbcnyas\xc4\xb1[/MOTD][AD]50123[/AD]";
        for _ in 0..60 {
            let _ = socket.send_to(packet, ("127.0.0.1", 4445));
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        return;
    }

    // `--lan-probe` broadcasts a fake Minecraft LAN packet and reports whether
    // the scanner picks it up (same packet the game sends on UDP 4445).
    if std::env::args().any(|arg| arg == "--lan-probe") {
        let core = core::Core::new();
        core.ensure_offline_properties();
        println!("ilk tarama: {} dünya", core.lan_worlds().len());
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("udp");
        let packet = b"[MOTD]Probe D\xc3\xbcnyas\xc4\xb1[/MOTD][AD]51234[/AD]";
        let _ = socket.send_to(packet, ("127.0.0.1", 4445));
        std::thread::sleep(std::time::Duration::from_millis(1200));
        let worlds = core.lan_worlds();
        println!("duyuru sonrası: {} dünya", worlds.len());
        for world in &worlds {
            println!("  {} @ {}:{}", world.motd, world.address, world.port);
        }
        return;
    }

    if let Some(slug) = std::env::args()
        .skip_while(|arg| arg != "--modpack-probe")
        .nth(1)
    {
        let core = core::Core::new();
        match core.install_modpack(&slug) {
            Ok(id) => {
                let instance = core
                    .instances()
                    .into_iter()
                    .find(|i| i.id == id)
                    .unwrap_or_default();
                println!(
                    "INSTALLED {} -> {} (MC {}, {} {}) mods={}",
                    slug, instance.name, instance.version, instance.loader,
                    instance.loader_version, instance.mod_count
                );
                for line in core.logs().iter().rev().take(6).rev() {
                    println!("LOG {line}");
                }
            }
            Err(error) => println!("MODPACK FAILED: {error}"),
        }
        return;
    }

    if std::env::args().any(|arg| arg == "--search-probe") {
        let args: Vec<String> = std::env::args().collect();
        let pos = args.iter().position(|a| a == "--search-probe").unwrap_or(0);
        let mc = args.get(pos + 1).cloned().unwrap_or_default();
        let loader = args.get(pos + 2).cloned().unwrap_or_default();
        let query = args.get(pos + 3).cloned().unwrap_or_default();
        let core = core::Core::new();
        match core.search(&query, "mod", "downloads", 6, &mc, &loader) {
            Ok(hits) => {
                println!("query='{query}' mc={mc} loader={loader}: {} hits", hits.len());
                for hit in hits.iter().take(6) {
                    println!("  {} ({} indirme)", hit.title, hit.downloads);
                }
            }
            Err(error) => println!("SEARCH FAILED: {error}"),
        }
        return;
    }

    if std::env::args().any(|arg| arg == "--versions-probe") {
        let core = core::Core::new();
        match core.versions() {
            Ok(versions) => {
                println!("versions: {}", versions.len());
                for version in versions.iter().take(3) {
                    println!("  {} {} installed={}", version.id, version.kind, version.is_installed);
                }
            }
            Err(error) => println!("FAILED: {error}"),
        }
        return;
    }

    // `--bore-probe` opens a real bore.pub tunnel to a local echo server and
    // checks that a public connection reaches it.
    if std::env::args().any(|arg| arg == "--bore-probe") {
        use std::io::{Read, Write};
        let (port, _hits) = social::local_echo().expect("echo listener");
        println!("local echo on {port}");
        match social::BoreTunnel::open("bore.pub", port) {
            Ok(tunnel) => {
                println!("public port: {}", tunnel.public_port);
                std::thread::sleep(std::time::Duration::from_secs(2));
                let target = format!("bore.pub:{}", tunnel.public_port);
                match std::net::TcpStream::connect(&target) {
                    Ok(mut stream) => {
                        stream
                            .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                            .ok();
                        let mut buf = [0u8; 128];
                        match stream.read(&mut buf) {
                            Ok(n) if n > 0 => {
                                println!("TUNNEL OK: {}", String::from_utf8_lossy(&buf[..n]).trim())
                            }
                            _ => println!("TUNNEL EMPTY"),
                        }
                        let _ = stream.write_all(b"ping\n");
                    }
                    Err(error) => println!("CONNECT FAILED: {error}"),
                }
                tunnel.close();
            }
            Err(error) => println!("TUNNEL FAILED: {error}"),
        }
        return;
    }

    // `--social-peer` beacons as another player without opening a window.
    if std::env::args().any(|arg| arg == "--social-peer") {
        let core = core::Core::new();
        let _ = core.social_state();
        std::thread::sleep(std::time::Duration::from_secs(600));
        return;
    }

    if std::env::args().any(|arg| arg == "--social-probe") {
        let core = core::Core::new();
        let _ = core.social_state();
        std::thread::sleep(std::time::Duration::from_secs(6));
        let state = core.social_state().unwrap_or_default();
        println!("{}", serde_json::to_string_pretty(&state).unwrap_or_default());
        return;
    }

    application()
        .with_assets(assets::AppAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            theme::install(cx);
            app::init_keybindings(cx);

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1160.), px(780.)),
                    cx,
                ))),
                window_min_size: Some(size(px(920.), px(640.))),
                titlebar: None,
                kind: WindowKind::Normal,
                is_resizable: true,
                window_decorations: Some(WindowDecorations::Client),
                ..Default::default()
            };

            open_window(options, cx, |window, cx| {
                cx.new(|cx| AppView::new(window, cx))
            })
            .expect("CookieLauncher penceresi açılamadı");
        });
}
