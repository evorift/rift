//! DPI-bypass motor soyutlaması (Faz 3 — moat). docs/05 §3, §7.
//!
//! Katmanlar:
//!  - [`Strategy`]: tipli strateji verisi (winws parametrelerine birebir) — derlenir/test edilir.
//!  - [`DpiEngine`]: start/stop/running sözleşmesi.
//!  - [`WinwsEngine`]: VARSAYILAN motor (Windows) — kanıtlanmış `winws` (zapret) sidecar'ı.
//!  - [`SimEngine`]: Windows dışı / dev için no-op simülasyon (UI↔IPC zincirini uçtan uca çalıştırır).
//!
//! NOT: Eski saf-Rust WinDivert motoru (`engine/real.rs` + tls/packet/quic/voice byte-cerrahisi) net3'e
//! geçişle KALDIRILDI — winws QUIC/gateway desync'ini gerçek yapıyor, eski motor masaüstü Discord'u
//! açamıyordu (bkz. net3/SOLUTION.md §3.3). Discord'un kendisi artık WARP split-tunnel ile (warp.rs)
//! taşınıyor; winws'in canlı işi Roblox + genel HTTPS SNI-desync'i.

/// Bir DPI-desync stratejisinin tipli modeli (winws parametrelerine birebir).
/// Örn c1: `--dpi-desync=fake,multidisorder --dpi-desync-split-pos=1,midsld
///         --dpi-desync-repeats=11 --dpi-desync-fooling=md5sig
///         --dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com`
#[derive(Clone, Debug)]
pub struct Strategy {
    pub id: &'static str,
    pub desync: &'static str,       // "fake,multidisorder"
    pub split_pos: &'static str,    // "1,midsld"
    pub repeats: u32,               // 11
    pub fooling: &'static str,      // "md5sig"
    pub fake_tls_mod: &'static str, // "rnd,dupsid,sni=www.google.com"
}

/// Bilinen stratejiler. "auto" = otomatik bulucu (şimdilik c1'e eş; ileride blockcheck).
pub fn strategies() -> &'static [Strategy] {
    &[
        Strategy {
            id: "c1",
            desync: "fake,multidisorder",
            split_pos: "1,midsld",
            repeats: 11,
            fooling: "md5sig",
            fake_tls_mod: "rnd,dupsid,sni=www.google.com",
        },
        Strategy {
            id: "multidisorder",
            desync: "multidisorder",
            split_pos: "1,midsld",
            repeats: 6,
            fooling: "none",
            fake_tls_mod: "",
        },
        Strategy {
            id: "fake",
            desync: "fake",
            split_pos: "1",
            repeats: 11,
            fooling: "md5sig",
            fake_tls_mod: "rnd,dupsid,sni=www.google.com",
        },
    ]
}

/// id → strateji. "auto" ve bilinmeyen → c1 (test edilmiş başlangıç preset'i, docs/05 §7).
pub fn strategy_by_id(id: &str) -> Strategy {
    strategies()
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .unwrap_or_else(|| strategies()[0].clone())
}

/// DPI-bypass motoru sözleşmesi. Servis bunu Start/Stop'ta çağırır.
pub trait DpiEngine: Send {
    /// Stratejiyi + hostlist'i uygula (WinDivert aç, filtre kur, paket döngüsü başlat).
    fn start(&mut self, strategy: &Strategy, hostlist: &[String]) -> Result<(), String>;
    /// Durdur (winws alt-sürecini öldür / tüneli kaldır — kurallar geri al).
    fn stop(&mut self);
    fn is_running(&self) -> bool;
    /// Per-app İNDİRME (ingress) hız limiti — net3 geçişiyle ARTIK no-op. İnbound (download) throttle eski
    /// saf-Rust WinDivert motorunun işiydi (KALDIRILDI); winws sidecar'ı per-app inbound paketleri kısamaz.
    /// YÜKLEME (egress) limiti hâlâ çalışır: service.rs `run_limit` NetQosPolicy ile uygular. Servis bu metodu
    /// yine çağırır (sözleşme korunuyor) ama hiçbir motor override etmez → no-op.
    fn set_limits(&mut self, _limits: &[(String, u32)]) {}
    /// "Off" modlu uygulamaların ÇALIŞAN PID'lerinin TCP/UDP source port'ları — winws WinDivert capture
    /// filter'ından HARİÇ tutulur (gerçek per-app DPI exclusion). Liste değiştiğinde motor kendini
    /// yeniden başlatabilir (winws WinDivert filter'ı hot-reload edemez). Boş → eski catch-all davranışı
    /// (regresyon yok). Servis pid_scan ile ~5sn'de bir besler; aynı liste → no-op (gereksiz restart yok).
    fn set_exclusion(&mut self, _excl: &crate::pid_scan::ExclusionPorts) {}
}

/// Varsayılan simülasyon motoru — gerçek paket müdahalesi YOK (dev / WinDivert kapalı).
/// UI ↔ IPC ↔ motor zincirini uçtan uca çalıştırır; telemetri simülasyonu ayrı.
#[derive(Default)]
pub struct SimEngine {
    running: bool,
}

impl DpiEngine for SimEngine {
    fn start(&mut self, strategy: &Strategy, hostlist: &[String]) -> Result<(), String> {
        eprintln!(
            "[evorift-svc][sim-engine] start strategy={} desync={} split={} repeats={} ({} domain)",
            strategy.id, strategy.desync, strategy.split_pos, strategy.repeats, hostlist.len()
        );
        self.running = true;
        Ok(())
    }
    fn stop(&mut self) {
        eprintln!("[evorift-svc][sim-engine] stop");
        self.running = false;
    }
    fn is_running(&self) -> bool {
        self.running
    }
}

// ============================================================================
// winws (zapret · MIT lisans, github.com/bol-van/zapret) SIDECAR motoru — VARSAYILAN.
// DPI-bypass'ı gömülü `winws.exe` alt-süreciyle yapar: kanıtlanmış zapret stratejisi (Türkcell
// Discord/Roblox + QUIC/UDP-443 + ses), SİSTEM GENELİ (tüm uygulamalar; yalnız hostlist siteleri).
// Eski saf-Rust WinDivert motoru (KALDIRILDI) kendi QUIC desync'ini tam çözemiyordu → masaüstü Discord
// "Starting"de takılıyordu. winws QUIC'i gerçek fake-initial ile düzgün desync eder → masaüstü açılır.
// Bundle yolu: evorift-svc.exe yanındaki `winws\` klasörü (NSIS/elle deploy ile kurulur).
// ============================================================================
#[cfg(windows)]
pub struct WinwsEngine {
    child: Option<std::process::Child>,
    /// Windows Job Object handle (0 = henüz yok), `isize` olarak tutulur (HANDLE = `*mut c_void` Send
    /// değildir; DpiEngine: Send gerektirir). KILL_ON_JOB_CLOSE bayrağıyla: bu süreç (servis) ölünce
    /// VEYA çökünce kernel atanan winws.exe'yi otomatik öldürür → yetim SYSTEM süreci kalmaz (§6).
    job: isize,
    /// Şu an uygulanmış "off" exclusion port'ları. Servis ~5sn'de bir set_exclusion çağırır; AYNI
    /// liste → no-op (gereksiz restart yok). Liste değişirse winws restart edilir (WinDivert
    /// filter hot-reload yok). Boş → eski `--wf-tcp` + `--wf-raw-part` yolu (regresyon yok).
    excl: crate::pid_scan::ExclusionPorts,
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
        Self {
            child: None,
            job: 0,
            excl: crate::pid_scan::ExclusionPorts::default(),
        }
    }

    /// Bundle dizini: `<exe_dizini>\winws\`.
    fn bundle_dir() -> Option<std::path::PathBuf> {
        Some(std::env::current_exe().ok()?.parent()?.join("winws"))
    }

    /// Job object'i (bir kez) oluştur: KILL_ON_JOB_CLOSE → servis süreci sonlanınca/çökünce kernel
    /// atanan tüm süreçleri (winws) öldürür. Best-effort: başarısızsa 0 (job yok → eski davranış).
    fn ensure_job(&mut self) -> isize {
        if self.job != 0 {
            return self.job;
        }
        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        unsafe {
            // windows-sys: HANDLE = isize; NULL handle = 0.
            let h = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if h == 0 {
                return 0;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                h,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            self.job = h;
            self.job
        }
    }

    /// Yeni doğan winws sürecini job'a ata (kill-on-close kapsamına alınsın). Best-effort.
    fn assign_to_job(&mut self, child: &std::process::Child) {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
        let job = self.ensure_job();
        if job == 0 {
            return;
        }
        unsafe {
            AssignProcessToJobObject(job as _, child.as_raw_handle() as _);
        }
    }

    /// Çalışan TÜM winws.exe'leri öldür (yetim/standalone winws kalmasın → tek instance, çakışma yok).
    fn kill_all() {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/f", "/im", "winws.exe"])
            .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
            .output();
    }

    /// Eski/yabancı WinDivert ÇEKİRDEK SÜRÜCÜSÜNÜ temizle. Windows tek bir global WinDivert sürücüsü
    /// yükler: başka bir DPI aracı (zapret, GoodbyeDPI, eski bir evorift/zapret kurulumu) FARKLI sürümde
    /// "WinDivert" sürücüsü yüklemiş/kaydetmişse, bizim winws kendi WinDivert 2.2'sini açamaz → ANINDA ölür
    /// (klasik "winws hemen çıkıyor" çakışması). winws'i başlatmadan önce yabancı sürücü servis(ler)ini
    /// durdur + sil; winws sonra KENDİ eşleşen sürümünü taze kaydeder. Best-effort + sessiz (pencere yok).
    /// winws.exe önceden öldürüldüğü için sürücüde açık handle kalmaz → stop/delete temiz çalışır.
    fn clear_stale_windivert() {
        use std::os::windows::process::CommandExt;
        let sc = |args: &[&str]| -> String {
            std::process::Command::new("sc")
                .args(args)
                .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                .unwrap_or_default()
        };
        // ÖNEMLİ: ÇALIŞAN bir sürücüyü doğrudan `sc delete` etmek DELETE_PENDING limbo'su yaratır →
        // ardından winws "WinDivert" servisini AÇAMAZ/oluşturamaz (durum kötüleşir, reboot gerekir).
        // Güvenli sıra: önce DURDUR (yükten indir) → STOPPED olduğunu DOĞRULA → ANCAK O ZAMAN sil.
        // Duramıyorsa (açık handle var) DOKUNMA — winws uyumlu (aynı 2.x) sürücüyü zaten yeniden kullanır.
        for name in ["WinDivert", "WinDivert1.4", "WinDivert1.1"] {
            if !sc(&["query", name]).contains("STATE") {
                continue; // böyle bir servis yok → geç
            }
            let _ = sc(&["stop", name]);
            let mut stopped = false;
            for _ in 0..5 {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let q = sc(&["query", name]);
                if !q.contains("STATE") || q.contains("STOPPED") {
                    stopped = true; // tamamen indi (veya zaten yok)
                    break;
                }
            }
            if stopped {
                let _ = sc(&["delete", name]); // temiz sil (limbo yok) → winws taze, eşleşen sürücüyü kaydeder
            }
        }
    }

    /// "Off" mod exclusion port'ları varsa winws WinDivert capture filter'ını YENİDEN DERLE → o uygulamaların
    /// paketleri winws'e ulaşmaz. Tüm windivert.filter/* parçalarını + TCP/80,443 capture'ını birleştirir,
    /// sonuna `and (tcp.SrcPort != ... and udp.SrcPort != ...)` clause'u ekler, ProgramData'ya yazar →
    /// path döner. Boş exclusion → None (eski catch-all `--wf-tcp` + `--wf-raw-part` yolu).
    fn build_master_filter(
        dir: &std::path::Path,
        excl: &crate::pid_scan::ExclusionPorts,
    ) -> Option<std::path::PathBuf> {
        if excl.is_empty() {
            return None;
        }
        // 1) WinDivert filter parçalarını oku (her biri tek AND chain'i; OR ile birleştirilecek).
        let read = |rel: &str| -> Option<String> {
            std::fs::read_to_string(dir.join(rel)).ok().map(|s| s.trim().to_string())
        };
        let discord_media = read(r"windivert.filter\windivert_part.discord_media_wide.txt")?;
        let stun = read(r"windivert.filter\windivert_part.stun.txt")?;
        let quic = read(r"windivert.filter\windivert_part.quic_initial_ietf.txt")?;

        // 2) Master capture filter: TCP/80,443 catch-all OR Discord media OR STUN OR QUIC initial.
        //    Her parça parens içinde — operator precedence (AND > OR) gereği zaten güvenli, ama açıkça yaz.
        let master = format!(
            "(outbound and tcp and (tcp.DstPort==80 or tcp.DstPort==443)) or ({discord_media}) or ({stun}) or ({quic})"
        );

        // 3) Exclusion clause: "off" uygulamalarının source port'larını HARİÇ TUT. WinDivert için
        //    tcp.SrcPort UDP paketlerinde 0, udp.SrcPort TCP paketlerinde 0. Yani `tcp.SrcPort != X`
        //    UDP paketi için `0 != X` = true (etkilemez); TCP paketi için gerçek SrcPort != X olur.
        //    Sonuç: TCP clause'u TCP'yi, UDP clause'u UDP'yi süzer.
        let mut excl_clauses: Vec<String> = Vec::new();
        for p in &excl.tcp {
            excl_clauses.push(format!("tcp.SrcPort != {p}"));
        }
        for p in &excl.udp {
            excl_clauses.push(format!("udp.SrcPort != {p}"));
        }
        let excl_clause = excl_clauses.join(" and ");

        let full = format!("({master}) and ({excl_clause})");

        // 4) Geçici filter dosyasına yaz: %PROGRAMDATA%\evorift\winws_master.filter. winws --wf-raw=@path
        //    ile bunu okur. Restart'ta üzerine yazılır (idempotent).
        let out = crate::ipc::data_dir().join("winws_master.filter");
        let _ = std::fs::create_dir_all(crate::ipc::data_dir());
        std::fs::write(&out, full).ok()?;
        Some(out)
    }

    /// KAPSAMLI (catch-all) zapret strateji argümanları — bundle yollarıyla. zapret `service_create.cmd`
    /// varsayılanıyla birebir. KRİTİK: TÜM TCP/443 + TÜM QUIC desync edilir (yalnız hostlist DEĞİL) →
    /// Discord MASAÜSTÜ uygulamasının HER bağlantısı kapsanır. (Eski hostlist-only strateji yalnız web'i
    /// açıyordu; masaüstü Discord hostlist dışı bağlantılar yüzünden "loading"de takılıyordu.)
    /// CANLI DOĞRULANDI (2026-06-08, Türkcell): Discord masaüstü uygulaması TAM açıldı.
    ///
    /// `excl` boşsa eski mod (--wf-tcp + --wf-raw-part); boş değilse master filter dosyası yazılır ve
    /// --wf-raw=@<path> ile geçilir → o uygulamaların paketleri winws'e ULAŞMAZ (gerçek per-app off).
    fn args(dir: &std::path::Path, excl: &crate::pid_scan::ExclusionPorts) -> Vec<String> {
        let pj = |rel: &str| dir.join(rel).to_string_lossy().into_owned();
        let mut args: Vec<String> = Vec::new();

        // CAPTURE FILTER: exclusion varsa raw master filter dosyası, yoksa modüler --wf-* flag'leri.
        if let Some(master) = Self::build_master_filter(dir, excl) {
            args.push(format!("--wf-raw=@{}", master.to_string_lossy()));
        } else {
            args.push("--wf-tcp=80,443".into());
            // discord_media_WIDE: Discord SES IP-discovery paketini PORTTAN BAĞIMSIZ yakalar (port gate YOK).
            // Discord ses sunucuları DEĞİŞKEN portlar kullanır (19294-19344, 50000-50100, oturuma göre döner)
            // → port aralığına güvenmek kırılgan (canlı: bir kez 19340, sonra başka port → ses tekrar koptu).
            // Bunun yerine paketin BENZERSİZ imzasıyla eşle: PayloadLength=74 + Payload32[0]=0x00010046
            // (type=0x0001, len=70) + 64 sıfır bayt. Bu imza yalnız Discord ses IP-discovery isteğine uyar →
            // HER portta yakalanır → fake-desync ile DPI atlatılır → ses bağlanır.
            args.push(format!("--wf-raw-part=@{}", pj(r"windivert.filter\windivert_part.discord_media_wide.txt")));
            args.push(format!("--wf-raw-part=@{}", pj(r"windivert.filter\windivert_part.stun.txt")));
            args.push(format!("--wf-raw-part=@{}", pj(r"windivert.filter\windivert_part.quic_initial_ietf.txt")));
        }

        args.extend([
            // TCP/80 (HTTP) catch-all
            "--filter-tcp=80".into(),
            "--dpi-desync=fake,fakedsplit".into(),
            "--dpi-desync-autottl=2".into(),
            "--dpi-desync-fooling=md5sig".into(),
            "--new".into(),
            // TCP/443 (TLS) — birincil desync (TÜM siteler)
            "--filter-tcp=443".into(),
            "--dpi-desync=fake,multidisorder".into(),
            "--dpi-desync-split-pos=1,midsld".into(),
            "--dpi-desync-repeats=11".into(),
            "--dpi-desync-fooling=md5sig".into(),
            "--dpi-desync-fake-tls-mod=rnd,dupsid,sni=www.google.com".into(),
            "--new".into(),
            // TCP/443 — ikincil (badseq) yedek desync
            "--filter-tcp=443".into(),
            "--dpi-desync=fake,multidisorder".into(),
            "--dpi-desync-split-pos=midsld".into(),
            "--dpi-desync-repeats=6".into(),
            "--dpi-desync-fooling=badseq,md5sig".into(),
            "--new".into(),
            // QUIC (UDP/443 HTTP/3) catch-all — TÜM QUIC fake
            "--filter-l7=quic".into(),
            "--dpi-desync=fake".into(),
            "--dpi-desync-repeats=11".into(),
            format!("--dpi-desync-fake-quic={}", pj(r"files\quic_initial_www_google_com.bin")),
            "--new".into(),
            // Discord ses (STUN) + Discord L7 — Flowseal/general.bat ile birebir (kanıtlanmış preset):
            // fake-discord + fake-stun yüküyle DPI'ı UDP el-sıkışmada erkenden kandır (yalnızca `fake`
            // yetmiyor; ISS UDP'nin İLK paketini düşürüyorsa sahte yük arkadan gelen gerçek STUN/Discord
            // paketinin önünü açar). repeats=6 Discord ses sunucu seçim turnu için yeterli (deneyimle).
            // Sahte yük olarak QUIC initial bin'ini kullanırız (zaten bundle'da; Google SNI'lı `google.com`
            // ClientHello + uzunluk → orta-kutuya "bu Google QUIC" der → engelli host listesinden kaçar).
            "--filter-l7=discord,stun".into(),
            "--dpi-desync=fake".into(),
            "--dpi-desync-repeats=6".into(),
            format!("--dpi-desync-fake-discord={}", pj(r"files\quic_initial_www_google_com.bin")),
            format!("--dpi-desync-fake-stun={}", pj(r"files\quic_initial_www_google_com.bin")),
        ]);
        args
    }
}

#[cfg(windows)]
impl DpiEngine for WinwsEngine {
    /// winws'i başlat (idempotent: zaten canlıysa dokunma → UI SetHostlist/SetStrategy churn'ünde respawn yok).
    /// strategy/hostlist YOK SAYILIR (winws kendi hostlist dosyasını + sabit kanıtlanmış stratejiyi kullanır).
    fn start(&mut self, _strategy: &Strategy, _hostlist: &[String]) -> Result<(), String> {
        if let Some(c) = self.child.as_mut() {
            if matches!(c.try_wait(), Ok(None)) {
                return Ok(()); // hâlâ çalışıyor → idempotent
            }
        }
        use std::os::windows::process::CommandExt;
        let dir = match Self::bundle_dir() {
            Some(d) => d,
            None => return Err("winws bundle dizini çözülemedi".into()),
        };
        let exe = dir.join("winws.exe");
        if !exe.exists() {
            // Dev/bundle yoksa: gerçek bypass yok ama UI↔IPC zinciri çalışsın (sim). Hata DÖNDÜRME.
            eprintln!("[evorift][winws] bundle yok ({}) — sim (gerçek bypass yok)", exe.display());
            return Ok(());
        }
        Self::kill_all(); // eski/standalone winws temizle (tek instance)
        Self::clear_stale_windivert(); // yabancı/eski WinDivert sürücüsünü temizle (winws anında ölmesin)

        let excl = self.excl.clone();
        let spawn = || {
            std::process::Command::new(&exe)
                .args(Self::args(&dir, &excl))
                .creation_flags(0x0800_0000) // CREATE_NO_WINDOW (winws konsol penceresi açmasın)
                .spawn()
        };
        let mut child = spawn().map_err(|e| format!("winws başlatılamadı: {e}"))?;
        // §6: winws'i job object'e ata → servis çökse/öldürülse bile kernel winws'i öldürür (yetim yok).
        self.assign_to_job(&child);

        // Dayanıklılık: winws bir WinDivert çakışmasıyla ~hemen ölebilir. Kısa bir süre bekleyip
        // anında-çıkışı yakala → sürücüyü BİR KEZ daha temizleyip yeniden dene (sürücü o an boşalmış olur).
        std::thread::sleep(std::time::Duration::from_millis(700));
        if matches!(child.try_wait(), Ok(Some(_))) {
            eprintln!("[evorift][winws] anında çıktı (WinDivert çakışması olası) — temizleyip yeniden deniyorum");
            Self::clear_stale_windivert();
            std::thread::sleep(std::time::Duration::from_millis(500));
            child = spawn().map_err(|e| format!("winws yeniden başlatılamadı: {e}"))?;
            self.assign_to_job(&child);
        }
        self.child = Some(child);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        Self::kill_all();
    }

    fn is_running(&self) -> bool {
        self.child.is_some()
    }

    /// "Off" mod exclusion port'ları değişirse winws'i yeniden başlatmak için child handle'ı düşür
    /// (servis watchdog ~5sn'de bir start çağırır → yeni args ile respawn olur, kayıp ≤ 5sn).
    /// Aynı liste → no-op (gereksiz restart yok; iyileştirme: kullanıcı modu değiştirmediği sürece
    /// PowerShell tarama aynı listeyi döner → winws'e dokunulmaz).
    fn set_exclusion(&mut self, excl: &crate::pid_scan::ExclusionPorts) {
        if &self.excl == excl {
            return;
        }
        eprintln!(
            "[evorift][winws] exclusion değişti (tcp={} udp={}) — winws yeniden başlatılacak",
            excl.tcp.len(),
            excl.udp.len()
        );
        self.excl = excl.clone();
        // Child'ı düşür (job kill'i ile birlikte) → watchdog idempotent start'la yeni args'ı uygular.
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

#[cfg(windows)]
impl Drop for WinwsEngine {
    fn drop(&mut self) {
        self.stop(); // servis kapanınca winws'i de öldür (yetim kalmasın)
        // Job handle'ını kapat: son handle kapanınca KILL_ON_JOB_CLOSE atanan süreçleri de temizler.
        if self.job != 0 {
            unsafe { windows_sys::Win32::Foundation::CloseHandle(self.job as _) };
            self.job = 0;
        }
    }
}

/// Aktif motoru üret. VARSAYILAN: winws (zapret) sidecar (Windows). Windows dışı: no-op simülasyon.
pub fn make_engine() -> Box<dyn DpiEngine> {
    #[cfg(windows)]
    {
        Box::new(WinwsEngine::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(SimEngine::default())
    }
}
