# Cookie Launcher — GPUI (100% Rust)

`cookie-launcher-tauri`'nin **tamamen Rust** istemcisi. Webview yok, Python yok,
localhost HTTP yok: arayüz GPUI ile çizilir, çekirdek (indirme, kurulum,
başlatma, sosyal tünel) doğrudan Rust içinde çalışır.

```
cookie-launcher-gpui (tek binary)
├── arayüz      : GPUI + gpui-kit, Cocoa Craft tasarım sistemi
└── çekirdek    : src/core.rs · launch.rs · social.rs · relay.rs
                  └── Mojang / Modrinth / bore.pub
```

Eski Tauri/Python çekirdeği tamamen kaldırıldı; `backend.rs` (Python
süpervizörü) silindi, yerine Rust modülleri geldi.

## Çalıştırma

```bash
cargo run                 # normal
CL_TAB=modrinth cargo run # belirli sekmede aç
```

İlk derleme GPUI'den dolayı uzun (~3-4 dk), sonrası artımlı.

## Çekirdek (hepsi Rust)

| Alan | Nerede | Durum |
| --- | --- | --- |
| Profil CRUD (`instances/<id>/instance.json`) | `core.rs` | ✅ |
| Dünyalar, içerik listeleme/toggle/silme | `core.rs` | ✅ |
| Mojang sürüm manifesti (922 sürüm, disk cache) | `core.rs` | ✅ |
| Vanilla launch: jar + kütüphaneler + natives + assetler + JVM | `launch.rs` | ✅ doğrulandı |
| **Fabric / Quilt** profili (`inheritsFrom` birleştirme, ASM dedupe) | `launch.rs` | ✅ doğrulandı |
| **Forge / NeoForge** (resmi installer + `arguments` hattı) | `core.rs` + `launch.rs` | ✅ doğrulandı (1.20.1 Forge, 1.21.1 NeoForge) |
| Modrinth arama (sürüm + yükleyici facet'li) | `core.rs` | ✅ |
| Modrinth kurulum (jar + bağımlılık) | `core.rs` | ✅ |
| **Modpack kurulumu** (`.mrpack` → profil) | `core.rs` | ✅ doğrulandı (47 mod) |
| Ekran görüntüleri (gerçek thumbnail) | `core.rs` + UI | ✅ |
| Log halkası (1000 satır) + durum | `core.rs` | ✅ |
| **Arkadaşlar**: bore.pub tüneli + CLT1 HMAC el sıkışması | `social.rs` + `relay.rs` | ✅ doğrulandı |

### Arkadaşlar akışı

1. **LAN dünyası keşfi** → oyun UDP 4445'e `[MOTD]…[AD]port[/AD]` yayınlar; çekirdek
   bunu dinler ve Arkadaşlar sayfasında listeler (15 saniye TTL).
   `server.properties` her açılışta `online-mode=false` yapılır, böylece offline
   hesaplar LAN dünyasına girebilir.
2. **Paylaş** → seçilen LAN dünyasının **gerçek portu** yerel relay'e (CLT1
   korumalı) bağlanır, `bore.pub` tüneli açılır ve `CL1-…` kodu üretilir
   (relay adresi, ticket ve token kodun içinde)
2. **Katıl** → kod çözülür, yerel authenticated proxy açılır, oyun
   `--quickPlayMultiplayer 127.0.0.1:<port>` ile doğrudan arkadaşın dünyasına girer
3. Token kablodan geçmez: tek kullanımlık nonce'un HMAC-SHA256 yanıtı gider,
   yanlış token / replay reddedilir

## Tasarım sistemi

- `assets/theme/cocoa-craft.json` — 128 renk anahtarı, radius 5/8, gölge yok
- `assets/fonts/` — Plus Jakarta Sans + JetBrains Mono (statik 400–700) + Silkscreen
- `theme.rs` → `Cocoa`: temanın adı olmayan roller (karamel wash, cream `--paper`,
  chrome, launch gradyanı)
- Ölçüler kaynakla aynı: titlebar 44, rail 218, nav 44, buton 34, input 38,
  launch 196×52, radius 3/5/8/10

## Animasyonlar (CSS süreleriyle)

| Efekt | Süre | Nerede |
| --- | --- | --- |
| Sekme geçişi fade | 140 ms | tüm sekmeler |
| Nav / kart hover (bg+border) | 120 ms `ease` | rail, profil ve mod kartları |
| Toast girişi (fade + 10 px kayma) | 160 ms | bildirim |
| Skeleton shimmer | 1.4 s döngü | Modrinth yükleme |
| Meşgul durum nabzı | 1.6 s döngü | rail durum noktası |
| Launch butonu basışı (`translateY(2px)`) | 80 ms | karamel buton |

`widgets.rs` → `MotionExt` (fade_bg / fade_border / fade_card / shimmer / pulse /
press) ve `ease` = cubic-bezier(0.25, 0.1, 0.25, 1) yaklaşımı.

## Klavye

`Ctrl+1…6` sekmeler · `Ctrl+K` arama · `Ctrl+Enter` başlat · `?` kısayollar
penceresi. GPUI `actions!` + `cx.bind_keys` + kök `key_context("Launcher")`.

## Samimi olarak eksik olanlar

- **Profil detay ekranı** (`pds-*`): içerik/sürüm/dünya/log/not sekmeleri
- Modpack içeriğini sonradan güncelleme
- `Noise_XX` uçtan uca şifreleme (CLT1 kimlik doğrulama var, taşıma şifrelemesi yok)
- Uygulama ikonu / paketleme (AppImage, deb, .desktop)

## Testler

`cargo test` — 17 test, ağ gerektirmez:

| Test | Neyi doğrular |
| --- | --- |
| `creates_lists_and_deletes_a_profile` | profil CRUD, `mods/` oluşturma, aynı ismin reddi |
| `slugifies_names_and_avoids_collisions` | id üretimi, çakışma |
| `rejects_unsupported_loaders` | Forge/NeoForge reddi, Fabric kabulü |
| `lists_toggles_and_deletes_installed_content` | `.disabled` çevirme, silme, sayaçlar |
| `reads_worlds_and_screenshots_from_disk` | dünya boyutu/tarihi, sadece PNG listeleme |
| `effective_version_resolves_fabric_and_rejects_forge` | `fabric-loader-x-y` çözümü |
| `open_folder_rejects_escaping_paths` | `../` ve mutlak yol reddi |
| `reads_installed_modrinth_slugs` | profilde kurulu slug haritası (`.mrpack`/mod) |
| `parses_the_minecraft_lan_broadcast` | UDP 4445 duyuru paketi (`[MOTD]…[/MOTD][AD]port[/AD]`) |
| `forces_offline_mode_and_keeps_other_properties` | `online-mode=false`, diğer ayarlar korunur |
| `hide_downloaded_filters_only_installed_projects` | "İndirilenleri gizle" filtresi |
| `mac_is_deterministic_and_role_bound` | CLT1 MAC rol ayrımı |
| `hex_roundtrip` | ticket kodlaması |
| `handshake_accepts_guest_and_rejects_replay` | gerçek TCP el sıkışması + replay reddi |
| `short_code_roundtrip` / `long_code_roundtrip` / `transport_links_are_normalized` | CL1 kodek |

`open_folder` testi yazılırken gerçek bir açık bulundu: `mc/../etc` gibi bir yol
`Path::starts_with` kontrolünü geçiyordu. Artık mutlak yollar ve `..` bileşenleri
kaynakta reddediliyor.

### Canlı doğrulamalar (ağ gerekir)

Her biri gerçek uzak servislerle çalışır, çıktısı yukarıdaki test turunda alındı:

| Komut | Sonuç |
| --- | --- |
| `--versions-probe` | 922 Mojang sürümü |
| `--search-probe 1.7.10-pre1 vanilla` | 1.7.10 modları (Zume, SpeedRunIGT…) |
| `--search-probe 1.21.11 fabric` | Fabric API, Sodium |
| `--install-probe sodium mod` | `sodium-fabric-0.6.13+mc1.21.4.jar` kuruldu |
| `--modpack-probe fabulously-optimized` | 49 dosya, **47 mod** |
| `--forge-probe 1.20.1 forge` | installer çalışır, `1.20.1-forge-47.4.28` hazır (82 classpath) |
| `--forge-probe 1.21.1 neoforge` | `neoforge-21.1.99` hazır (93 classpath) |
| `--launch-probe 1.21.4` | oyun açıldı (`running=true`, 71 classpath) |
| `--relay-probe` | paylaş → katıl → `JOIN OK` |
| `--lan-probe` | UDP 4445'e atılan sahte duyuru keşfediliyor |
| `--lan-share-probe` | keşfedilen LAN portu paylaşılıp misafirle veri akıtılıyor |

## Geliştirme yardımcıları

```bash
cargo run -p xtask -- capture cikti.png      # pencereyi öne getirmeden yakalar
cargo run -p xtask -- tab modrinth cikti.png # sekmeyi seçip yakalar
cargo run -- --versions-probe       # 922 sürümü doğrular
cargo run -- --loader-probe 1.21.4 fabric
cargo run -- --launch-probe 1.21.4  # gerçek Minecraft başlatır
cargo run -- --modpack-probe fabulously-optimized
cargo run -- --relay-probe          # paylaş+katıl zincirini test eder
cargo run -- --install-probe sodium mod   # tek mod + bağımlılık kurulumu
```

Ekran görüntüleri `docs/` altında.
