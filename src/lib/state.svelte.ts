import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { eventLog } from "./log.svelte";
import { toasts } from "./toast.svelte";

type Status = "off" | "connecting" | "on";
/// Uygulama başı koruma modu (kullanıcı seçer; servis aggregate eder):
///  - "off"  → Korumasız · en hızlı. Domain otomatik tespiti durdurulur, WARP'a alınmaz.
///             (winws sistem geneli çalışmaya devam eder — tek uygulamayı winws'tan dışlamak
///              için PID/WinDivert filtresi henüz yok; gerçek "0 ek gecikme" tüm uygulamalar
///              off + WARP kapalı olduğunda yaşanır.)
///  - "dpi"  → DPI · hızlı. Varsayılan. winws sidecar bu uygulamanın TCP/443 + QUIC + Discord
///             ses/STUN trafiğini fake-desync ile DPI'dan geçirir. Cloudflare WARP devrede DEĞİL.
///  - "warp" → WARP · maksimum koruma. winws + Cloudflare WARP WireGuard split-tunnel
///             birlikte. Discord IP aralıkları (Cloudflare 162.159 + Discord 66.22 + medya 104.29)
///             şifreli tünelden gider → DPI yalnız şifreli UDP görür. En sağlam ama +~30ms ping.
export type AppMode = "off" | "dpi" | "warp";
export type AppRow = {
  id: string; name: string; kind: string;
  mode: AppMode; allow: boolean;
  down: number; up: number; // hız limiti kbps (0 = sınırsız)
  heavy: boolean;           // çok bant kullanan (Oyun Modu tespiti)
  exe?: string; path?: string; // gerçek enumerasyon (Faz P1.3, list_apps)
  domains?: string[];       // bu uygulama için otomatik tespit edilen bypass domain'leri
};
type Metrics = { running: boolean; ping: number; jitter: number; loss: number; down: number; up: number };
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export const HISTORY_LEN = 80; // sparkline örnek sayısı
const PREFS_KEY = "evorift_prefs";

// Tray tooltip metni (dile göre). state i18n'i import etmez (döngü olmasın) → küçük yerel harita.
const TRAY_ON: Record<string, string> = { tr: "Korumalı", en: "Protected", es: "Protegido", ru: "Защищено" };
const TRAY_OFF: Record<string, string> = { tr: "Kapalı", en: "Off", es: "Apagado", ru: "Выключено" };

// Bilinen yoğun (arka plan bant) uygulamalar — Oyun Modu/Solo varsayılan "heavy" tespiti (Faz P1.3).
const HEAVY_EXES = new Set([
  "chrome.exe", "msedge.exe", "firefox.exe", "opera.exe", "brave.exe",
  "steam.exe", "steamwebhelper.exe", "epicgameslauncher.exe",
  "onedrive.exe", "dropbox.exe", "googledrivefs.exe", "obs64.exe", "obs32.exe",
]);

/// URL/serbest girdi → YALIN alan adı (host). "https://discord.com/app?x=1" → "discord.com".
/// Motorun beyaz-listesine ([a-z0-9.-], en az bir nokta, ≤253) uymuyorsa "" döner.
/// Neden: kullanıcı tam URL yapıştırırsa, ipc::validate o girdiyi (':' '/') reddeder ve TÜM set_hostlist
/// komutu düşer → engine hostlist GÜNCELLENMEZ → maskeleme/bypass çalışmaz ("URL'ler gizlenmiyor" bug'ı).
export function toDomain(input: string): string {
  let v = (input ?? "").trim().toLowerCase();
  if (!v) return "";
  v = v.replace(/^[a-z][a-z0-9+.\-]*:\/\//, ""); // şema (https:// vb.)
  v = v.split(/[/?#]/)[0];                        // yol/sorgu/fragment
  const at = v.lastIndexOf("@");
  if (at >= 0) v = v.slice(at + 1);               // userinfo
  v = v.split(":")[0];                            // port
  v = v.replace(/^\.+|\.+$/g, "");                // baştaki/sondaki nokta
  if (!v || v.length > 253 || !v.includes(".") || !/^[a-z0-9.\-]+$/.test(v)) return "";
  return v;
}

class AppState {
  status = $state<Status>("off");

  // ---- canlı metrikler (servisten "telemetry" event'i ile beslenir; bkz. lib.rs) ----
  ping = $state(0); // ms
  jitter = $state(0); // ms
  loss = $state(0); // %
  down = $state(0); // Mbps
  up = $state(0); // Mbps
  pingHistory = $state<number[]>([]);

  // ---- bağlantı / strateji seçimleri (bölümler arası + yeniden başlatma kalıcı) ----
  strategy = $state<string>("auto");
  dns = $state<string>("cloudflare");
  // Gerçek Discord/Roblox UYGULAMASI tek üst-domain'den fazlasını kullanır: websocket gateway
  // (gateway.discord.gg), CDN (cdn.discordapp.com), ses (discord.media), Roblox asset (rbxcdn.com).
  // Eksik olursa uygulama bağlanamaz → tam çekirdek liste şart. host_matches suffix eşleşir.
  // (Bu liste svc_ctl'in kanıtlanmış 10-domain listesiyle birebir + YouTube.)
  static CORE_SITES = [
    "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
    "gateway.discord.gg", "cdn.discordapp.com", "roblox.com", "www.roblox.com", "rbxcdn.com",
  ] as const;
  sites = $state<string[]>([...AppState.CORE_SITES, "youtube.com", "googlevideo.com"]);

  // ---- algılanan uygulamalar (per-app mode/izin/limit; kalıcı) ----
  // Varsayılan mod = "dpi" (winws yeterli + hızlı). Discord'da DPI = GoodbyeDPI-eşdeğeri yaklaşım:
  // TCP/443 fake-multidisorder gateway.discord.gg + discord.media TLS handshake'ini açar → Discord
  // client UDP voice'u kendi başına bağlar (operatör genelde UDP'yi bloklamaz). Ek olarak winws
  // UDP Discord IP-discovery + STUN imzalarına fake-desync uygular (Superbox gibi UDP'ye dokunan
  // hatlar için ek savunma). DPI yetmezse kullanıcı Discord'u WARP'a yükseltebilir (Superbox fallback).
  // VALORANT = "off" (anti-cheat Vanguard kernel-level → DPI müdahalesi şüpheli).
  apps = $state<AppRow[]>([
    { id: "discord", name: "Discord", kind: "Sohbet · Ses", mode: "dpi", allow: true, down: 0, up: 0, heavy: false },
    { id: "roblox", name: "Roblox", kind: "Oyun", mode: "dpi", allow: true, down: 0, up: 0, heavy: false },
    { id: "chrome", name: "Google Chrome", kind: "Tarayıcı", mode: "dpi", allow: true, down: 0, up: 0, heavy: true },
    { id: "steam", name: "Steam", kind: "Oyun · Launcher", mode: "off", allow: true, down: 0, up: 0, heavy: true },
    { id: "onedrive", name: "OneDrive", kind: "Bulut yedekleme", mode: "off", allow: true, down: 0, up: 0, heavy: true },
    { id: "valorant", name: "VALORANT", kind: "Oyun · Vanguard", mode: "off", allow: true, down: 0, up: 0, heavy: false },
  ]);

  // ---- hız sınırı / oyun modu yardımcı durum ----
  soloMode = $state(false);            // oyun sırasında arka plan upload'larını durdur
  #soloSnapshot: { id: string; up: number }[] | null = null; // solo öncesi up limitleri (geri yükleme)
  // oyun modu öncesi durum (kapanınca geri yükle)
  #gmSnapshot: {
    strategy: string;
    dns: string;
    tweaks: Record<string, unknown>;
    limits: { id: string; down: number; up: number }[];
  } | null = null;

  // ---- otomatik oyun modu + paylaşılan tweak durumu (Advanced ekranıyla ortak) ----
  gameMode = $state(false);
  // aktivasyon anı (koruma/oyun modu açılırken) — binary 01 yağmuru bunu izler
  activating = $state(false);
  tweaks = $state({
    nagle: false, heuristics: false, throttleIdx: false, nicPower: false, highPerf: false,
    autotuning: "normal", congestion: "cubic", rss: true, rsc: true, offload: true, mtu: 1500,
  });

  // ---- ayarlar ----
  autostart = $state(false);
  startMinimized = $state(false);
  autoProtect = $state(true); // app açılınca korumayı otomatik başlat (servis modeli: app açık = korumalı)
  reduceMotion = $state(false);
  language = $state<"tr" | "en" | "es" | "ru">("tr");
  isp = $state<string>("turkcell-superbox");

  // ---- yönetici (elevation) durumu — gerçek tweak/limit/firewall yalnız elevated'da uygulanır (Faz P2.4) ----
  isAdmin = $state(true);        // default true → kontrol edilene kadar banner gizli (yanıp sönme yok)
  elevDismissed = $state(false); // banner'ı bu oturum için kapat

  // ---- onboarding (ilk çalıştırma) ----
  showOnboarding = $state(false);

  async init() {
    // kaydedilmiş tercihleri yükle + uygula + otomatik kaydet (her şey kalıcı)
    this.#loadPrefs();
    this.#applyPrefs();
    this.#startPersist();

    eventLog.info("evorift başlatıldı · v0.1.0");
    try {
      const s = await invoke<{ running: boolean }>("protection_status");
      this.status = s.running ? "on" : "off";
    } catch (_) {}

    // Servis modeli: app açılınca korumayı otomatik başlat (kullanıcı isteği: "app açınca servis çalışır").
    // Zaten açıksa (önceki oturum/servis boot'ta açtıysa) dokunma. autoProtect kapalıysa atla.
    if (this.autoProtect && this.status === "off") {
      this.toggle(); // await yok → UI açılışını bloke etme; toggle kendi animasyon/durumunu yönetir
    }

    // Kalıcı Oyun Modu: bir kez açıldıysa (PC kapanıp açılsa bile) ayarları servise YENİDEN uygula.
    // Protection toggle / snapshot YOK — snapshot #loadPrefs ile yüklendi, OS tweak'leri idempotent.
    if (this.gameMode) this.#applyGameTweaks();

    // gerçek çalışan ağ-uygulamalarını tespit et (mock listenin yerine, Faz P1.3)
    this.refreshApps();
    // bypass'lı uygulamaların domain'lerini otomatik tespit et + periyodik tara (kullanıldıkça yeni domain yakalar)
    this.#startDomainWatch();
    // yönetici mi? (gerçek tweak/limit/firewall için gerekli — banner bunu gösterir)
    try { this.isAdmin = await invoke<boolean>("is_admin"); } catch (_) {}

    // canlı metrikleri servisin "telemetry" event'inden al (docs/05 §2)
    try {
      await listen<Metrics>("telemetry", (e) => this.#applyTelemetry(e.payload));
    } catch (_) {}

    // tray "Korumayı Aç/Kapat" event'i (Faz 4.7)
    try {
      await listen("tray-toggle", () => this.toggle());
    } catch (_) {}
    this.#syncTray();
    this.syncHostlist(); // kalıcı site listesini servise bildir (Faz 3.4)
    this.syncAppModes(); // per-app mod tercihlerini servise bildir → WARP tüneli agregasyona göre senkron

    // ilk çalıştırma onboarding'i (localStorage'da işaret yoksa göster) — docs/02 §6
    try {
      if (typeof localStorage !== "undefined" && !localStorage.getItem("evorift_onboarded")) {
        this.showOnboarding = true;
      }
    } catch (_) {}
  }

  async toggle() {
    if (this.status === "connecting") return;
    if (this.status === "on") {
      // KAPAT — anında (WARP gibi): durumu hemen çevir, durdurmayı arka planda gönder. Motor fail-open
      // (paketi aynen geçirir) olduğundan internet asla kesilmez → durdurma yanıtını beklemeye gerek yok.
      this.status = "off";
      this.gameMode = false;
      this.activating = false;
      this.#resetMetrics();
      this.#syncTray();
      invoke("stop_protection").catch(() => {});
      eventLog.info("Koruma kapatıldı");
      return;
    }
    // AÇ — güncel hostlist + güvenli DNS + motoru başlat.
    this.activating = true; // 01 yağmuru başlasın
    this.status = "connecting";
    try {
      await invoke("set_hostlist", { domains: this.sites }).catch(() => {}); // motor güncel listeyle başlasın
      // Her açılışta seçili DNS'i yeniden uygula (yalnız manuel seçimde değil) → AKTİF adaptör daima
      // güvenli DNS + taze önbellek alır (engelli/zehirli kayıtlar temizlenir). "bazı DNS çalışmıyor"
      // şikâyetinin başlıca nedeni buydu. Bloke etme: fire-and-forget (servis arka planda uygular).
      invoke("set_dns", { profile: this.dns }).catch(() => {});
      await invoke("start_protection");
      this.status = "on"; // start dönünce ANINDA açık göster — 1.8 sn animasyon gecikmesi kaldırıldı
      eventLog.ok(`Koruma açıldı · strateji: ${this.strategy} · DNS: ${this.dns}`);
    } catch (_) {
      this.status = "off";
      eventLog.error("Koruma başlatılamadı");
    }
    this.#syncTray();
    await sleep(900); // yağmur yalnız KOZMETİK — "on" durumunu GECİKTİRMEZ
    this.activating = false;
  }

  // ---- otomatik oyun modu ----
  // Tek tıkla: korumayı aç + en iyi strateji/DNS + güvenli (🟢) tweak'leri uygula.
  // Yalnız 🟢 güvenli ayarları açar; 🟡/🔴 (RSS/RSC/offload/MTU) ellenmez.
  // Oyun Modu açılınca uygulanan ayarların listesi (önizleme butonu için).
  static GM_SAFE_TWEAKS = ["nagle", "heuristics", "nicPower", "highPerf"] as const;

  async applyGameMode() {
    // 1) mevcut durumu kaydet → kapanınca eski haline dön. ZATEN varsa DOKUNMA: reboot sonrası reapply
    //    veya tekrar-aç, kullanıcının gerçek temel ayarlarını taşıyan snapshot'ı EZMESİN.
    if (!this.#gmSnapshot) {
      this.#gmSnapshot = {
        strategy: this.strategy,
        dns: this.dns,
        tweaks: { ...this.tweaks },
        limits: this.apps.map((a) => ({ id: a.id, down: a.down, up: a.up })),
      };
    }
    // 2) korumayı aç (yağmur)
    if (this.status === "off") {
      await this.toggle();
    } else {
      this.activating = true;
      await sleep(1800); // 01 yağmuru görünür sürsün
      this.activating = false;
    }
    // 3) en iyi ayarlar
    this.strategy = "auto";
    this.dns = "cloudflare";
    this.tweaks.nagle = true;
    this.tweaks.heuristics = true;
    this.tweaks.nicPower = true;
    this.tweaks.highPerf = true;
    this.tweaks.congestion = "cubic";
    this.tweaks.autotuning = "normal";
    this.gameMode = true;
    // 4) servise ilet (Hız-sınırlama KALDIRILDI — yalnız strateji/DNS/güvenli tweak; donma yapan popup yok)
    this.#applyGameTweaks();
    eventLog.ok("Oyun Modu açıldı · en iyi ayarlar uygulandı");
  }

  /// Oyun Modu ayarlarını servise gönder (strateji + DNS + 🟢 güvenli tweak'ler). applyGameMode ve
  /// boot'taki kalıcı-reapply ortak kullanır (protection toggle / snapshot YOK).
  #applyGameTweaks() {
    this.#send("set_strategy", { id: "auto" });
    this.#send("set_dns", { profile: "cloudflare" });
    for (const key of AppState.GM_SAFE_TWEAKS) this.#send("set_tweak", { key, value: "on" });
    this.#send("set_tweak", { key: "congestion", value: "cubic" });
    this.#send("set_tweak", { key: "autotuning", value: "normal" });
  }

  exitGameMode() {
    const snap = this.#gmSnapshot;
    if (snap) {
      // ayarları oyun modu öncesine geri yükle
      this.strategy = snap.strategy;
      this.dns = snap.dns;
      Object.assign(this.tweaks, snap.tweaks);
      for (const l of snap.limits) {
        const a = this.apps.find((x) => x.id === l.id);
        if (a) { a.down = l.down; a.up = l.up; }
      }
      // servise geri yükle (best-effort)
      this.#send("set_strategy", { id: snap.strategy });
      this.#send("set_dns", { profile: snap.dns });
      for (const key of AppState.GM_SAFE_TWEAKS) {
        this.#send("set_tweak", { key, value: (snap.tweaks as Record<string, unknown>)[key] ? "on" : "off" });
      }
      this.#send("set_tweak", { key: "congestion", value: String((snap.tweaks as Record<string, unknown>).congestion) });
      this.#send("set_tweak", { key: "autotuning", value: String((snap.tweaks as Record<string, unknown>).autotuning) });
      for (const l of snap.limits) this.#sendLimit(l.id, l.down, l.up);
      this.#gmSnapshot = null;
    }
    this.gameMode = false;
    eventLog.info("Oyun Modu kapatıldı · ayarlar geri yüklendi");
  }

  /// Per-app hız limiti ayarla (Hız Sınırı menüsü).
  setLimit(id: string, down: number, up: number) {
    const a = this.apps.find((x) => x.id === id);
    if (a) { a.down = down; a.up = up; }
    this.#sendLimit(id, down, up);
  }

  /// Bypass alan adı listesini servise gönder (Faz 3.4). Site eklen/çıkınca + bağlanınca çağrılır.
  syncHostlist() {
    // Yalnız GEÇERLİ yalın alan adlarını gönder. ipc::validate, listede TEK bir geçersiz domain (tam URL,
    // port, yol vb.) olsa TÜM SetHostlist'i reddeder → engine hostlist güncellenmez → bypass çalışmaz.
    // Bu yüzden burada savunmacı süz: bozuk/URL girdileri toDomain ile yalına indir, geçersizleri ele.
    const domains = Array.from(new Set(this.sites.map(toDomain).filter(Boolean)));
    this.#send("set_hostlist", { domains });
  }

  // ---- Apps → otomatik domain maskeleme ----
  // Bir uygulamayı "Bypass"a alınca, o uygulamanın ULAŞTIĞI tüm domain'leri otomatik tespit edip
  // maskeleme (hostlist) listesine ekler. Açık kaldıkça periyodik tarama yeni domain'leri yakalar.

  /// Uygulamanın koruma modunu değiştir (off/dpi/warp). off = korumasız+hızlı; dpi = winws (varsayılan,
  /// Discord ses dahil); warp = winws + Cloudflare WARP split-tunnel (maks koruma, +~30ms ping).
  /// off → domain otomatik tespiti durur; mevcut domain'ler kalıcı kalır (silinmez, kullanıcı isteği).
  /// dpi/warp → domain taraması anında hızlanır + servis WARP'ı agregasyona göre açar/kapatır.
  async setAppMode(r: AppRow, mode: AppMode) {
    if (r.mode === mode) return;
    const prev = r.mode;
    r.mode = mode;
    // Servise BÜTÜNÜKLÜ snapshot gönder → WARP tüneli agregasyon ile senkronlansın
    this.syncAppModes();
    // Toast'lar gerçek davranışı yansıtsın: off = gerçek DPI exclusion (5sn'de devreye girer);
    // dpi = winws + domain keşfi; warp = winws + WARP tünel.
    if (mode === "off") {
      toasts.info(`${r.name}: DPI'dan hariç tutulacak · ~5sn içinde devreye girer`);
      return;
    }
    if (prev === "off") this.#resetDomainWatch(); // domain taramasını hızlandır
    const added = await this.#detectAndAdd(r);
    if (mode === "warp") {
      toasts.success(`${r.name}: WARP tüneli devrede${added > 0 ? ` · ${added} domain` : ""}`);
    } else if (added > 0) {
      toasts.success(`${r.name}: DPI koruması açık · ${added} domain otomatik maskeleniyor`);
    } else if (prev === "off") {
      toasts.info(`${r.name}: DPI koruması açık · uygulamayı kullanırken domain'ler yakalanacak`);
    }
  }

  /// Tüm uygulama modlarının BÜTÜNÜKLÜ snapshot'ını servise gönder: (id, mode, exe path).
  /// Servis bu listeden iki şey çıkarır:
  ///  1. WARP tüneli açık/kapalı (en az bir "warp" → açık; aksi halde saf winws)
  ///  2. "off" modlu uygulamaların PID'lerinin source port'ları → winws WinDivert capture filter'ından
  ///     hariç tutulur (gerçek per-app DPI off — bkz. pid_scan modülü)
  syncAppModes() {
    const modes: [string, AppMode, string][] = this.apps.map((a) => [a.id, a.mode, a.path ?? ""]);
    this.#send("set_app_modes", { modes });
  }

  /// Uygulamanın ulaştığı domain'leri backend'den tespit et + sites'e ekle. YENİ eklenen sayıyı döner.
  async #detectAndAdd(r: AppRow): Promise<number> {
    if (!r.exe) return 0;
    let found: string[] = [];
    try { found = await invoke<string[]>("detect_app_domains", { exe: r.exe }); } catch (_) { found = []; }
    if (!found.length) return 0;
    const prev = new Set(r.domains ?? []);
    const newly = found.filter((d) => !prev.has(d));
    r.domains = Array.from(new Set([...(r.domains ?? []), ...found]));
    const before = this.sites.length;
    this.sites = Array.from(new Set([...this.sites, ...found]));
    if (this.sites.length !== before) {
      this.syncHostlist();
      eventLog.info(`${r.name}: ${newly.length} domain otomatik bypass'a eklendi`);
    }
    return newly.length;
  }

  // ---- adaptif domain izleme aralığı ----
  // Bypass'lı bir uygulama AÇILINCA (veya evorift açıldığında uygulama zaten açıksa) domain taraması
  // HIZLI başlar (100ms) → yeni domain'ler ANINDA maskelenir. Sonra her 3 dk'da aralık ×3 büyür
  // (100→300→900→2700→5000ms), TAVAN 5sn. Aktivite (yeni domain bulunması / bypass açılması / evorift
  // açılışı) hızlı pencereyi sıfırlar. (Eski: sabit 15sn → yeni açılan uygulamanın domain'leri geç yakalanıyordu.)
  static DW_BASE = 100;        // ms — en hızlı tarama aralığı (başlangıç)
  static DW_MAX = 5000;        // ms — tavan
  static DW_STEP_MS = 180_000; // 3 dk — her adımda aralık ×3
  #domainWatchStart = 0;       // mevcut hızlı pencerenin başlangıç zamanı (backoff bundan hesaplanır)

  /// Hızlı pencereyi sıfırla → bir sonraki tarama tekrar DW_BASE (100ms) aralıkla başlar.
  /// Tetikler: evorift açılışı, bir uygulamayı bypass'a alma, yeni domain bulunması (uygulama yeni açıldı).
  #resetDomainWatch() {
    this.#domainWatchStart = Date.now();
  }

  /// Adaptif otomatik tarama (recursive setTimeout → ağır PowerShell taramaları ÜST ÜSTE BİNMEZ; bir tur
  /// bitmeden sonraki başlamaz = optimize). Koruma açıkken bypass'lı her uygulamanın domain'lerini tespit
  /// eder; aralığı geçen süreye göre DW_BASE→DW_MAX arası ×3/3dk büyütür. Yeni domain → hızlıya döner.
  #startDomainWatch() {
    this.#resetDomainWatch(); // evorift açılışı → 100ms'den başla (uygulama zaten açıksa domain'leri hemen yakala)
    const tick = async () => {
      let added = 0;
      if (this.status === "on") {
        for (const r of this.apps) {
          if (r.mode !== "off" && r.exe) {
            const n = await this.#detectAndAdd(r);
            if (n > 0) { added += n; toasts.info(`${r.name}: ${n} yeni domain maskelendi`); }
          }
        }
      }
      // yeni domain = uygulama aktif yeni bağlantı açıyor (muhtemelen yeni açıldı) → hızlı pencereye dön
      if (added > 0) this.#resetDomainWatch();
      // backoff: geçen süreye göre aralığı ×3/3dk büyüt, 5sn'de tut
      const elapsed = Date.now() - this.#domainWatchStart;
      const steps = Math.floor(elapsed / AppState.DW_STEP_MS);
      const interval = Math.min(AppState.DW_MAX, AppState.DW_BASE * Math.pow(3, steps));
      setTimeout(tick, interval);
    };
    tick();
  }

  /// Solo Mod: açıkken yoğun (arka plan) uygulamaların YÜKLEMESİNİ kıs (egress — QoS'un güvenilir
  /// sınırladığı yön); kapanınca eski up limitlerini geri yükle. set_limit ile gerçek uygulanır (admin'de).
  setSolo(on: boolean) {
    this.soloMode = on;
    if (on) {
      this.#soloSnapshot = this.apps.filter((a) => a.heavy && a.allow).map((a) => ({ id: a.id, up: a.up }));
      for (const a of this.apps) {
        if (a.heavy && a.allow) { a.up = 256; this.#sendLimit(a.id, a.down, 256); }
      }
      eventLog.info("Solo Mod açık · arka plan yüklemeleri kısıldı");
    } else {
      for (const s of this.#soloSnapshot ?? []) {
        const a = this.apps.find((x) => x.id === s.id);
        if (a) { a.up = s.up; this.#sendLimit(a.id, a.down, s.up); }
      }
      this.#soloSnapshot = null;
      eventLog.info("Solo Mod kapalı · limitler geri yüklendi");
    }
  }

  /// Gerçek çalışan ağ-uygulamalarını tespit et + kalıcı tercihlerle (bypass/izin/limit) birleştir (Faz P1.3).
  /// Boş dönerse (Tauri yok / tespit edilemedi) mevcut liste korunur.
  async refreshApps() {
    try {
      const det = await invoke<{ id: string; name: string; exe: string; path: string; kind: string }[]>("list_apps");
      if (!det.length) return;
      const prev = new Map(this.apps.map((a) => [a.id, a]));
      const detIds = new Set(det.map((d) => d.id));
      const merged: AppRow[] = det.map((d) => {
        const p = prev.get(d.id);
        return {
          id: d.id, name: d.name || d.exe, kind: d.kind || d.exe, exe: d.exe, path: d.path,
          // Yeni algılanan uygulamalar VARSAYILAN "off" — kullanıcı bilinçli açsın (rastgele ön-seçim olmasın).
          // Çekirdek liste (Discord/Roblox) zaten kalıcı tercihten gelir → bu yalnız bilinmeyen exe'lere uygulanır.
          mode: p?.mode ?? "off",
          allow: p?.allow ?? true,
          down: p?.down ?? 0, up: p?.up ?? 0,
          heavy: p?.heavy ?? HEAVY_EXES.has(d.exe.toLowerCase()),
          domains: p?.domains ?? [], // KALICI: otomatik bulunan domain'ler refresh/restart'ta KAYBOLMASIN
        };
      });
      // tespit edilmeyen ama ANLAMLI tercihi olan uygulamaları KORU → tercih/firewall-kuralı kaybını önle (review MED-3)
      for (const a of this.apps) {
        if (!detIds.has(a.id) && (!a.allow || a.down > 0 || a.up > 0 || a.mode !== "dpi")) merged.push(a);
      }
      this.apps = merged;
      this.syncAppModes(); // yeni algılanan uygulamaların modlarını servise bildir
      eventLog.info(`${det.length} ağ uygulaması algılandı`);
    } catch (_) {}
  }

  /// Kullanıcı-dostu "Ağı Onar": interneti bozabilecek ayarları güvenli varsayılana döndür + DNS onar.
  repairNetwork() {
    Object.assign(this.tweaks, {
      congestion: "cubic", autotuning: "normal", mtu: 1500, offload: true, rss: true, rsc: true,
    });
    this.#send("set_tweak", { key: "congestion", value: "cubic" });
    this.#send("set_tweak", { key: "autotuning", value: "normal" });
    this.#send("set_tweak", { key: "mtu", value: "1500" });
    this.#send("set_tweak", { key: "offload", value: "on" });
    this.#send("set_tweak", { key: "rss", value: "on" });
    this.#send("set_tweak", { key: "rsc", value: "on" });
    for (const tool of ["flushdns", "registerdns", "dnscache"]) this.#send("run_repair", { tool });
    eventLog.warn("Ağ onarıldı · ayarlar güvenli varsayılana döndü");
  }

  /// Tüm ayarları varsayılana döndür — YALNIZ UI değil, SERVİSE de uygula (gerçek sistem de sıfırlansın).
  /// (Eski Settings.resetDefaults yalnız UI'ı sıfırlıyordu → uygulanmış registry/netsh tweak'leri sistemde
  /// açık kalıyordu = yanıltıcı. Artık servise de gönderiliyor.)
  resetToDefaults() {
    this.autostart = false;
    this.startMinimized = false;
    this.reduceMotion = false;
    this.language = "tr";
    this.strategy = "auto";
    this.dns = "cloudflare";
    this.gameMode = false;
    this.#gmSnapshot = null;
    // 🟢 bool tweak'leri kapat + servise gönder
    for (const key of ["nagle", "heuristics", "throttleIdx", "nicPower", "highPerf"] as const) {
      this.tweaks[key] = false;
      this.#send("set_tweak", { key, value: "off" });
    }
    // autostart kaydını kaldır + strateji/DNS varsayılanı
    this.#send("set_autostart", { enable: false, minimized: false });
    this.#send("set_strategy", { id: "auto" });
    this.#send("set_dns", { profile: "cloudflare" });
    // congestion/autotuning/mtu/offload/rss/rsc varsayılanı + DNS önbellek temizliği (servise gönderir)
    this.repairNetwork();
    eventLog.warn("Ayarlar varsayılana döndürüldü · UI + sistem");
  }

  #send(cmd: string, args: Record<string, unknown>) {
    invoke(cmd, args).catch(() => {}); // Tauri yoksa/servis kapalıysa sessiz
  }

  /// set_limit'i her zaman gerçek exe yolu ile gönder (QoS eşleştirmesi boşluklu adlarda da çalışsın).
  #sendLimit(id: string, down: number, up: number) {
    const a = this.apps.find((x) => x.id === id);
    this.#send("set_limit", { id, path: a?.path ?? "", down, up });
  }

  // ---- tray + otomatik başlatma (Faz 4.7) ----
  setAutostart(enable: boolean) {
    this.autostart = enable;
    this.#send("set_autostart", { enable, minimized: this.startMinimized });
  }
  setStartMinimized(v: boolean) {
    this.startMinimized = v;
    if (this.autostart) this.#send("set_autostart", { enable: true, minimized: v });
  }
  /// Yönetici olarak yeniden başlat (UAC). Kabul edilirse mevcut süreç kapanır.
  async relaunchAsAdmin() {
    try { await invoke("relaunch_as_admin"); } catch (_) {}
  }
  #syncTray() {
    const word = this.status === "on" ? TRAY_ON[this.language] : TRAY_OFF[this.language];
    this.#send("set_tray_tooltip", { text: `evorift — ${word ?? this.status}` });
  }

  // ---- onboarding ----
  finishOnboarding() {
    this.showOnboarding = false;
    try { localStorage.setItem("evorift_onboarded", "1"); } catch (_) {}
  }
  replayOnboarding() {
    this.showOnboarding = true;
  }

  // ---- kalıcılık (localStorage) ----
  #loadPrefs() {
    try {
      if (typeof localStorage === "undefined") return;
      const raw = localStorage.getItem(PREFS_KEY);
      if (!raw) return;
      const p = JSON.parse(raw);
      if (p.language) this.language = p.language;
      if (typeof p.reduceMotion === "boolean") this.reduceMotion = p.reduceMotion;
      if (typeof p.autostart === "boolean") this.autostart = p.autostart;
      if (typeof p.startMinimized === "boolean") this.startMinimized = p.startMinimized;
      if (typeof p.autoProtect === "boolean") this.autoProtect = p.autoProtect;
      if (p.strategy) this.strategy = p.strategy;
      if (p.dns) this.dns = p.dns;
      if (p.isp) this.isp = p.isp;
      if (Array.isArray(p.sites)) this.sites = p.sites;
      // MIGRASYON: eski varsayılan yalnız 3 üst-domain'di (discord.com/roblox.com/youtube.com) →
      // gerçek Discord/Roblox uygulamasının kritik domain'leri (gateway.discord.gg, cdn.discordapp.com,
      // discord.media, rbxcdn.com vb.) eksikti → uygulama bağlanamıyordu. Eksik çekirdek domain'leri ekle.
      this.sites = Array.from(new Set([...this.sites, ...AppState.CORE_SITES]));
      // MIGRASYON: eski/bozuk girdileri (tam URL, port, yol, büyük harf) yalın alan adına indir +
      // geçersizleri ele → kalıcı listede kalmış bozuk bir URL artık hostlist'i zehirlemesin.
      this.sites = Array.from(new Set(this.sites.map(toDomain).filter(Boolean)));
      if (Array.isArray(p.apps)) {
        // MIGRASYON: eski şema `bypass: boolean` → yeni `mode: "off"|"dpi"|"warp"`.
        // bypass=true → "dpi" (GoodbyeDPI-eşdeğeri varsayılan, Discord ses dahil yeter); bypass=false → "off".
        // WARP'a yükseltmek kullanıcının elinde (Superbox gibi DPI yetmeyen hatlar için fallback).
        this.apps = (p.apps as Array<Partial<AppRow> & { bypass?: boolean }>).map((a) => {
          let mode: AppMode;
          if (a.mode === "off" || a.mode === "dpi" || a.mode === "warp") {
            mode = a.mode;
          } else {
            mode = a.bypass === true ? "dpi" : "off";
          }
          return { ...(a as AppRow), mode };
        });
      }
      if (p.tweaks) Object.assign(this.tweaks, p.tweaks);
      if (typeof p.soloMode === "boolean") this.soloMode = p.soloMode;
      // Kalıcı Oyun Modu: bir kez açılırsa PC kapanıp açılsa bile açık kalır + temel ayar snapshot'ı.
      if (typeof p.gameMode === "boolean") this.gameMode = p.gameMode;
      if (p.gmSnapshot) this.#gmSnapshot = p.gmSnapshot;
    } catch (_) {}
  }

  // dil + hareket tercihini DOM'a uygula (erişilebilirlik)
  #applyPrefs() {
    if (typeof document === "undefined") return;
    document.documentElement.lang = this.language;
    document.documentElement.classList.toggle("reduce-motion", this.reduceMotion);
  }

  // herhangi bir tercih değişince otomatik kaydet + DOM'a yansıt
  #startPersist() {
    $effect.root(() => {
      $effect(() => {
        const snap = JSON.stringify({
          language: this.language,
          reduceMotion: this.reduceMotion,
          autostart: this.autostart,
          startMinimized: this.startMinimized,
          autoProtect: this.autoProtect,
          strategy: this.strategy,
          dns: this.dns,
          isp: this.isp,
          sites: this.sites,
          apps: this.apps,
          tweaks: { ...this.tweaks },
          soloMode: this.soloMode,
          gameMode: this.gameMode,
          gmSnapshot: this.#gmSnapshot,
        });
        try { localStorage.setItem(PREFS_KEY, snap); } catch (_) {}
        // dil + reduce-motion her değişimde DOM'a yansısın
        document.documentElement.lang = this.language;
        document.documentElement.classList.toggle("reduce-motion", this.reduceMotion);
      });
    });
  }

  // ---- canlı telemetri (servisten gelen "telemetry" event'i) ----
  #applyTelemetry(m: Metrics) {
    this.ping = m.ping;
    this.jitter = m.jitter;
    this.loss = m.loss;
    this.down = m.down;
    this.up = m.up;
    if (m.running) {
      if (this.pingHistory.length === 0) {
        this.pingHistory = Array(HISTORY_LEN).fill(m.ping); // grafiği düz başlat
      } else {
        const h = this.pingHistory.slice(-(HISTORY_LEN - 1));
        h.push(m.ping);
        this.pingHistory = h;
      }
    } else {
      this.pingHistory = [];
    }
  }

  #resetMetrics() {
    this.ping = 0; this.jitter = 0; this.loss = 0; this.down = 0; this.up = 0;
    this.pingHistory = [];
  }
}

export const app = new AppState();
