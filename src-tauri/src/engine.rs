//! Engine katmanı (blueprint `docs/07 §3` — `IBypassEngine`). Her atlatma aracı tek bir
//! [`BypassEngine`] arayüzü arkasında: strateji veriden komut satırı üretir, süreç yönetir,
//! ön-uçuş (preflight) raporlar. "Strateji = veri" ilkesi: parametreler string değil, tipli.
//!
//! Mimari katmanlar (net3 docs/07 §1):
//!  - [`Strategy`]: tipli DPI-desync strateji verisi (winws parametrelerine birebir).
//!  - [`BypassEngine`]: id/kind/caps + build_args/start/stop/is_running sözleşmesi (tüm motorlar ortak).
//!  - [`WinwsEngine`]: VARSAYILAN motor — kanıtlanmış `winws` (zapret) sidecar (Türkcell canlı doğrulandı).
//!  - [`SimEngine`]: Windows dışı / dev no-op (UI↔IPC zincirini uçtan uca çalıştırır).
//!  - [`crate::byedpi`] / [`crate::goodbyedpi`]: kernel-siz / alternatif motorlar (bundle gerektirir).
//!  - [`crate::warp`]::WarpEngine: tünel motoru (Discord split-tunnel · Tam Koruma full-tunnel).
//!
//! NOT: Eski saf-Rust WinDivert motoru (engine/real.rs + byte-cerrahisi) net3'e geçişte KALDIRILDI;
//! winws QUIC/gateway desync'ini gerçek yapar (net3/SOLUTION.md §3.3). Discord'un kendisi WARP
//! split-tunnel ile taşınır; winws'in canlı işi Roblox + genel HTTPS SNI-desync'i.

use crate::pid_scan::ExclusionPorts;

/// Motor sınıfı (docs/07 §3 `EngineKind`). Orkestrasyon hangi sistem kaynağının gerektiğini buradan bilir.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineKind {
    /// Paket-manipülasyonu (DPI desync) — WinDivert sürücüsü gerektirir, VPN değil, ek gecikme yok.
    Desync,
    /// Yerel SOCKS5 proxy (ByeDPI/ciadpi) — kernel-siz; WinDivert engelliyken (Kaspersky) tek seçenek.
    LocalProxy,
    /// Şifreli tünel (WireGuard/WARP) — trafiği taşır; DPI içeriği göremez.
    Tunnel,
}

/// Motor yetenekleri (docs/07 §3 `EngineCapabilities`). UI gri-leştirme + preflight buna bakar.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct EngineCaps {
    /// Uygulama/IP başı seçici çalıştırma destekler mi (split-tunnel / per-app)?
    pub split_tunnel: bool,
    /// WinDivert çekirdek sürücüsü gerektirir mi (AV/Kaspersky kontrolü için)?
    pub requires_windivert: bool,
    /// Çekirdek-sürücü-siz mi (Kaspersky/AV ortamında çalışabilen tek tür)?
    pub kernel_less: bool,
}

/// Motor kataloğu girdisi (UI'ye dönen meta — hangi motorlar var, hangisi kullanılabilir).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EngineInfo {
    pub id: String,
    pub name: String,
    pub kind: EngineKind,
    pub caps: EngineCaps,
    /// Bu motorun binary'si bu kurulumda mevcut mu (bundle var mı)? UI gri-leştirir.
    pub available: bool,
}

/// Bir DPI-desync stratejisinin tipli modeli (winws parametrelerine birebir). docs/07 §3: "strateji = veri".
/// Örn c1: `--dpi-desync=fake,multidisorder --dpi-desync-split-pos=1,midsld --dpi-desync-repeats=11
///         --dpi-desync-fooling=md5sig --dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com`
#[derive(Clone, Debug)]
pub struct Strategy {
    pub id: &'static str,
    /// desync method(s): fake | split2 | disorder2 | multisplit | multidisorder (comma-joined).
    pub desync: &'static str,
    /// split position(s), e.g. "1,midsld" or "2". Empty = omit.
    pub split_pos: &'static str,
    /// repeat count for the fake/desync packet. 0 = omit.
    pub repeats: u32,
    /// fooling method: md5sig | badseq | badsum | datanoack | ts | hopbyhop | none. "none"/"" = omit.
    pub fooling: &'static str,
    /// fake TLS ClientHello modifier, e.g. "rnd,dupsid,sni=www.google.com". Empty = omit.
    pub fake_tls_mod: &'static str,
    /// fixed fake-packet TTL (--dpi-desync-ttl). 0 = omit (use fooling or autottl instead).
    pub ttl: u32,
    /// auto TTL fallback (--dpi-desync-autottl). 0 = omit.
    pub autottl: u32,
    /// sequence-overlap bytes (--dpi-desync-split-seqovl). 0 = omit.
    pub seqovl: u32,
    /// WinDivert TCP capture ports (--wf-tcp). Consumed by the capture-filter builder (item 1.4).
    pub wf_tcp: &'static str,
    /// WinDivert UDP capture ports incl. Discord voice (--wf-udp). Consumed in item 1.4.
    pub wf_udp: &'static str,
    /// fake QUIC initial payload (.bin, relative to bundle). Consumed by the QUIC stage (item 1.4).
    pub fake_quic: &'static str,
    /// apply only to hostlist domains instead of catch-all (--hostlist). Wired in item 1.3.
    pub hostlist_only: bool,
    /// LAYERED MODE (2026-08-16). Id of a HARMLESS catch-all chain applied to everything the
    /// hostlist did NOT match, emitted as a lower-priority winws profile after the gated ones.
    /// Empty = no fallback layer (pure hostlist gating, or pure catch-all).
    ///
    /// This exists because of a real, user-reported failure: "Güçlü Koruma" ran ONE route-dependent
    /// chain (`fake` + `ttl=1` + `autottl=3`, no fooling) catch-all over every TLS/443 flow. On the
    /// line it was tuned against that opened the hard domains; on a different line the forged
    /// ClientHello outlives the DPI hop, reaches the real server, and the server kills the
    /// connection — so protection BROKE a site that needed no bypass at all, while "Hafif" (which
    /// simply never touched that domain) worked. A strategy that can make things worse than off is
    /// not a stronger strategy.
    ///
    /// The fix is layering, not tuning: the aggressive chain is gated to domains known to need it,
    /// and everything else gets a chain that cannot corrupt a connection (see `is_harmless`).
    pub fallback: &'static str,
}

impl Strategy {
    /// Build the per-profile TLS desync flags for `--filter-tcp=443` from the typed fields (item 1.1).
    /// Emits only the fields that are set, so the default "c1" reproduces the live-verified primary
    /// stage byte-for-byte. Does NOT include the trailing `--new` separator (the caller adds it).
    pub fn tls_profile_args(&self) -> Vec<String> {
        // The "off" chain has no desync method — emitting `--dpi-desync=` would be a malformed flag
        // winws rejects at startup. No method means no stage at all, which is the point of "off".
        if self.desync.is_empty() {
            return Vec::new();
        }
        let mut a = vec![
            "--filter-tcp=443".to_string(),
            format!("--dpi-desync={}", self.desync),
        ];
        if !self.split_pos.is_empty() {
            a.push(format!("--dpi-desync-split-pos={}", self.split_pos));
        }
        if self.repeats > 0 {
            a.push(format!("--dpi-desync-repeats={}", self.repeats));
        }
        if !self.fooling.is_empty() && self.fooling != "none" {
            a.push(format!("--dpi-desync-fooling={}", self.fooling));
        }
        if self.ttl > 0 {
            a.push(format!("--dpi-desync-ttl={}", self.ttl));
        }
        if self.autottl > 0 {
            a.push(format!("--dpi-desync-autottl={}", self.autottl));
        }
        if self.seqovl > 0 {
            a.push(format!("--dpi-desync-split-seqovl={}", self.seqovl));
        }
        if !self.fake_tls_mod.is_empty() {
            a.push(format!("--dpi-desync-fake-tls-mod={}", self.fake_tls_mod));
        }
        a
    }

    /// Can this chain damage a connection to a server that needs no bypass at all?
    ///
    /// The distinction is whether a forged packet can survive as far as the real server:
    ///
    ///  * NO forged packet at all (`multisplit`/`multidisorder`/`split2`/`disorder2` alone) — the
    ///    ClientHello is only segmented/reordered at the TCP layer. Every TLS server reassembles
    ///    TCP, so this is transparent to the endpoint. Harmless.
    ///  * A forged packet INVALIDATED by a fooling option (`md5sig`, `badseq`, `badsum`) — the real
    ///    server drops it (bad TCP-MD5 option / out-of-window sequence / bad checksum) while the
    ///    DPI, which does not validate, swallows it. Harmless to the endpoint.
    ///  * A forged packet kept valid and merely TTL-limited (`--dpi-desync-ttl` / `autottl` with no
    ///    fooling) — correctness depends entirely on the DPI being exactly the assumed number of
    ///    hops away. Guess wrong and the forgery reaches the server, which then tears the
    ///    connection down. NOT harmless: this is precisely what broke Güçlü Koruma.
    ///
    /// Only harmless chains are ever allowed to run catch-all over traffic nobody asked us to fix.
    pub fn is_harmless(&self) -> bool {
        let forges = self.desync.split(',').any(|m| m.trim() == "fake");
        if !forges {
            return true;
        }
        // A forged packet is safe for the endpoint only if a fooling option guarantees the real
        // server discards it (bad TCP-MD5 option, out-of-window sequence, bad checksum). Without
        // one, the forgery's fate depends on TTL guessing.
        !self.fooling.is_empty() && self.fooling != "none"
    }
}

/// Bilinen stratejiler. "auto" = otomatik bulucu (autopilot.rs); kataloğun ilki güvenli başlangıç.
/// NOTE: the generic strategies keep `wf_udp = ""` → the proven raw-part signature capture only
/// (no explicit `--wf-udp`), so the default catch-all stays byte-for-byte the live-verified config.
/// ISP presets (see `presets()`) carry explicit `wf_udp` port ranges (SplitWire-style capture).
pub fn strategies() -> &'static [Strategy] {
    const QUIC: &str = r"files\quic_initial_www_google_com.bin";
    &[
        Strategy {
            id: "c1",
            desync: "fake,multidisorder",
            split_pos: "1,midsld",
            repeats: 11,
            fooling: "md5sig",
            fake_tls_mod: "rnd,dupsid,sni=www.google.com",
            ttl: 0,
            autottl: 0,
            seqovl: 0,
            wf_tcp: "80,443",
            wf_udp: "",
            fake_quic: QUIC,
            hostlist_only: false,
            fallback: SAFE_FALLBACK,
        },
        Strategy {
            id: "multidisorder",
            desync: "multidisorder",
            split_pos: "1,midsld",
            repeats: 6,
            fooling: "none",
            fake_tls_mod: "",
            ttl: 0,
            autottl: 0,
            seqovl: 0,
            wf_tcp: "80,443",
            wf_udp: "",
            fake_quic: QUIC,
            hostlist_only: false,
            fallback: SAFE_FALLBACK,
        },
        Strategy {
            id: "fake",
            desync: "fake",
            split_pos: "1",
            repeats: 11,
            fooling: "md5sig",
            fake_tls_mod: "rnd,dupsid,sni=www.google.com",
            ttl: 0,
            autottl: 0,
            seqovl: 0,
            wf_tcp: "80,443",
            wf_udp: "",
            fake_quic: QUIC,
            hostlist_only: false,
            fallback: SAFE_FALLBACK,
        },
        // ---- Harmless chains (see Strategy::is_harmless) -------------------------------------
        // These forge nothing, or forge only packets the real server is guaranteed to drop. They
        // are the ONLY chains allowed to run catch-all over traffic the user never asked us to fix.
        //
        // "safe-split": pure TCP-layer segmentation of the ClientHello. No forged packet exists at
        // all, so there is nothing that can reach a server and confuse it — the endpoint just
        // reassembles TCP as it always does. This is the default fallback layer.
        Strategy {
            id: "safe-split",
            desync: "multisplit",
            split_pos: "1,midsld",
            repeats: 0,
            fooling: "",
            fake_tls_mod: "",
            ttl: 0,
            autottl: 0,
            seqovl: 0,
            wf_tcp: "80,443",
            wf_udp: "",
            fake_quic: QUIC,
            hostlist_only: false,
            fallback: "",
        },
        // Sequence-overlap split: the first segment deliberately overlaps the second, so a DPI that
        // reassembles naively sees a different stream than the server does. Still no forgery.
        Strategy {
            id: "safe-seqovl",
            desync: "multisplit",
            split_pos: "1",
            repeats: 0,
            fooling: "",
            fake_tls_mod: "",
            ttl: 0,
            autottl: 0,
            seqovl: 652,
            wf_tcp: "80,443",
            wf_udp: "",
            fake_quic: QUIC,
            hostlist_only: false,
            fallback: "",
        },
        // Forged ClientHello carrying a bogus TCP-MD5 signature option: DPI accepts it, the real
        // server rejects it outright (RFC 2385 validation), so the endpoint is unaffected.
        Strategy {
            id: "safe-fake",
            desync: "fake,multisplit",
            split_pos: "1,midsld",
            repeats: 6,
            fooling: "md5sig",
            fake_tls_mod: "rnd,dupsid,sni=www.google.com",
            ttl: 0,
            autottl: 0,
            seqovl: 0,
            wf_tcp: "80,443",
            wf_udp: "",
            fake_quic: QUIC,
            hostlist_only: false,
            fallback: "",
        },
        // Same idea via an out-of-window sequence number instead of md5sig — some DPI boxes track
        // MD5 options but not sequence windows, and vice versa, so both are worth racing.
        Strategy {
            id: "safe-badseq",
            desync: "fake,multidisorder",
            split_pos: "1,midsld",
            repeats: 6,
            fooling: "badseq",
            fake_tls_mod: "rnd,dupsid,sni=www.google.com",
            ttl: 0,
            autottl: 0,
            seqovl: 0,
            wf_tcp: "80,443",
            wf_udp: "",
            fake_quic: QUIC,
            hostlist_only: false,
            fallback: "",
        },
        // "off": no desync stage at all. NOT dead weight — on a line where the block is purely
        // DNS-based, secure DNS alone opens everything and the correct engine action is to touch
        // nothing. The tuner races this first, so "do nothing" can win on merit instead of the app
        // mangling packets for no reason. (Measured on the user's line: the site the aggressive
        // catch-all broke opened instantly when nothing touched it.)
        Strategy {
            id: "off",
            desync: "",
            split_pos: "",
            repeats: 0,
            fooling: "",
            fake_tls_mod: "",
            ttl: 0,
            autottl: 0,
            seqovl: 0,
            wf_tcp: "80,443",
            wf_udp: "",
            fake_quic: QUIC,
            hostlist_only: false,
            fallback: "",
        },
    ]
}

/// The chain used for everything the aggressive hostlist did not match. Must be harmless.
pub const SAFE_FALLBACK: &str = "safe-split";

/// Candidate ladder for the per-line tuner, ORDERED LEAST-INVASIVE FIRST.
///
/// The order is the policy: the first candidate that opens the targets wins, so the engine
/// converges on the gentlest chain that works rather than the most aggressive one available. TTL
/// chains sit at the bottom because they are the only ones that can damage a working connection
/// (see `Strategy::is_harmless`) — they are tried, but only after everything safer has failed.
/// FIRST PASS: one representative per bypass MECHANISM, still least-invasive first.
///
/// Measuring all 15 candidates cost 91 seconds on the user's line (measured 2026-08-16), because
/// every candidate needs its own winws start and its own probe and they cannot run concurrently —
/// they would fight over the single global WinDivert driver.
///
/// Most of that time is spent distinguishing variants of a mechanism that has already been shown
/// not to work on this line: if plain `fake`+md5sig opens nothing, its repeat-count and split-pos
/// siblings almost certainly will not either. So the first pass tries one of each KIND, and the
/// remaining variants are only measured if the first pass fails to find something good enough.
pub fn tuner_ladder_first_pass() -> &'static [&'static str] {
    &[
        "off",               // do nothing
        "safe-split",        // pure TCP segmentation
        "safe-fake",         // forged + md5sig (server rejects it)
        "c1",                // forged + multidisorder + md5sig
        "superonline",       // forged + md5sig, no split
        "tt-alt",            // bare TTL (route-dependent — first of the risky kind)
        "turkcell-hotspot",  // bare TTL + autottl
    ]
}

/// Everything else, tried only when the first pass did not find a good enough chain.
pub fn tuner_ladder_second_pass() -> &'static [&'static str] {
    &["safe-seqovl", "safe-badseq", "multidisorder", "fake", "superonline-alt", "vodafone-hotspot", "tt", "kablonet"]
}

pub fn tuner_ladder() -> &'static [&'static str] {
    &[
        "off",
        "safe-split",
        "safe-seqovl",
        "safe-fake",
        "safe-badseq",
        "c1",
        "multidisorder",
        "fake",
        "superonline",
        // superonline-alt sets a TTL *and* md5sig fooling. The fooling is what decides safety: the
        // real server rejects the forged segment on its bad MD5 option whatever its TTL was. It
        // therefore belongs up here with the harmless chains, not down among the bare-TTL ones —
        // a unit test enforces exactly this ordering, and caught it sitting in the wrong place.
        "superonline-alt",
        "vodafone-hotspot",
        // ---- Below this line: bare TTL chains, no fooling. These are the only candidates that can
        // damage a connection to a server that needed no bypass, so they are tried last and only
        // ever deployed after measurement proved they work on THIS line.
        "tt-alt",
        "tt",
        "kablonet",
        "turkcell-hotspot",
    ]
}

/// Per-ISP presets (docs/03 §4.1 — SplitWire `Resources/zapret/zapret-winws/presets.txt`, verbatim).
/// These tune the PRIMARY desync (fake/multisplit + ttl/autottl/fooling) per Turkish ISP; `wf_tcp`/
/// `wf_udp` carry each ISP's capture ports (incl. Discord voice 50000-50100) for item 1.4. Selectable
/// by id through profiles (ApplyProfile → strategy_by_id resolves presets too).
pub fn presets() -> &'static [Strategy] {
    const QUIC: &str = r"files\quic_initial_www_google_com.bin";
    &[
        // Türk Telekom: fake + ttl 4
        Strategy { id: "tt", desync: "fake", split_pos: "", repeats: 0, fooling: "", fake_tls_mod: "",
            ttl: 4, autottl: 0, seqovl: 0, wf_tcp: "80,443", wf_udp: "443,50000,50100", fake_quic: QUIC, hostlist_only: false, fallback: SAFE_FALLBACK },
        // Türk Telekom Alternatif: fake + ttl 3
        Strategy { id: "tt-alt", desync: "fake", split_pos: "", repeats: 0, fooling: "", fake_tls_mod: "",
            ttl: 3, autottl: 0, seqovl: 0, wf_tcp: "80,443", wf_udp: "443,50000,50100", fake_quic: QUIC, hostlist_only: false, fallback: SAFE_FALLBACK },
        // SuperOnline: fake + md5sig fooling
        Strategy { id: "superonline", desync: "fake", split_pos: "", repeats: 0, fooling: "md5sig", fake_tls_mod: "",
            ttl: 0, autottl: 0, seqovl: 0, wf_tcp: "80,443", wf_udp: "443,50000,50100", fake_quic: QUIC, hostlist_only: false, fallback: SAFE_FALLBACK },
        // SuperOnline Alternatif: fake + md5sig + ttl 3 (voice port range 50000-50099)
        Strategy { id: "superonline-alt", desync: "fake", split_pos: "", repeats: 0, fooling: "md5sig", fake_tls_mod: "",
            ttl: 3, autottl: 0, seqovl: 0, wf_tcp: "80,443", wf_udp: "443,50000-50099", fake_quic: QUIC, hostlist_only: false, fallback: SAFE_FALLBACK },
        // Kablonet: fake + ttl 4
        Strategy { id: "kablonet", desync: "fake", split_pos: "", repeats: 0, fooling: "", fake_tls_mod: "",
            ttl: 4, autottl: 0, seqovl: 0, wf_tcp: "80,443", wf_udp: "443,50000,50100", fake_quic: QUIC, hostlist_only: false, fallback: SAFE_FALLBACK },
        // Turkcell Hotspot: fake + ttl 1 + autottl 3
        Strategy { id: "turkcell-hotspot", desync: "fake", split_pos: "", repeats: 0, fooling: "", fake_tls_mod: "",
            ttl: 1, autottl: 3, seqovl: 0, wf_tcp: "80,443", wf_udp: "443,50000,50100", fake_quic: QUIC, hostlist_only: false, fallback: SAFE_FALLBACK },
        // Vodafone Hotspot: multisplit at pos 2 (no fake/ttl)
        Strategy { id: "vodafone-hotspot", desync: "multisplit", split_pos: "2", repeats: 0, fooling: "", fake_tls_mod: "",
            ttl: 0, autottl: 0, seqovl: 0, wf_tcp: "80,443", wf_udp: "443,50000,50100", fake_quic: QUIC, hostlist_only: false, fallback: SAFE_FALLBACK },
    ]
}

/// id → strategy. Searches generic strategies first, then ISP presets. "auto"/unknown → c1
/// (the live-verified starting preset, docs/05 §7).
pub fn strategy_by_id(id: &str) -> Strategy {
    strategies()
        .iter()
        .chain(presets().iter())
        .find(|s| s.id == id)
        .cloned()
        .unwrap_or_else(|| strategies()[0].clone())
}

/// Tüm atlatma motoru adaptörlerinin ortak sözleşmesi (docs/07 §3 `IBypassEngine`). Orkestrasyon
/// (service.rs) Start/Stop/strateji değişiminde bunu çağırır; UI motoru id ile seçer (profil.engine).
pub trait BypassEngine: Send {
    /// Kararlı motor kimliği ("zapret" | "byedpi" | "goodbyedpi" | "warp" | "sim").
    fn id(&self) -> &'static str;
    fn kind(&self) -> EngineKind;
    fn caps(&self) -> EngineCaps;
    /// Bu motorun binary'si mevcut mu (bundle var mı)? Yoksa start() sim no-op'a iner (boot bozulmaz).
    fn is_available(&self) -> bool;
    /// Profilden gerçek komut satırını üret — UI'de canlı gösterilebilir (docs/06 §1.4 "hiçbir sihir gizli değil").
    /// Süreç başlatmaz; yalnız argümanları döndürür (kopyalanabilir komut satırı).
    fn build_args(&self, strategy: &Strategy, hostlist: &[String]) -> Vec<String>;
    /// Stratejiyi + hostlist'i uygula (süreç başlat / sürücü aç). idempotent olmalı (zaten canlıysa no-op).
    fn start(&mut self, strategy: &Strategy, hostlist: &[String]) -> Result<(), String>;
    /// Durdur (alt-süreci öldür).
    fn stop(&mut self);
    fn is_running(&self) -> bool;
    /// Per-app İNDİRME (ingress) hız limiti — winws sidecar'ında no-op (egress QoS ayrı yoldan, system::limit).
    fn set_limits(&mut self, _limits: &[(String, u32)]) {}
    /// "Off" modlu uygulamaların PID source port'ları — WinDivert capture filter'ından hariç tutulur.
    /// Liste değişince motor kendini yeniden başlatabilir. Boş → catch-all (regresyon yok).
    fn set_exclusion(&mut self, _excl: &ExclusionPorts) {}
    /// Would `set_exclusion(excl)` actually change anything (i.e. cause a RESTART)?
    ///
    /// Lets the watchdog rate-limit exclusion-driven restarts without having to guess: applying an
    /// unchanged set is free, applying a changed one costs every live connection. Default false so
    /// engines that ignore exclusions never report a spurious change.
    fn exclusion_differs(&self, _excl: &ExclusionPorts) -> bool {
        false
    }
}

/// Aktif motoru id ile üret (factory, docs/07 §3). Bilinmeyen id / Windows dışı → SimEngine.
/// "zapret"/"auto"/"" → winws (varsayılan). Tünel (warp) AYRI yönetilir (WarpEngine, service.rs).
pub fn make_engine(id: &str) -> Box<dyn BypassEngine> {
    match id {
        #[cfg(windows)]
        "byedpi" => Box::new(crate::byedpi::ByeDpiEngine::new()),
        #[cfg(windows)]
        "byedpi-proxifyre" => Box::new(crate::byedpi::ByeDpiEngine::with_routing(
            crate::byedpi::Routing::ProxiFyre { browsers: false },
        )),
        #[cfg(windows)]
        "byedpi-drover" => Box::new(crate::byedpi::ByeDpiEngine::with_routing(crate::byedpi::Routing::Drover)),
        #[cfg(windows)]
        "goodbyedpi" => Box::new(crate::goodbyedpi::GoodbyeDpiEngine::new()),
        #[cfg(windows)]
        "zapret" | "auto" | "" => Box::new(WinwsEngine::new()),
        #[cfg(windows)]
        _ => Box::new(WinwsEngine::new()),
        #[cfg(not(windows))]
        _ => Box::new(SimEngine::new(id_to_static(id))),
    }
}

/// Like `make_engine` but applies a profile's `engine_params` (item 7.2).
///
/// For **goodbyedpi**: `{"preset":"<id>"}` applies a named preset; individual fields
/// `"mode"` (1-9, u8), `"set_ttl"` (u32), `"dns_redirect"` ("cloudflare"|"yandex"|null)
/// override after the preset (or standalone when no preset is given).
///
/// For **byedpi**: `{"preset":"<id>"}` selects a preset; individual fields `"split"`,
/// `"disorder"`, `"fake"`, `"mod_http"`, `"tlsrec"`, `"auto"` (strings or null),
/// `"ttl"` (u32), `"oob"` (bool) override after the preset. `"browsers"` (bool) controls
/// ProxiFyre browser-routing when the engine id is `byedpi-proxifyre`.
///
/// Keeps the concrete-type downcast inside the factory (the trait object can't `apply_preset`).
pub fn make_engine_with_params(id: &str, params: &serde_json::Value) -> Box<dyn BypassEngine> {
    let preset = params.get("preset").and_then(|v| v.as_str());
    #[cfg(windows)]
    {
        match id {
            "goodbyedpi" => {
                let mut e = crate::goodbyedpi::GoodbyeDpiEngine::new();
                // 1. Apply named preset first (may be overridden below).
                if let Some(pid) = preset {
                    if let Some(p) = crate::goodbyedpi::presets().into_iter().find(|p| p.id == pid) {
                        e.apply_preset(&p);
                    }
                }
                // 2. Override individual fields if present.
                if let Some(m) = params.get("mode").and_then(|v| v.as_u64()) {
                    e.set_mode(crate::goodbyedpi::GdMode::from_u8(m as u8));
                }
                if let Some(ttl) = params.get("set_ttl").and_then(|v| v.as_u64()) {
                    e.set_ttl_val(ttl as u32);
                }
                match params.get("dns_redirect").and_then(|v| v.as_str()) {
                    Some("cloudflare") => e.set_dns_redirect(Some(crate::goodbyedpi::DnsRedirect::cloudflare())),
                    Some("yandex") => e.set_dns_redirect(Some(crate::goodbyedpi::DnsRedirect::yandex_nonstandard())),
                    Some("none") | Some("off") => e.set_dns_redirect(None),
                    _ => {}
                }
                Box::new(e)
            }
            "byedpi" | "byedpi-proxifyre" | "byedpi-drover" => {
                let browsers = params.get("browsers").and_then(|v| v.as_bool()).unwrap_or(false);
                let routing = match id {
                    "byedpi-proxifyre" => crate::byedpi::Routing::ProxiFyre { browsers },
                    "byedpi-drover" => crate::byedpi::Routing::Drover,
                    _ => crate::byedpi::Routing::None,
                };
                let mut e = crate::byedpi::ByeDpiEngine::with_routing(routing);
                // 1. Apply named preset first.
                if let Some(pid) = preset {
                    if let Some(p) = crate::byedpi::presets().into_iter().find(|p| p.id == pid) {
                        e.apply_preset(&p);
                    }
                }
                // 2. Override individual ByeDPI config fields if present.
                let mut cfg = e.config().clone();
                macro_rules! opt_str {
                    ($field:ident, $key:expr) => {
                        if params.get($key).is_some() {
                            cfg.$field = params[$key].as_str().map(|s| s.to_string());
                        }
                    };
                }
                opt_str!(split,    "split");
                opt_str!(disorder, "disorder");
                opt_str!(fake,     "fake");
                opt_str!(mod_http, "mod_http");
                opt_str!(tlsrec,   "tlsrec");
                opt_str!(auto,     "auto");
                if let Some(ttl) = params.get("ttl").and_then(|v| v.as_u64()) {
                    cfg.ttl = ttl as u32;
                }
                if let Some(oob) = params.get("oob").and_then(|v| v.as_bool()) {
                    cfg.oob = oob;
                }
                if let Some(port) = params.get("port").and_then(|v| v.as_u64()) {
                    cfg.port = port as u16;
                }
                e.set_config(cfg);
                Box::new(e)
            }
            _ => make_engine(id),
        }
    }
    #[cfg(not(windows))]
    {
        make_engine(id)
    }
}

/// Tüm bilinen DPI motorlarının kataloğu + bu kurulumda kullanılabilirlik (UI motor seçici + preflight).
/// Tünel (WARP) DPI motoru değil → katalogda DEĞİL (Tam Koruma / per-app modlarıyla ayrı yönetilir).
pub fn catalog() -> Vec<EngineInfo> {
    let mut out = Vec::new();
    for id in ["zapret", "byedpi", "byedpi-proxifyre", "byedpi-drover", "goodbyedpi"] {
        let e = make_engine(id);
        out.push(EngineInfo {
            id: e.id().to_string(),
            name: engine_display_name(e.id()).to_string(),
            kind: e.kind(),
            caps: e.caps(),
            available: e.is_available(),
        });
    }
    out
}

fn engine_display_name(id: &str) -> &'static str {
    match id {
        "zapret" => "Zapret (winws · DPI desync)",
        "byedpi" => "ByeDPI (SOCKS5 · kernel-less)",
        "byedpi-proxifyre" => "ByeDPI + ProxiFyre (split tunnel)",
        "byedpi-drover" => "ByeDPI + drover (Discord only)",
        "goodbyedpi" => "GoodbyeDPI (DPI desync + DNS)",
        _ => "Unknown engine",
    }
}

#[cfg(not(windows))]
fn id_to_static(id: &str) -> &'static str {
    match id {
        "byedpi" => "byedpi",
        "byedpi-proxifyre" => "byedpi-proxifyre",
        "byedpi-drover" => "byedpi-drover",
        "goodbyedpi" => "goodbyedpi",
        _ => "zapret",
    }
}

// ============================================================================
// SimEngine — Windows dışı / dev no-op. Gerçek paket müdahalesi YOK; UI↔IPC↔motor zincirini çalıştırır.
// ============================================================================
pub struct SimEngine {
    id: &'static str,
    running: bool,
}

impl SimEngine {
    pub fn new(id: &'static str) -> Self {
        Self { id, running: false }
    }
}

impl Default for SimEngine {
    fn default() -> Self {
        Self::new("sim")
    }
}

impl BypassEngine for SimEngine {
    fn id(&self) -> &'static str {
        self.id
    }
    fn kind(&self) -> EngineKind {
        EngineKind::Desync
    }
    fn caps(&self) -> EngineCaps {
        EngineCaps { split_tunnel: false, requires_windivert: false, kernel_less: true }
    }
    fn is_available(&self) -> bool {
        true
    }
    fn build_args(&self, strategy: &Strategy, hostlist: &[String]) -> Vec<String> {
        vec![
            format!("--sim-engine={}", self.id),
            format!("--strategy={}", strategy.id),
            format!("--hostlist={}", hostlist.len()),
        ]
    }
    fn start(&mut self, strategy: &Strategy, hostlist: &[String]) -> Result<(), String> {
        eprintln!(
            "[evorift-svc][sim-engine:{}] start strategy={} desync={} ({} domain)",
            self.id, strategy.id, strategy.desync, hostlist.len()
        );
        self.running = true;
        Ok(())
    }
    fn stop(&mut self) {
        eprintln!("[evorift-svc][sim-engine:{}] stop", self.id);
        self.running = false;
    }
    fn is_running(&self) -> bool {
        self.running
    }
}

// ============================================================================
// winws (zapret · MIT, github.com/bol-van/zapret) SIDECAR — VARSAYILAN motor.
// Kanıtlanmış catch-all zapret stratejisi: TÜM TCP/443 + QUIC + Discord ses (Türkcell CANLI 2026-06-08).
// Bundle: evorift-svc.exe yanındaki `winws\` klasörü. Job Object: servis ölünce kernel winws'i öldürür.
// ============================================================================
/// Eski/yabancı WinDivert ÇEKİRDEK SÜRÜCÜSÜNÜ temizle. Windows tek global WinDivert.sys yükler;
/// farklı sürüm yüklüyse winws anında ölür.
///
/// Sıra: DISABLED ise demand-start'a al → DURDUR → STOPPED yokla → HER DURUMDA sil.
///
/// REVİZE (2026-08-14, canlı test): eskiden yalnız STOPPED doğrulanırsa siliyordu. Sürücüyü açık
/// tutan canlı bir winws.exe varken `sc stop` çalışmaz → silme hiç denenmezdi → bozuk kayıt HER
/// yeniden başlatmada hayatta kalır ve motoru kalıcı olarak çalışmaz hale getirir. Gerçekte
/// görülen hal buydu: iki gün önceki bir debug ağacından kalma, START_TYPE=DISABLED bir kayıt.
/// Çalışırken silmek DELETE_PENDING (1072) üretir ve reboot'ta temizlenir — kullanılamaz bir kaydı
/// süresiz bırakmaktan iyidir. Silmek her hâlükârda güvenli: winws sonraki başlatmada kendi
/// bundle'ından doğru yol + doğru start type ile yeniden kaydeder. Best-effort.
///
/// Module-level and `pub` because the remote test agent (`testd::recovery`) fires the exact same
/// cleanup from its deadman switch. One implementation, two callers — a second copy over there
/// would drift from this one the moment a service name is added.
#[cfg(windows)]
pub fn clear_stale_windivert() {
    use std::os::windows::process::CommandExt;
    let sc = |args: &[&str]| -> String {
        std::process::Command::new("sc")
            .args(args)
            .creation_flags(0x0800_0000)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    for name in ["WinDivert", "WinDivert1.4", "WinDivert1.1"] {
        if !sc(&["query", name]).contains("STATE") {
            continue;
        }
        // A registration left at START_TYPE=DISABLED can never be started, so winws can never load
        // it — and `sc stop` on a disabled service does nothing useful either. Put it back to
        // demand-start FIRST so the stop below can actually take effect. Seen in the wild: a stale
        // dev-tree registration stuck DISABLED made every winws launch fail instantly.
        let _ = sc(&["config", name, "start=", "demand"]);
        // Best-effort: a stop that fails is still followed by the STOPPED poll below, which is
        // what actually gates the delete — the stop's own exit code adds nothing.
        let _ = sc(&["stop", name]);
        let mut stopped = false;
        for _ in 0..5 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let q = sc(&["query", name]);
            if !q.contains("STATE") || q.contains("STOPPED") {
                stopped = true;
                break;
            }
        }
        // Delete even when the stop did NOT succeed. A loaded driver with a live handle-holder
        // cannot be stopped, and the previous "only delete if stopped" rule meant the bad
        // registration survived every restart — the exact state that bricks the engine. `sc delete`
        // on a running driver marks it for deletion (error 1072) and it goes away on reboot, which
        // still beats leaving a permanently unusable entry in place. Deleting is safe regardless:
        // winws recreates the service from its own bundle on next start.
        let out = sc(&["delete", name]);
        if !stopped && !out.contains("SUCCESS") {
            eprintln!(
                "[evorift][windivert] '{name}' kaydi temizlenemedi (surec handle tutuyor olabilir); \
                 winws yuklenemeyebilir — scripts/FIX-WINDIVERT.bat yonetici olarak calistirilmali"
            );
        }
    }
}

/// Non-Windows stub — WinDivert is a Windows kernel driver; there is nothing to clear elsewhere.
#[cfg(not(windows))]
pub fn clear_stale_windivert() {}

#[cfg(windows)]
pub struct WinwsEngine {
    child: Option<std::process::Child>,
    /// Job Object handle (0=yok), isize (HANDLE Send değil; trait Send gerektirir). KILL_ON_JOB_CLOSE:
    /// servis ölünce/çökünce kernel atanan winws.exe'yi öldürür → yetim SYSTEM süreci kalmaz.
    job: isize,
    /// Uygulanmış "off" exclusion port'ları. Aynı liste → no-op; değişirse winws restart (filter hot-reload yok).
    excl: ExclusionPorts,
}

#[cfg(windows)]
impl Default for WinwsEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
impl WinwsEngine {
    pub fn new() -> Self {
        Self { child: None, job: 0, excl: ExclusionPorts::default() }
    }

    /// Bundle dizini: `<exe_dizini>\winws\` (EXE-relative; cwd DEĞİL — paketlenince cwd değişir, net3 §4.2).
    pub fn bundle_dir() -> Option<std::path::PathBuf> {
        Some(std::env::current_exe().ok()?.parent()?.join("winws"))
    }

    /// winws.exe bundle'da var mı? (is_available + start sim-fallback kararı için).
    fn exe_exists() -> bool {
        Self::bundle_dir().map(|d| d.join("winws.exe").exists()).unwrap_or(false)
    }

    /// Ensure the kill-on-close job exists (lazy, once per engine lifetime).
    fn ensure_job(&mut self) -> isize {
        if self.job == 0 {
            self.job = crate::proc::create_kill_on_close_job();
        }
        self.job
    }

    fn assign_to_job(&mut self, child: &std::process::Child) {
        let job = self.ensure_job();
        crate::proc::assign_to_job(job, child);
    }

    /// Kill all running winws.exe instances (single-instance + orphan cleanup; WinDivert conflict prevention).
    fn kill_all() {
        crate::proc::kill_image("winws.exe");
    }

    /// Where winws's own stdout/stderr goes.
    ///
    /// It went NOWHERE before: the child was spawned with default (inherited) stdio from a Session-0
    /// SYSTEM service that has no console, so every message winws printed about why it refused to
    /// start — bad filter syntax, missing .bin, WinDivert load failure — was discarded by the OS.
    /// That is the single biggest reason engine failures were undiagnosable from the outside.
    pub fn log_path() -> std::path::PathBuf {
        crate::sys::log_dir().join("winws.log")
    }

    /// Spawn winws with stdout+stderr captured to `log_path()`.
    ///
    /// Capture is best-effort by design: if the log file cannot be opened (locked, disk full) the
    /// engine still starts with inherited stdio rather than refusing to protect the user because
    /// logging is unavailable. The failure to capture is itself logged.
    fn spawn_winws(exe: &std::path::Path, argv: &[String]) -> std::io::Result<std::process::Child> {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(exe);
        cmd.args(argv).creation_flags(crate::proc::CREATE_NO_WINDOW).stdin(std::process::Stdio::null());
        let path = Self::log_path();
        let _ = std::fs::create_dir_all(crate::sys::log_dir());
        // Roll the capture file before each start so the tail we surface on failure belongs to THIS
        // launch, not to a run from last week. 256 KiB is far more than winws ever emits per run.
        if std::fs::metadata(&path).map(|m| m.len() > 256 * 1024).unwrap_or(false) {
            let _ = std::fs::remove_file(&path);
        }
        match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => match f.try_clone() {
                Ok(f2) => {
                    cmd.stdout(std::process::Stdio::from(f)).stderr(std::process::Stdio::from(f2));
                }
                Err(e) => crate::elog::warn("engine", "log_clone", &format!("winws log handle: {e}")),
            },
            Err(e) => crate::elog::warn(
                "engine",
                "log_open",
                &format!("winws output could not be captured ({}): {e}", path.display()),
            ),
        }
        cmd.spawn()
    }

    /// Wait up to `budget` for the child to prove it is staying alive.
    /// Returns `Some(exit_code)` if it died inside the window, `None` if it is still running.
    fn settle(child: &mut std::process::Child, budget: std::time::Duration) -> Option<i32> {
        const STEP: std::time::Duration = std::time::Duration::from_millis(25);
        let deadline = std::time::Instant::now() + budget;
        loop {
            match child.try_wait() {
                Ok(Some(st)) => return Some(st.code().unwrap_or(-1)),
                Ok(None) => {}
                // A handle we cannot query is not evidence of death; let the watchdog handle it.
                Err(_) => return None,
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(STEP);
        }
    }

    /// Last few lines winws wrote — the actual reason it refused to run, surfaced into the error
    /// string the UI shows instead of a bare "winws exited".
    fn log_tail() -> String {
        let s = std::fs::read_to_string(Self::log_path()).unwrap_or_default();
        let tail: Vec<&str> = s.lines().rev().filter(|l| !l.trim().is_empty()).take(4).collect();
        if tail.is_empty() {
            "(engine produced no output)".to_string()
        } else {
            tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
        }
    }

    /// "Off" exclusion port'ları varsa winws WinDivert capture filter'ını yeniden derle → o uygulamaların
    /// paketleri winws'e ulaşmaz. Tüm windivert.filter/* parçaları + TCP/80,443 + exclusion clause →
    /// ProgramData'ya yazılır. Boş exclusion → None (catch-all `--wf-tcp` + `--wf-raw-part`).
    fn build_master_filter(dir: &std::path::Path, excl: &ExclusionPorts) -> Option<std::path::PathBuf> {
        if excl.is_empty() {
            return None;
        }
        let read = |rel: &str| -> Option<String> {
            std::fs::read_to_string(dir.join(rel)).ok().map(|s| s.trim().to_string())
        };
        let discord_media = read(r"windivert.filter\windivert_part.discord_media_wide.txt")?;
        let stun = read(r"windivert.filter\windivert_part.stun.txt")?;
        let quic = read(r"windivert.filter\windivert_part.quic_initial_ietf.txt")?;
        let master = format!(
            "(outbound and tcp and (tcp.DstPort==80 or tcp.DstPort==443)) or ({discord_media}) or ({stun}) or ({quic})"
        );
        let mut excl_clauses: Vec<String> = Vec::new();
        for p in &excl.tcp {
            excl_clauses.push(format!("tcp.SrcPort != {p}"));
        }
        for p in &excl.udp {
            excl_clauses.push(format!("udp.SrcPort != {p}"));
        }
        let excl_clause = excl_clauses.join(" and ");
        let full = format!("({master}) and ({excl_clause})");
        let out = crate::ipc::data_dir().join("winws_master.filter");
        let _ = std::fs::create_dir_all(crate::ipc::data_dir());
        std::fs::write(&out, full).ok()?;
        Some(out)
    }

    /// KAPSAMLI (catch-all) zapret argümanları — bundle yollarıyla. TÜM TCP/443 + QUIC desync (yalnız
    /// hostlist DEĞİL) → Discord MASAÜSTÜ'nün HER bağlantısı kapsanır. CANLI DOĞRULANDI (2026-06-08 Türkcell).
    /// `excl` boş değilse master filter dosyası + `--wf-raw=@path`; boşsa modüler `--wf-*` flag'leri.
    fn args(dir: &std::path::Path, excl: &ExclusionPorts, strategy: &Strategy, hostlist: &[String]) -> Vec<String> {
        let pj = |rel: &str| dir.join(rel).to_string_lossy().into_owned();
        let mut args: Vec<String> = Vec::new();

        // Fake QUIC/voice payload (item 1.4): selectable per strategy. Fall back to the always-bundled
        // default if the named .bin isn't present (defensive — winws would error on a missing file).
        let fake_rel = if strategy.fake_quic.is_empty() {
            r"files\quic_initial_www_google_com.bin"
        } else {
            strategy.fake_quic
        };
        let fake_bin = {
            let candidate = dir.join(fake_rel);
            if candidate.exists() {
                candidate.to_string_lossy().into_owned()
            } else {
                pj(r"files\quic_initial_www_google_com.bin")
            }
        };

        // CAPTURE FILTER: exclusion → raw master filter; else modular --wf-* flags (proven scaffold).
        // wf_tcp/wf_udp come from the strategy (item 1.4): generic strategies use wf_tcp=80,443 + empty
        // wf_udp (raw-part signature capture only = proven default); ISP presets add their --wf-udp ports.
        if let Some(master) = Self::build_master_filter(dir, excl) {
            args.push(format!("--wf-raw=@{}", master.to_string_lossy()));
        } else {
            let wf_tcp = if strategy.wf_tcp.is_empty() { "80,443" } else { strategy.wf_tcp };
            args.push(format!("--wf-tcp={wf_tcp}"));
            if !strategy.wf_udp.is_empty() {
                args.push(format!("--wf-udp={}", strategy.wf_udp));
            }
            args.push(format!("--wf-raw-part=@{}", pj(r"windivert.filter\windivert_part.discord_media_wide.txt")));
            args.push(format!("--wf-raw-part=@{}", pj(r"windivert.filter\windivert_part.stun.txt")));
            args.push(format!("--wf-raw-part=@{}", pj(r"windivert.filter\windivert_part.quic_initial_ietf.txt")));
        }

        // ---- LAYERED PROFILE CONSTRUCTION (2026-08-16) -------------------------------------------
        //
        // winws evaluates profiles IN ORDER and the FIRST whose filter matches a flow handles it.
        // That ordering is what makes layering possible, and it is also what made the old third
        // stage (a second unconditional `--filter-tcp=443` block) dead code: nothing could ever
        // reach it, because the stage above matched every 443 flow first.
        //
        // The layout now is:
        //   1. aggressive chain, GATED to the hostlist  (domains known to need a bypass)
        //   2. harmless chain, catch-all                (everything else — must not be able to hurt)
        //   3. Discord voice/STUN                       (never gated: STUN carries no hostname)
        //
        // Layer 2 is why "Güçlü" can be broad without being dangerous. Before this, broad meant
        // running ONE route-dependent forgery over every HTTPS flow on the machine.
        let gated = strategy.hostlist_only && !hostlist.is_empty();
        let hostlist_flag: Option<String> = if gated {
            let path = crate::ipc::data_dir().join("hostlist.txt");
            let _ = std::fs::create_dir_all(crate::ipc::data_dir());
            let write_result = std::fs::write(&path, hostlist.join("\r\n"));
            // hostlist.txt CANNOT be encrypted: `winws.exe` is a separate bundled binary that reads
            // it from disk, so ciphertext would simply stop the engine working. Narrowing the ACL is
            // the protection that remains — it keeps another standard account on this machine from
            // reading which sites this user asked to unblock. It does NOT stop an administrator.
            // Re-applied on every write because the file is recreated each time.
            crate::secure::harden_acl(&path);
            if let Err(e) = write_result {
                // Not cosmetic: without this file winws refuses to start, and the old code only
                // printed to a stderr nobody reads and then passed --hostlist anyway.
                crate::elog::error(
                    "engine",
                    "hostlist_write",
                    &format!("hostlist file could not be written ({}): {e}", path.display()),
                );
            }
            Some(format!("--hostlist={}", path.to_string_lossy()))
        } else {
            None
        };
        let push_hostlist = |args: &mut Vec<String>| {
            if let Some(h) = &hostlist_flag {
                args.push(h.clone());
            }
        };

        // ---- Layer 1: the selected chain (gated when a hostlist is in play) ----------------------
        if !strategy.desync.is_empty() {
            // TCP/80 (HTTP) — plaintext Host header, same scope as the TLS stage.
            args.push("--filter-tcp=80".into());
            args.push("--dpi-desync=fake,fakedsplit".into());
            args.push("--dpi-desync-autottl=2".into());
            args.push("--dpi-desync-fooling=md5sig".into());
            push_hostlist(&mut args);
            args.push("--new".into());

            // TCP/443 (TLS) — PRIMARY desync, built from the selected typed Strategy (item 1.1).
            //
            // Skipped when the gated chain is IDENTICAL to the catch-all fallback below: the
            // fallback covers a strict superset of the same flows with the same treatment, so this
            // stage could only ever do the same work twice — and its `--hostlist` costs a lookup on
            // every single TLS connection to get there. This is the shape an untuned Güçlü takes
            // (both layers resolve to the harmless default), i.e. the common case on a fresh install.
            //
            // Compared by the ARGUMENTS the two stages would emit, not by strategy id: a caller can
            // hand us a Strategy whose id says one thing and whose desync/fooling fields say
            // another (the tuner and the mode planner both mutate fields on a catalog entry). An
            // id comparison would then drop a stage that was genuinely different — a unit test
            // caught exactly that.
            let redundant_with_fallback = gated
                && !strategy.fallback.is_empty()
                && strategy_by_id(strategy.fallback).tls_profile_args() == strategy.tls_profile_args();
            if !redundant_with_fallback {
                args.extend(strategy.tls_profile_args());
                push_hostlist(&mut args);
                args.push("--new".into());
            }

            // QUIC (UDP/443, HTTP/3). Gated exactly like the TLS stage. Outside the hostlist QUIC is
            // left ALONE on purpose: a broken HTTP/3 path is invisible (the browser silently falls
            // back to TCP after a stall), so mangling it catch-all buys nothing and costs latency on
            // every site the user never asked us to touch.
            args.push("--filter-l7=quic".into());
            args.push("--dpi-desync=fake".into());
            args.push("--dpi-desync-repeats=11".into());
            args.push(format!("--dpi-desync-fake-quic={fake_bin}"));
            push_hostlist(&mut args);
            args.push("--new".into());
        }

        // ---- Layer 2: harmless catch-all fallback ------------------------------------------------
        // Only reachable for flows layer 1 did not claim, i.e. only when layer 1 was hostlist-gated.
        if gated && !strategy.fallback.is_empty() {
            let fb = strategy_by_id(strategy.fallback);
            // Hard invariant, enforced here rather than trusted: a chain that can forge a packet the
            // real server will ACCEPT must never run over traffic nobody asked us to fix. If a
            // future edit points `fallback` at such a chain, drop the layer and say so — degrading
            // to "narrower coverage" is recoverable, shipping a catch-all forgery is not.
            if fb.is_harmless() {
                let fb_args = fb.tls_profile_args();
                if !fb_args.is_empty() {
                    args.extend(fb_args);
                    args.push("--new".into());
                }
            } else {
                crate::elog::warn(
                    "engine",
                    "unsafe_fallback",
                    &format!(
                        "fallback chain '{}' can forge packets the server accepts — catch-all layer dropped, \
                         coverage is hostlist-only",
                        fb.id
                    ),
                );
            }
        }

        // ---- Layer 3: Discord voice (STUN) + Discord L7 ------------------------------------------
        // Never hostlist-gated: STUN carries no hostname, so a --hostlist here would match nothing.
        // Scoped by L7 protocol, so it cannot touch ordinary web traffic.
        args.push("--filter-l7=discord,stun".into());
        args.push("--dpi-desync=fake".into());
        args.push("--dpi-desync-repeats=6".into());
        args.push(format!("--dpi-desync-fake-discord={fake_bin}"));
        args.push(format!("--dpi-desync-fake-stun={fake_bin}"));
        args
    }

}

/// Does this (strategy, hostlist) pair actually produce any packet-level work?
///
/// "off" with no fallback produces only the Discord voice stage, which is worth running for Discord
/// users but is not "protection" for the web. The caller uses this to decide whether starting a
/// winws process is justified at all — on a DNS-only block, the honest answer is to run nothing and
/// say so, not to keep a kernel driver attached for show.
///
/// Free function, not a `WinwsEngine` method: the orchestration layer needs it on every platform,
/// and `WinwsEngine` only exists on Windows.
pub fn has_web_stage(strategy: &Strategy, hostlist: &[String]) -> bool {
    if !strategy.desync.is_empty() {
        return true;
    }
    let gated = strategy.hostlist_only && !hostlist.is_empty();
    gated && !strategy.fallback.is_empty()
}

#[cfg(windows)]
impl BypassEngine for WinwsEngine {
    fn id(&self) -> &'static str {
        "zapret"
    }
    fn kind(&self) -> EngineKind {
        EngineKind::Desync
    }
    fn caps(&self) -> EngineCaps {
        EngineCaps { split_tunnel: true, requires_windivert: true, kernel_less: false }
    }
    fn is_available(&self) -> bool {
        Self::exe_exists()
    }
    /// The selected typed strategy now drives the PRIMARY TLS/443 stage (item 1.1); the HTTP/QUIC/voice
    /// stages stay the proven catch-all scaffold. Returns the full command line for UI display.
    fn build_args(&self, strategy: &Strategy, hostlist: &[String]) -> Vec<String> {
        match Self::bundle_dir() {
            Some(dir) => Self::args(&dir, &self.excl, strategy, hostlist),
            None => vec!["<winws bundle missing>".into()],
        }
    }
    fn start(&mut self, strategy: &Strategy, hostlist: &[String]) -> Result<(), String> {
        if let Some(c) = self.child.as_mut() {
            if matches!(c.try_wait(), Ok(None)) {
                return Ok(()); // still alive → idempotent
            }
        }
        let dir = match Self::bundle_dir() {
            Some(d) => d,
            None => return Err("could not resolve the winws bundle directory".into()),
        };
        let exe = dir.join("winws.exe");
        if !exe.exists() {
            return Err(format!("winws.exe not found ({}) — bundle is missing", exe.display()));
        }

        // SPEED (2026-08-16). This used to run `kill_all()` (a taskkill spawn) AND
        // `clear_stale_windivert()` (up to 3 service names × sc config + sc stop + 5×500ms poll +
        // sc delete) on EVERY start, before even attempting to launch. That is the bulk of the
        // 3.9s cold start the user was waiting through, and it was pure insurance: on a healthy
        // machine every one of those calls is a no-op that still costs its full poll budget.
        //
        // Both are now failure-path only. The happy path does one cheap in-process check (toolhelp
        // snapshot, sub-millisecond) and spawns. If winws then dies immediately — the actual symptom
        // of a stale/conflicting WinDivert registration — the full cleanup runs and we retry. Same
        // recovery, paid only when it is needed.
        if crate::proc::image_running("winws.exe") {
            Self::kill_all();
        }

        let excl = self.excl.clone();
        let strat = strategy.clone();
        let hl = hostlist.to_vec();
        let argv = Self::args(&dir, &excl, &strat, &hl);
        crate::elog::info("engine", "winws_args", &format!("winws {}", argv.join(" ")));

        let mut child = Self::spawn_winws(&exe, &argv).map_err(|e| format!("winws failed to start: {e}"))?;
        self.assign_to_job(&child);

        // Poll instead of sleeping a flat 700ms. A healthy winws is alive after ~30ms, so the old
        // fixed sleep spent ~670ms per start proving something already true.
        if let Some(code) = Self::settle(&mut child, std::time::Duration::from_millis(700)) {
            crate::elog::warn(
                "engine",
                "winws_instant_exit",
                &format!(
                    "winws exited immediately (code {code}) — likely a stale/conflicting WinDivert \
                     registration; clearing the driver and retrying"
                ),
            );
            clear_stale_windivert();
            // NO `--debug=1` on the retry.
            //
            // It was added here to explain WHY winws refused to start (winws is silent on a healthy
            // run, so the captured stdio was empty for the one case that mattered). It was removed
            // again on 2026-08-16: winws's debug output is per-connection and can print matched
            // hostnames, and `winws.log` is captured to disk and swept into the support bundle. A
            // privacy rule that says no requested domain is ever written in plaintext cannot have an
            // exception that fires exactly when something is going wrong.
            //
            // What is kept instead costs nothing and leaks nothing: the exit code, and our own
            // argument line (which carries the hostlist FILE PATH, never its contents).
            let mut retry =
                Self::spawn_winws(&exe, &argv).map_err(|e| format!("winws failed to restart: {e}"))?;
            self.assign_to_job(&retry);
            if let Some(code2) = Self::settle(&mut retry, std::time::Duration::from_millis(1500)) {
                // Do NOT keep a dead child and report success — that is the silent-success bug.
                let tail = Self::log_tail();
                return Err(format!(
                    "winws exited immediately twice (code {code2}). Last engine output: {tail}"
                ));
            }
            child = retry;
        }
        self.child = Some(child);
        Ok(())
    }
    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        if crate::proc::image_running("winws.exe") {
            Self::kill_all();
        }
    }
    /// MEASURED, not remembered (CLAUDE.md rule 10). This used to return `self.child.is_some()`,
    /// which stays true forever after winws dies — the handle is only cleared by an explicit stop.
    /// So a crashed engine kept reporting "running" until the 5s watchdog happened to notice.
    fn is_running(&self) -> bool {
        match &self.child {
            // try_wait needs &mut; the interior state we need is just "has it exited", which the
            // OS can answer from the handle without mutating our side. Ok(None) = still alive.
            Some(c) => crate::proc::pid_alive(c.id()),
            None => false,
        }
    }
    fn exclusion_differs(&self, excl: &ExclusionPorts) -> bool {
        &self.excl != excl
    }
    fn set_exclusion(&mut self, excl: &ExclusionPorts) {
        if &self.excl == excl {
            return;
        }
        eprintln!(
            "[evorift][winws] exclusion değişti (tcp={} udp={}) — winws yeniden başlatılacak",
            excl.tcp.len(),
            excl.udp.len()
        );
        self.excl = excl.clone();
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

#[cfg(windows)]
impl Drop for WinwsEngine {
    fn drop(&mut self) {
        self.stop();
        if self.job != 0 {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(self.job as _) };
            self.job = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 1.1: the default "c1" strategy must reproduce the live-verified primary TLS stage exactly
    /// (byte-for-byte) — guards against the typed builder drifting from the proven Türkcell config.
    #[test]
    fn c1_reproduces_proven_primary_stage() {
        let a = strategy_by_id("c1").tls_profile_args();
        let got: Vec<&str> = a.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            got,
            vec![
                "--filter-tcp=443",
                "--dpi-desync=fake,multidisorder",
                "--dpi-desync-split-pos=1,midsld",
                "--dpi-desync-repeats=11",
                "--dpi-desync-fooling=md5sig",
                "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com",
            ]
        );
    }

    /// Unset/"none" fields are omitted (no empty flags emitted).
    #[test]
    fn multidisorder_omits_unset_fields() {
        let a = strategy_by_id("multidisorder").tls_profile_args();
        let got: Vec<&str> = a.iter().map(|s| s.as_str()).collect();
        assert_eq!(
            got,
            vec![
                "--filter-tcp=443",
                "--dpi-desync=multidisorder",
                "--dpi-desync-split-pos=1,midsld",
                "--dpi-desync-repeats=6",
            ]
        );
        assert!(!a.iter().any(|s| s.contains("fooling")), "fooling=none must be omitted");
        assert!(!a.iter().any(|s| s.contains("fake-tls-mod")), "empty fake_tls_mod must be omitted");
    }

    #[test]
    fn fake_strategy_args() {
        let a = strategy_by_id("fake").tls_profile_args();
        assert_eq!(a[1], "--dpi-desync=fake");
        assert!(a.iter().any(|s| s == "--dpi-desync-split-pos=1"));
        assert!(a.iter().any(|s| s == "--dpi-desync-repeats=11"));
    }

    /// ttl / autottl / seqovl emit their flags only when > 0.
    #[test]
    fn ttl_autottl_seqovl_emitted_when_set() {
        let s = Strategy {
            id: "t",
            desync: "fake",
            split_pos: "",
            repeats: 0,
            fooling: "none",
            fake_tls_mod: "",
            ttl: 4,
            autottl: 3,
            seqovl: 2,
            wf_tcp: "80,443",
            wf_udp: "443",
            fake_quic: "",
            hostlist_only: false,
            fallback: "",
        };
        let a = s.tls_profile_args();
        assert!(a.iter().any(|x| x == "--dpi-desync-ttl=4"));
        assert!(a.iter().any(|x| x == "--dpi-desync-autottl=3"));
        assert!(a.iter().any(|x| x == "--dpi-desync-split-seqovl=2"));
        // unset fields stay absent
        assert!(!a.iter().any(|x| x.contains("split-pos")));
        assert!(!a.iter().any(|x| x.contains("repeats")));
    }

    /// Item 1.2: ISP presets exist, resolve by id, and tune the primary desync per docs/03 §4.1.
    #[test]
    fn isp_presets_resolve_and_tune() {
        assert_eq!(presets().len(), 7);
        let tt = strategy_by_id("tt");
        assert_eq!(tt.desync, "fake");
        assert_eq!(tt.ttl, 4);
        let a = tt.tls_profile_args();
        assert!(a.iter().any(|s| s == "--dpi-desync=fake"));
        assert!(a.iter().any(|s| s == "--dpi-desync-ttl=4"));
        let vf = strategy_by_id("vodafone-hotspot");
        assert_eq!(vf.desync, "multisplit");
        assert!(vf.tls_profile_args().iter().any(|s| s == "--dpi-desync-split-pos=2"));
        let tc = strategy_by_id("turkcell-hotspot");
        assert!(tc.tls_profile_args().iter().any(|s| s == "--dpi-desync-autottl=3"));
        // SuperOnline-Alt carries the alternate voice port range for item 1.4.
        assert_eq!(strategy_by_id("superonline-alt").wf_udp, "443,50000-50099");
    }

    /// Item 3.4: the engine catalog includes the composite ByeDPI modes (resolvable by `make_engine`).
    #[cfg(windows)]
    #[test]
    fn catalog_includes_composite_byedpi() {
        let ids: Vec<String> = catalog().into_iter().map(|e| e.id).collect();
        assert!(ids.contains(&"byedpi-proxifyre".to_string()));
        assert!(ids.contains(&"byedpi-drover".to_string()));
        // factory resolves them to the right concrete engine id
        assert_eq!(make_engine("byedpi-drover").id(), "byedpi-drover");
    }

    /// All strategy + preset ids are unique kebab slugs (selectable through profiles).
    #[test]
    fn preset_ids_unique_and_valid() {
        let mut ids: Vec<&str> = strategies().iter().chain(presets().iter()).map(|s| s.id).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "strategy/preset ids must be unique");
        for s in presets() {
            assert!(
                s.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "preset id '{}' must be a kebab slug",
                s.id
            );
        }
    }

    /// Item 1.3: build_args adds `--hostlist` to the TCP/QUIC groups when the strategy is hostlist-only
    /// and a list is given; stays catch-all (no `--hostlist`) otherwise. Voice/STUN group never gated.
    #[cfg(windows)]
    #[test]
    fn hostlist_mode_toggles_flag() {
        let eng = WinwsEngine::new();
        let off = eng.build_args(&strategy_by_id("c1"), &["discord.com".to_string()]);
        assert!(
            !off.iter().any(|a| a.starts_with("--hostlist=")),
            "catch-all must not gate by hostlist"
        );
        let mut s = strategy_by_id("c1");
        s.hostlist_only = true;
        let on = eng.build_args(&s, &["discord.com".to_string(), "roblox.com".to_string()]);
        let count = on.iter().filter(|a| a.starts_with("--hostlist=")).count();
        assert!(count >= 1, "hostlist mode must emit --hostlist");
        assert!(count <= 4, "voice/STUN group must stay catch-all (TCP/QUIC groups only)");
    }

    /// Item 1.4: fake .bin payload + UDP voice ports. The bundled default .bin must exist and be
    /// referenced; generic strategies keep raw-part capture (no --wf-udp); ISP presets add voice ports.
    #[cfg(windows)]
    #[test]
    fn fake_bin_and_voice_ports() {
        let bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("winws")
            .join("files")
            .join("quic_initial_www_google_com.bin");
        assert!(bin.exists(), "bundled fake QUIC .bin must exist: {}", bin.display());

        let eng = WinwsEngine::new();
        let g = eng.build_args(&strategy_by_id("c1"), &[]);
        assert!(!g.iter().any(|a| a.starts_with("--wf-udp=")), "generic strategy must not add --wf-udp");
        assert!(
            g.iter().any(|a| a.contains("quic_initial_www_google_com.bin")),
            "must reference the bundled fake .bin"
        );
        assert!(g.iter().any(|a| a.starts_with("--dpi-desync-fake-quic=")));

        let p = eng.build_args(&strategy_by_id("tt"), &[]);
        assert!(
            p.iter().any(|a| a == "--wf-udp=443,50000,50100"),
            "tt preset must add explicit voice ports"
        );
    }
}
