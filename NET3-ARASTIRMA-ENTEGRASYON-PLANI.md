# net3 → evorift — Araştırma Entegrasyon Planı

_Derlendi: 2026-06-13_

Bu plan, `C:\Users\Evrim\Desktop\projects\net3\` altındaki **iki araştırma katmanını** evorift'e
(Rust + Tauri v2 + SvelteKit) nasıl ekleyeceğimizi faz faz, **kontrol noktaları** ve **beklenen
fonksiyonlar** ile derler:

1. **`net3/SOLUTION.md`** — Türkiye'de Discord (metin+ses) + Roblox'u açan, **canlı doğrulanmış**
   çalışan çözüm (winws DPI desync + Discord-only WARP split-tunnel).
2. **`net3/docs/00-07`** — GoodbyeDPI-Turkey + SplitWire-Turkey **kaynak kodu incelenerek** çıkarılan
   **ürün blueprint'i** (çok-motorlu mimari, Auto-Pilot, profiller, rollback log, onarım, preflight).

---

## 0. Durum tespiti — net3'ten neler ZATEN entegre

Plana başlamadan önce çift iş yapmamak için: net3'ün **çekirdeği büyük oranda inmiş durumda**.
Aşağıdakiler evorift'te _mevcut_ ve bu planın kapsamı **dışında** (sadece doğrulanır):

| net3 parçası | evorift'teki karşılığı | Durum |
|---|---|---|
| winws (zapret) sidecar + proven_args | `engine.rs` `WinwsEngine` + `args()` + Job object | ✅ İnmiş |
| EXE-relative bundle çözümü | `engine.rs::bundle_dir()` | ✅ İnmiş |
| WinDivert tek-sürüm + stale temizliği | `engine.rs::clear_stale_windivert()` + `kill_all()` | ✅ İnmiş |
| Discord-only WARP split-tunnel (`162.159/66.22/104.29`) | `warp.rs::ALLOWED_IPS` + `WarpEngine` | ✅ İnmiş (birebir) |
| Full-tunnel + tünel DNS pinleme (kill-switch) | `warp.rs::FULL_ALLOWED_IPS` + `inject_interface_dns()` + `WARP_DNS` | ✅ İnmiş |
| Loop-güvenli endpoint | `warp.rs::WARP_ENDPOINT` + test | ✅ İnmiş |
| Headless ayrıcalıklı servis (UI bozamaz) | `service.rs` + `bin/evorift-svc.rs` (LocalSystem) | ✅ İnmiş |
| **CPU bug fix** — PowerShell enum → native Win32 | `netinfo.rs` (`GetExtendedTcpTable`/`QueryFullProcessImageNameW`) | ✅ İnmiş (untracked) |
| domain-watch `document.hidden` gate + 20–60s cadence | `state.svelte.ts` `#startDomainWatch` | ✅ İnmiş |

> **Sonuç:** Plan, net3'ün **çalışan çözümünü tekrar yazmaz**. Plan, **`docs/06`+`07` blueprint'inin
> henüz koda dönüşmemiş kısımlarını** evorift'e eklemeye odaklanır: çok-motorluluk, Auto-Pilot,
> profiller, rollback log, onarım sihirbazı, preflight/teşhis.

---

## 1. Mimari eşleme — blueprint (C#/Avalonia) → evorift (Rust/Tauri)

`docs/07` greenfield bir C#/Avalonia uygulaması varsayar. evorift **aynı 3 katmanı zaten** Rust/Tauri
ile uygular; blueprint'in soyutlamalarını sıfırdan değil, **mevcut dosyaların üstüne** kuracağız:

| `07` blueprint katmanı | evorift'teki yer | Eklenecek |
|---|---|---|
| UI katmanı (Avalonia) | SvelteKit `src/` + `state.svelte.ts` | Basit/Uzman mod, Auto-Pilot ekranı, teşhis paneli |
| Orkestrasyon (Core service) | `service.rs` `dispatch()` + watchdog + telemetry | Profil yöneticisi, durum makinesi, rollback log, Auto-Pilot |
| Engine adaptörleri (`IBypassEngine`) | `engine.rs` `DpiEngine` trait | `trait BypassEngine` genelleştirme + ByeDPI/GoodbyeDPI/sing-box adaptörleri |
| IPC sözleşmesi | `ipc.rs` `Command`/`Response` + `validate()` | Yeni komutlar (profil, autopilot, preflight, repair) |

**Anahtar ilke (net3 §10 + `07` §1):** Tüm ayrıcalıklı iş `evorift-svc.exe`'de (LocalSystem) kalır; UI
yalnız IPC ile niyet bildirir. Her yeni özellik bu sınırı korumalı.

---

## 2. Faz faz yol haritası

Her faz: **Hedef → Beklenen fonksiyonlar (dosya:imza) → Kontrol noktaları**. Fazlar bağımsız
gönderilebilir; sıralama riske göredir (önce kararlılık, sonra zeka, en son genişleme).

---

### Faz 0 — Temel kararlılık & V0.1.3'ü kapat (önce bunu bitir)

**Hedef:** net3 çözümü + CPU/WARP düzeltmeleri commit'lenip 0.1.3 olarak çıksın. Yeni özellik yok;
zemini sağlamlaştır. (Detay: `V0.1.3-BUGFIX-PLAN.md`.)

**Beklenen fonksiyonlar / işler:**
- `netinfo.rs` — git'e ekle (şu an untracked); `cargo test` ile `sockets()`/`pid_exe_path()` doğrula.
- `engine.rs::clear_stale_windivert` — `sc` fırtınasını yalnız _gerçek_ (yeniden)başlatmada çalıştır
  (son uygulanan exclusion set'i cache'le).
- Ölü kod temizliği: `detect_anticheat()` + `AntiCheat` + `ANTICHEAT_PROCS` kaldır (varsa).
- `service.rs` watchdog — `MissedTickBehavior::Skip` / tek `std::thread` single-flight garantisi.
- CHANGELOG/README "Known Issues" notunu düzelt (net3 + V0.1.3 atfı).

**Kontrol noktaları:**
- [ ] `cargo check --target-dir tmp_check` temiz (çalışan `tauri dev`'i bozmadan).
- [ ] `svelte-check` → kendi dosyalarımızda 0 hata (BlackHole/three uyarıları hariç).
- [ ] 5 dk minimize çalışmada `powershell.exe`/`conhost.exe` üretimi **sıfır** (Process Explorer).
- [ ] Discord masaüstü (metin+ses) + Roblox + YouTube canlı doğrulandı.
- [ ] `npm run tauri build` (dev kapalı) → exe + MSI + portable ZIP + SHA256SUMS üretti.

---

### Faz 1 — Motor soyutlaması (çok-motorluluğun zemini)

**Hedef:** `docs/07 §3 IBypassEngine`'i evorift'e taşı. Bugün tek motor (winws) + WARP var; soyutlama
ileride GoodbyeDPI/ByeDPI/sing-box eklemeyi "yeni dosya" işine indirger. **Strateji = veri** ilkesi
(`07 §3`): parametreler string değil, tipli nesneler.

**Beklenen fonksiyonlar:**
- `engine.rs` — mevcut `DpiEngine` trait'ini genelleştir:
  ```rust
  pub enum EngineKind { Desync, LocalProxy, Tunnel }      // 07 §3
  pub struct EngineCaps { pub split_tunnel: bool, pub requires_windivert: bool, pub auto_scan: bool }
  pub trait BypassEngine {
      fn id(&self) -> &'static str;                        // "zapret" | "goodbyedpi" | "byedpi" | "warp"
      fn kind(&self) -> EngineKind;
      fn caps(&self) -> EngineCaps;
      fn build_args(&self, profile: &Profile) -> Vec<String>;   // UI'de canlı gösterilebilir komut satırı
      fn preflight(&self) -> PreflightResult;              // Faz 5
      fn start(&mut self, profile: &Profile) -> Result<(), String>;
      fn stop(&mut self);
      fn is_running(&self) -> bool;
  }
  ```
- `WinwsEngine` mevcut `args()`'ı `build_args(&Profile)` arkasına taşı (geriye-uyum: profil yoksa
  `proven_args` defaultu).
- `service.rs::EngineState` — `dpi: Box<dyn BypassEngine>` (zaten `Box<dyn DpiEngine>`; isim/şema
  genişletmesi).

**Kontrol noktaları:**
- [ ] winws hâlâ `BypassEngine` arkasından birebir aynı argümanlarla çalışıyor (regresyon yok).
- [ ] `build_args()` çıktısı UI'ye string olarak akıtılabiliyor (kopyalanabilir komut satırı — `06 §1.4`).
- [ ] Yeni motor eklemek için **sadece** yeni `impl BypassEngine` + `engines()` listesine satır gerekiyor.

---

### Faz 2 — Profil sistemi (özelleştirmenin kalbi)

**Hedef:** `docs/07 §4` + `06 §2.4`. Sabit `DEFAULT_HOSTLIST` yerine **adlandırılmış, içe/dışa
aktarılabilir profiller**: {motor, parametreler, hedef uygulamalar/alan adları, DNS}.

**Beklenen fonksiyonlar:**
- Yeni dosya `src-tauri/src/profile.rs`:
  ```rust
  pub struct Profile {
      pub schema_version: u32, pub id: String, pub name: String,
      pub engine: String,                       // "zapret" | ...
      pub scope: Scope,                         // { mode: System|Split, apps, browsers }
      pub dns: DnsCfg,                           // { enabled, v4, doh }
      pub engine_params: serde_json::Value,     // motora-özel tipli
      pub hostlist: Vec<String>,
  }
  pub fn load_all() -> Vec<Profile>;            // %PROGRAMDATA%\evorift\profiles\*.json
  pub fn save(p: &Profile) -> Result<(),String>;
  pub fn export(id: &str) -> Result<String,String>;   // paylaşılabilir JSON
  pub fn import(json: &str) -> Result<Profile,String>;
  ```
- `ipc.rs::Command` — `ListProfiles`, `SaveProfile{json}`, `ApplyProfile{id}`, `DeleteProfile{id}`
  + `validate()` kuralları (id `[a-z0-9-]`, hostlist ≤500, vb. — mevcut `SetHostlist` kalıbıyla).
- `service.rs::dispatch` — `ApplyProfile` → aktif profili kur (eskiyi rollback log üzerinden kaldır).
- `lib.rs` — `#[tauri::command]` köprüleri: `list_profiles`, `save_profile`, `apply_profile`.

**Kontrol noktaları:**
- [ ] Üç hazır profil tohumlu gelir: "Discord+Ses", "YouTube hızlı", "Her şey".
- [ ] Profil dışa→içe aktar round-trip kayıpsız (`cargo test`).
- [ ] Aktif profil değişiminde eski hizmet/firewall/DNS temiz kaldırılıyor (Faz 4 rollback ile).
- [ ] `%PROGRAMDATA%\evorift\profiles\` ACL'i sıkı (SYSTEM+Admin), `warp.conf` kalıbı gibi.

---

### Faz 3 — DNS/DoH yöneticisi + onarım sihirbazı

**Hedef:** `docs/05`. evorift'te `run_dns`/DoH/`run_repair` **kısmen var**; eksik olan **geri alma +
doğrulama + Discord onarımı**.

**Beklenen fonksiyonlar:**
- `service.rs` — mevcut `run_dns`/`run_repair`'i tamamla:
  ```rust
  fn reset_dns()  // Set-DnsClientServerAddress -ResetServerAddresses + DoH registry temizliği (05 §1)
  fn verify_dns() -> DnsVerify   // fiziksel adaptörlerde sonucu doğrula (05 §1 VerifyDNSSettings)
  ```
- Yeni dosya `src-tauri/src/repair.rs` (Discord onarımı — `05 §2`):
  ```rust
  fn find_discord_path() -> Option<PathBuf>;   // çalışan süreç → %LOCALAPPDATA%\Discord\app-*
  fn repair_discord() -> Result<(),String>;    // cache temizle + resmi indirici ile yeniden kur
  fn install_webcord() -> Result<(),String>;   // GitHub release zip → %LOCALAPPDATA%\evorift\WebCord
  ```
- `ipc.rs::Command` — `ResetDns`, `RepairDiscord`, `InstallWebcord` + `validate()`.
- UI: "Onarım" sayfası (Discord onar / WebCord kur / DNS sıfırla butonları).

**Kontrol noktaları:**
- [ ] DNS sıfırlama kullanıcının **önceki** ayarına döner, DHCP'ye değil (eski değer kaydedilmişse).
- [ ] `verify_dns()` UI'de "DNS güvenli mi?" rozetini besliyor.
- [ ] Discord onarımı resmi indirici URL'sini kullanıyor (`discord.com/api/downloads/...`).
- [ ] Tüm indirmeler SHA/HTTPS doğrulamalı; harici servise gönderim yok (yalnız resmi indirici).

---

### Faz 4 — Transaction / Rollback log (garantili geri alma)

**Hedef:** `docs/05 §4.2` + `07 §5`. Şu an temizlik ad-hoc; **yapılan her sistem değişikliği loglanıp**
ters sırada geri alınmalı (firewall, DNS, hizmet, registry, dosya).

**Beklenen fonksiyonlar:**
- Yeni dosya `src-tauri/src/rollback.rs`:
  ```rust
  pub enum Change {
      ServiceCreated(String), FirewallRule(String), RegistryWrite{path,name,old:Option<String>},
      DnsChanged{if_index:u32, old:Vec<String>}, FileCopied(PathBuf), TunnelInstalled(String),
  }
  pub struct RollbackLog { /* %PROGRAMDATA%\evorift\rollback.json */ }
  impl RollbackLog {
      pub fn record(&mut self, c: Change);
      pub fn rollback_all(&mut self) -> Result<(),String>;   // ters sıra, bağımlılık-farkında
  }
  ```
- `service.rs` — `run_block`/`run_dns`/`run_tweak`/`warp.start`/`engine.start` çağrılarını
  `log.record(...)` ile sarmala (DNS'te **eski değeri** önce oku-kaydet).
- Hizmet kaldırma sırası bağımlılık grafiğiyle (`05 §3`: WinDivert'i tüketenler önce — zapret/winws →
  WinDivert).

**Kontrol noktaları:**
- [ ] "Tamamen Temizle" tek tıkla: firewall + DNS + tünel + winws + registry → sistem ilk hâline.
- [ ] Servis crash sonrası açılışta yarım-kalan değişiklikler tespit + geri alınıyor.
- [ ] `cargo test` — kayıt→geri-al round-trip (mock sistem çağrılarıyla).
- [ ] WinDivert'i tüketen hizmet, WinDivert'ten **önce** kaldırılıyor (sıra testi).

---

### Faz 5 — Preflight & teşhis paneli

**Hedef:** `docs/07 §8` + `06 §2.5`. "Neden çalışmıyor?" sorusunu uygulama kendisi cevaplasın.

**Beklenen fonksiyonlar:**
- Yeni dosya `src-tauri/src/preflight.rs`:
  ```rust
  pub struct PreflightResult { pub ok: bool, pub checks: Vec<Check> }  // Check{name, pass, hint}
  pub fn run() -> PreflightResult;   // admin? · WinDivert AV-engelli mi? · WinDivert çakışma ·
                                     // WARP register çalışıyor mu? · DoH Win11 mi?
  pub fn windivert_conflict() -> Option<String>;   // başka süreç WinDivert.sys tutuyor mu (07 §8)
  ```
- `service.rs` — `connectivity_test`'i Auto-Pilot'a hazır hâle getir (zaten `lib.rs::connectivity_test`
  var; teşhiste yeniden kullan).
- `ipc.rs::Command` — `Preflight`, `Diagnose{targets:Vec<String>}` (DNS→TCP→TLS→HTTP zinciri + öneri).
- UI: teşhis paneli — her kontrol yeşil/kırmızı + **somut öneri** ("Kaspersky → WinDivert engelli →
  ByeDPI dene", "WARP register başarısız → desync kullan").

**Kontrol noktaları:**
- [ ] Yönetici değilken net açıklama + `relaunch_as_admin()` (mevcut) önerisi.
- [ ] WinDivert başka süreçte yüklüyse tespit + "çakışma temizle" önerisi.
- [ ] Teşhis bir hedef site için DNS/TCP/TLS/HTTP basamaklarını ayrı ayrı raporluyor.

---

### Faz 6 — Auto-Pilot (birleşik strateji bulucu)

**Hedef:** `docs/06 §2.2` + `07 §6`. "Beni Koru" → ISS tespit → DNS ayarla → hedef siteleri **her
strateji ile gerçek test et** → skor tablosu → en iyiyi uygula. Mevcut blockcheck yalnız tek motor;
Auto-Pilot tüm strategileri/motorları tarar.

**Beklenen fonksiyonlar:**
- Yeni dosya `src-tauri/src/autopilot.rs`:
  ```rust
  pub struct Candidate { pub engine: String, pub strategy: Strategy }
  pub struct ScoreRow { pub candidate: String, pub per_site: Vec<(String,bool,u32)>, pub score: i64 }
  pub fn detect_isp() -> Option<String>;          // ASN/whois (07 §6 adım 1)
  pub fn run(targets: &[String], depth: Depth, on_progress: impl Fn(ScoreRow))
      -> Vec<ScoreRow>;                           // RunOnce (hizmet kurmadan) → DNS/TCP/TLS/HTTP → skor
  ```
- Her aday için **hizmet kurmadan** `RunOnce`: `engine.build_args()` ile geçici winws başlat → test →
  durdur (kalıcı iz yok).
- `ipc.rs::Command` — `AutoPilot{targets, depth}` (Subscribe benzeri canlı ilerleme akışı).
- `service.rs` — canlı `ScoreRow` akışını `Response::Telemetry` benzeri yeni `Response::AutoPilot(row)`
  ile UI'ye stream et.
- UI: canlı skor çubuğu + "Uygula ve Hatırla" (kazanan profili Faz 2 profili olarak kaydet).

**Kontrol noktaları:**
- [ ] Test sırasında kalıcı hizmet/firewall/tünel **kurulmuyor** (RunOnce temiz).
- [ ] `depth=hızlı` ilk "tüm hedefler açık" adayında duruyor.
- [ ] Skor = açılan site × ağırlık − gecikme cezası; tablo UI'de canlı akıyor.
- [ ] Kazanan profil tek tıkla Faz 2 profiline kaydoluyor.

---

### Faz 7 — Yeni motorlar (ByeDPI, GoodbyeDPI) + görsel düzenleyici

**Hedef:** `docs/03`+`04`+`06 §2.1/2.3`. Soyutlama (Faz 1) hazır olunca motor ekle. Öncelik
**ByeDPI/ciadpi** — _kernel-siz_, WinDivert engelli (Kaspersky) makinelerde çalışan tek motor (`07 §3`).

**Beklenen fonksiyonlar:**
- `engine_byedpi.rs` — `impl BypassEngine`, `kind = LocalProxy`, `requires_windivert = false`;
  `build_args` = `--split 1 --disorder 3+s --mod-http=h,d --auto=torst --tlsrec 1+s` (`07 §3` birebir).
  ProxiFyre/drover ile split-tunnel (`03`).
- `engine_goodbyedpi.rs` — `impl BypassEngine`, `-5`/`-9` modları + DNS-redirect (`01`).
- Bundle: `resources/byedpi/`, `resources/goodbyedpi/` (SHA-256 + kaynak repo `manifest.json` ile — `07 §11`).
- UI uzman mod: **görsel strateji düzenleyici** — parametre kartları → `build_args` canlı önizleme +
  geçersiz kombinasyon doğrulaması (`06 §2.3`).

**Kontrol noktaları:**
- [ ] Preflight WinDivert'i engelli bulunca UI otomatik ByeDPI'yi öneriyor (desync motorları gri).
- [ ] Her gömülü binary'nin SHA-256'sı UI'de gösteriliyor (`06 §2.8`).
- [ ] Görsel düzenleyici geçersiz kombinasyonu **kurulumdan önce** reddediyor.

---

### Faz 8 — Cila, tray, genişleme (sürekli)

**Hedef:** `docs/06 §2.6` + `07 §Faz 3-4`. Çoğu evorift'te zaten var (tray, otomatik güncelleme,
autostart); kalanları tamamla + uzun-vade.

**Beklenen fonksiyonlar / işler:**
- Tray sağ-tık → profil değiştir / duraklat / çık (mevcut tray menüsünü genişlet).
- "Duraklat/Devam" — hizmeti silmeden durdur (durum makinesi `PAUSED` — `07 §7`).
- Bildirimler: "bağlantı koptu/yeniden bağlanılıyor", "yeni preset mevcut".
- **sing-box** adaptörü (`impl BypassEngine`, modern tünel: WARP/REALITY/Hysteria2/TUIC — `06 §2.1`).
- Topluluk preset deposu (sürümlü git/HTTP — `07 §Faz 2`).

**Kontrol noktaları:**
- [ ] Durum makinesi tek aktif profil garantisi veriyor (`07 §7`): IDLE→APPLYING→ACTIVE→PAUSED/ERROR.
- [ ] Tray'den profil değişimi UI açmadan çalışıyor.
- [ ] sing-box adaptörü WARP'ı mevcut split-tunnel davranışıyla yeniden üretebiliyor (regresyon yok).

---

## 3. Bağımlılık grafiği (faz sırası neden böyle)

```
Faz 0 (kararlılık) ──► Faz 1 (motor soyutlaması) ──┬─► Faz 2 (profiller) ──► Faz 6 (Auto-Pilot)
                                                    ├─► Faz 7 (yeni motorlar)
Faz 4 (rollback) ◄── Faz 3 (DNS/onarım) ───────────┘
Faz 5 (preflight/teşhis) ──► Faz 6 (Auto-Pilot kullanır)
Faz 8 (cila) — her şeyin üstüne, sürekli
```

- **Faz 1 kilit taşı:** Profil (2), Auto-Pilot (6), yeni motorlar (7) hepsi soyutlamaya dayanır.
- **Faz 4 (rollback)** Faz 2'den (profil değişimi temiz kaldırma ister) önce _başlanmalı_, paralel gider.
- **Faz 5 (preflight)** Auto-Pilot'tan (6) önce — Auto-Pilot ISS tespiti + WinDivert kontrolünü kullanır.

---

## 4. Genel kontrol noktaları (her faz için ortak — CLAUDE.md)

Her faz teslimi şunları geçmeli:
- [ ] **Doğrulama sırası:** `cargo check` → `svelte-check` → (yalnız dev kapalıyken) `npm run build`.
- [ ] Çalışan `tauri dev`'i bozmamak için `cargo check --target-dir tmp_check`.
- [ ] **Fail-safe:** Bundle/ağ/admin yoksa her yeni yol loglanan no-op'a iner — boot ASLA bozulmaz
      (winws web'i açık tutar; `warp.rs` sim yolu kalıbı).
- [ ] Tüm ayrıcalıklı iş `evorift-svc.exe`'de; UI yalnız IPC ile niyet bildirir (webview'de iş yok).
- [ ] Yeni `ipc.rs::Command` varyantı → `validate()`'e whitelist kuralı (asla doğrulanmamış path/komut).
- [ ] Yeni harici binary → SHA-256 + kaynak repo `manifest.json`'a; dağıtımda yalnız build çıktısı.
- [ ] `BlackHole.svelte`'e dokunma (ayrı sohbette geliştiriliyor).

---

## 5. Hızlı özet

- **net3 çekirdeği (SOLUTION.md) + V0.1.3 düzeltmeleri zaten inmiş** → Faz 0 sadece commit + çıkış.
- **Asıl entegre edilecek araştırma = `docs/06`+`07` blueprint'i** → çok-motorluluk, profiller,
  Auto-Pilot, rollback, onarım, preflight.
- **Faz 1 (motor soyutlaması) kilit taşı** — sonraki her şey ona dayanır.
- evorift blueprint'in 3-katmanlı mimarisini **zaten uyguluyor**; sıfırdan değil, mevcut
  `service.rs`/`engine.rs`/`warp.rs`/`ipc.rs` üstüne genişletiyoruz.
