//! Auto-Pilot — birleşik strateji bulucu (docs/07 §6, docs/06 §2.2). "Beni Koru" → hedef siteleri
//! HER motor/strateji adayıyla GERÇEK test et → skor tablosu → en iyi profili öner. Mevcut blockcheck
//! yalnız tek motoru tarar; Auto-Pilot tüm kullanılabilir motorları + stratejileri sırayla dener.
//!
//! ÖNEMLİ: RunOnce (kalıcı hizmet kurmadan) bir motoru başlatır → test eder → durdurur. winws/GoodbyeDPI
//! WinDivert'i SİSTEM GENELİ tutar → aynı anda yalnız TEK desync motoru çalışmalı. Bu yüzden çağıran
//! (service.rs AutoPilot kolu) önce AKTİF motoru duraklatır, Auto-Pilot biter bitmez eski durumu geri yükler.

use serde::{Deserialize, Serialize};
use crate::engine::{self, Strategy};

/// Scan depth (docs/03 §4.4 blockcheck SCANLEVEL, docs/07 §6):
///  - `Quick`: a curated shortlist, stop at the first all-open candidate (fast).
///  - `Standard`: the full available matrix, stop at the first all-open (balanced).
///  - `Force`: the full matrix, try EVERY candidate (find the absolute best, slowest).
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Depth {
    Quick,
    Standard,
    Force,
}

impl Depth {
    /// Lenient parse: quick/fast → Quick, force/full → Force, anything else → Standard.
    pub fn from_str_lenient(s: &str) -> Depth {
        match s.to_ascii_lowercase().as_str() {
            "quick" | "fast" => Depth::Quick,
            "force" | "full" => Depth::Force,
            _ => Depth::Standard,
        }
    }

    /// Stop at the first all-open candidate? True for Quick/Standard; Force tries every candidate.
    fn early_stop(&self) -> bool {
        !matches!(self, Depth::Force)
    }
}

/// Bir aday = (motor id, strateji id).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Candidate {
    pub engine: String,
    pub strategy: String,
}

/// Bir adayın skor satırı (UI canlı tabloya akıtır).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoreRow {
    pub engine: String,
    pub strategy: String,
    /// (hedef, açıldı_mı, gecikme_ms).
    pub per_target: Vec<(String, bool, u32)>,
    /// skor = (açılan hedef sayısı)×100 − (ortalama gecikme cezası). Yüksek = daha iyi.
    pub score: i64,
}

/// ISP detection (docs/07 §6 step 1) — OPT-IN for privacy. Without `consent` we NEVER send the user's IP
/// to a third party → returns `None`. With explicit consent, a one-shot lookup of the public IP's org is
/// attempted (best-effort). The result only PRIORITIZES presets; it never changes which presets exist.
pub fn detect_isp(consent: bool) -> Option<String> {
    if !consent {
        return None;
    }
    lookup_isp_online()
}

/// Best-effort online ISP lookup (consent path only). Queries a public IP-info endpoint for the org name.
/// Network-dependent → not unit-tested; returns None on any failure.
fn lookup_isp_online() -> Option<String> {
    let out = crate::sys::query_os(
        "powershell",
        &[
            "-NoProfile",
            "-Command",
            "try { (Invoke-RestMethod -Uri 'https://ipinfo.io/org' -TimeoutSec 5) } catch { '' }",
        ],
    );
    let s = out.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Map an ISP/org name to the best-matching ISP preset id (docs/03 §4.1). Pure → testable. SuperOnline is
/// checked before Turkcell (Turkcell SuperOnline → superonline preset).
pub fn isp_preset_id(isp: &str) -> Option<&'static str> {
    let l = isp.to_ascii_lowercase();
    if l.contains("turk telekom") || l.contains("turktelekom") || l.contains("ttnet") {
        Some("tt")
    } else if l.contains("superonline") {
        Some("superonline")
    } else if l.contains("turkcell") {
        Some("turkcell-hotspot")
    } else if l.contains("vodafone") {
        Some("vodafone-hotspot")
    } else if l.contains("kablo") {
        Some("kablonet")
    } else {
        None
    }
}

/// Reorder candidates so the ISP's matching preset is tried first (helps Quick depth find a winner
/// faster). Stable otherwise. Pure → testable. `isp = None` (no consent / unknown) → unchanged.
pub fn prioritize_for_isp(mut cands: Vec<Candidate>, isp: Option<&str>) -> Vec<Candidate> {
    if let Some(pid) = isp.and_then(isp_preset_id) {
        cands.sort_by_key(|c| !(c.engine == "zapret" && c.strategy == pid));
    }
    cands
}

/// Full candidate matrix (item 6.1): every base DPI engine × every strategy/preset it knows. NOT gated
/// by availability (deterministic) — `candidates()` applies the availability filter. Tunnels (WARP) and
/// the composite ByeDPI routing modes are excluded — Auto-Pilot finds a desync *strategy*, not routing.
///  - zapret: all generic strategies + all ISP presets (`engine::strategies()` + `engine::presets()`).
///  - goodbyedpi: each `goodbyedpi::presets()` (mode + ttl + dns).
///  - byedpi: each `byedpi::presets()` (ciadpi param sets).
pub fn candidate_matrix() -> Vec<Candidate> {
    let mut out = Vec::new();
    for s in engine::strategies().iter().chain(engine::presets().iter()) {
        out.push(Candidate { engine: "zapret".into(), strategy: s.id.into() });
    }
    for p in crate::goodbyedpi::presets() {
        out.push(Candidate { engine: "goodbyedpi".into(), strategy: p.id.into() });
    }
    for p in crate::byedpi::presets() {
        out.push(Candidate { engine: "byedpi".into(), strategy: p.id.into() });
    }
    out
}

/// Candidate matrix filtered to engines whose binary is actually available on this machine (so Auto-Pilot
/// only tests engines it can run). zapret presets are applied via `strategy_by_id`; goodbyedpi/byedpi
/// preset application in RunOnce is a follow-up (those engines aren't bundled yet → currently default config).
pub fn candidates() -> Vec<Candidate> {
    candidate_matrix()
        .into_iter()
        .filter(|c| engine::make_engine(&c.engine).is_available())
        .collect()
}

fn is_isp_preset(id: &str) -> bool {
    engine::presets().iter().any(|p| p.id == id)
}

/// Candidate set for a scan depth (item 6.3; deterministic from the full matrix — `run()` filters by
/// availability). Quick = a curated shortlist (zapret `c1` + the 7 ISP presets, the most likely winners);
/// Standard/Force = the full matrix. Quick/Standard stop at the first all-open candidate; Force tries all.
pub fn candidates_for(depth: Depth) -> Vec<Candidate> {
    match depth {
        Depth::Quick => candidate_matrix()
            .into_iter()
            .filter(|c| c.engine == "zapret" && (c.strategy == "c1" || is_isp_preset(&c.strategy)))
            .collect(),
        Depth::Standard | Depth::Force => candidate_matrix(),
    }
}

/// Build a minimal but valid TLS 1.2 ClientHello carrying `host` as the SNI (item 6.2). This is the exact
/// packet a SNI-blocking DPI inspects: if the censor sees the blocked name it RSTs the connection; if the
/// bypass works, a real ServerHello comes back. Std-only (no TLS crate) — we only need the handshake to
/// reach the server, not to complete it.
pub(crate) fn client_hello(host: &str) -> Vec<u8> {
    let host_b = host.as_bytes();
    // SNI extension (type 0x0000)
    let name_len = host_b.len();
    let server_name_list_len = name_len + 3; // entry: type(1) + name_len(2) + name
    let ext_data_len = server_name_list_len + 2; // + list_len(2)
    let mut sni: Vec<u8> = Vec::new();
    sni.extend_from_slice(&0x0000u16.to_be_bytes()); // ext type: server_name
    sni.extend_from_slice(&(ext_data_len as u16).to_be_bytes());
    sni.extend_from_slice(&(server_name_list_len as u16).to_be_bytes());
    sni.push(0x00); // name type: host_name
    sni.extend_from_slice(&(name_len as u16).to_be_bytes());
    sni.extend_from_slice(host_b);

    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(&[0x03, 0x03]); // client_version: TLS 1.2
    body.extend_from_slice(&[0u8; 32]); // random (zeros are fine for a reachability probe)
    body.push(0x00); // session_id length
    let suites: [u8; 8] = [0x13, 0x01, 0x13, 0x02, 0xc0, 0x2f, 0x00, 0x2f]; // a few common suites
    body.extend_from_slice(&(suites.len() as u16).to_be_bytes());
    body.extend_from_slice(&suites);
    body.extend_from_slice(&[0x01, 0x00]); // compression: 1 method, null
    body.extend_from_slice(&(sni.len() as u16).to_be_bytes());
    body.extend_from_slice(&sni);

    let mut hs: Vec<u8> = Vec::with_capacity(body.len() + 4);
    hs.push(0x01); // handshake type: ClientHello
    let blen = body.len();
    hs.extend_from_slice(&[(blen >> 16) as u8, (blen >> 8) as u8, blen as u8]);
    hs.extend_from_slice(&body);

    let mut rec: Vec<u8> = Vec::with_capacity(hs.len() + 5);
    rec.push(0x16); // content type: handshake
    rec.extend_from_slice(&[0x03, 0x01]); // record version: TLS 1.0 (max-compat)
    rec.extend_from_slice(&(hs.len() as u16).to_be_bytes());
    rec.extend_from_slice(&hs);
    rec
}

/// Classify the first response byte after our ClientHello: `0x16` (handshake/ServerHello) or `0x15`
/// (alert) means the server was REACHED — the SNI wasn't reset → open. No bytes / RST / other → blocked.
pub(crate) fn tls_responded(first: Option<u8>) -> bool {
    matches!(first, Some(0x16) | Some(0x15))
}

/// Compute a candidate's score from its per-target results (item 6.2). Opened-count dominates; average
/// latency is a small penalty. Pure → testable; an all-open row always outranks an all-blocked one.
fn compute_score(per_target: &[(String, bool, u32)]) -> i64 {
    let opened = per_target.iter().filter(|(_, ok, _)| *ok).count() as i64;
    let total_ms: u64 = per_target.iter().filter(|(_, ok, _)| *ok).map(|(_, _, ms)| *ms as u64).sum();
    let avg_ms = if opened > 0 { (total_ms / opened as u64) as i64 } else { 0 };
    opened * 100 - avg_ms / 10
}

/// Probe one target: DNS → TCP/443 → TLS ClientHello with SNI → classify the response (item 6.2).
/// Returns (open, ms) where `open` means the TLS handshake was actually answered (not SNI-reset). This
/// distinguishes "TCP connects but DPI resets the TLS" (blocked) from a real reachable endpoint (open).
fn test_target(host: &str) -> (bool, u32) {
    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::{Duration, Instant};
    let hostport = format!("{host}:443");
    let Ok(mut addrs) = hostport.to_socket_addrs() else {
        return (false, 0); // DNS failed
    };
    let Some(addr) = addrs.next() else {
        return (false, 0);
    };
    let st = Instant::now();
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(3)) else {
        return (false, st.elapsed().as_millis().min(u32::MAX as u128) as u32); // TCP blocked
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
    if stream.write_all(&client_hello(host)).is_err() {
        return (false, st.elapsed().as_millis().min(u32::MAX as u128) as u32);
    }
    let mut buf = [0u8; 8];
    let open = match stream.read(&mut buf) {
        Ok(n) if n >= 1 => tls_responded(Some(buf[0])),
        _ => false, // RST / closed / timeout → SNI reset (blocked)
    };
    (open, st.elapsed().as_millis().min(u32::MAX as u128) as u32)
}

/// Bir adayı RunOnce ile uygula → hedefleri test et → durdur → skor satırı. Kalıcı hizmet KURMAZ
/// (geçici motor instance'ı; Drop ile temizlenir). Çağıran aktif motoru önceden duraklatmış olmalı.
pub fn run_once_and_score(cand: &Candidate, targets: &[String]) -> ScoreRow {
    let strat: Strategy = engine::strategy_by_id(&cand.strategy);
    let hostlist: Vec<String> = targets.to_vec();

    // Geçici motor: başlat (idempotent + bundle yoksa sim no-op) → kısa stabilizasyon → test.
    let mut eng = engine::make_engine(&cand.engine);
    let _ = eng.start(&strat, &hostlist);
    std::thread::sleep(std::time::Duration::from_millis(1200)); // desync'in oturması için

    let mut per_target = Vec::new();
    for t in targets {
        let (open, ms) = test_target(t);
        per_target.push((t.clone(), open, ms));
    }
    eng.stop(); // RunOnce: leave no persistent trace

    let score = compute_score(&per_target);
    ScoreRow {
        engine: cand.engine.clone(),
        strategy: cand.strategy.clone(),
        per_target,
        score,
    }
}

/// Tüm adayları dene → skor satırlarını döndür (en yüksek skor başta). `on_progress` her satır
/// tamamlanınca çağrılır (UI canlı tablo). depth=Fast: tüm hedefler açılan ilk adayda dur.
/// Core streaming driver (item 6.5): score each candidate and invoke `on_progress` IMMEDIATELY (so the UI
/// sees rows live, not one blocking batch), honoring early-stop. `score` is injectable so tests can drive
/// it deterministically. Returns the rows sorted best-first.
fn run_with<S, F>(cands: &[Candidate], depth: Depth, mut score: S, mut on_progress: F) -> Vec<ScoreRow>
where
    S: FnMut(&Candidate) -> ScoreRow,
    F: FnMut(&ScoreRow),
{
    let early = depth.early_stop();
    let mut rows: Vec<ScoreRow> = Vec::new();
    for cand in cands {
        let row = score(cand);
        on_progress(&row); // stream this row NOW, before scoring the next candidate
        let all_open = !row.per_target.is_empty() && row.per_target.iter().all(|(_, ok, _)| *ok);
        rows.push(row);
        if early && all_open {
            break; // Quick/Standard: stop at the first fully-working candidate
        }
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.score)); // best first
    rows
}

pub fn run<F: FnMut(&ScoreRow)>(targets: &[String], depth: Depth, on_progress: F) -> Vec<ScoreRow> {
    let cands: Vec<Candidate> = candidates_for(depth)
        .into_iter()
        .filter(|c| engine::make_engine(&c.engine).is_available())
        .collect();
    run_with(&cands, depth, |c| run_once_and_score(c, targets), on_progress)
}

/// Skor satırlarından en iyi adayı bir kaydedilebilir profile çevir (docs/06 §3 "Uygula ve Hatırla").
pub fn best_as_profile(rows: &[ScoreRow], targets: &[String]) -> Option<crate::profile::Profile> {
    let best = rows.first()?;
    Some(crate::profile::Profile {
        schema_version: crate::profile::SCHEMA_VERSION,
        id: "autopilot".into(),
        name: "Auto-Pilot (önerilen)".into(),
        engine: best.engine.clone(),
        isp: detect_isp(false).unwrap_or_default(), // privacy: no auto-lookup when building the profile
        scope: crate::profile::Scope::default(),
        dns: crate::profile::DnsCfg { enabled: true, provider: "cloudflare".into(), doh: true },
        strategy: best.strategy.clone(),
        hostlist: targets.to_vec(),
        engine_params: serde_json::Value::Null,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item 6.1: the candidate matrix covers every base engine × all its strategies/presets.
    #[test]
    fn candidate_matrix_covers_all_presets() {
        let m = candidate_matrix();
        let zapret = engine::strategies().len() + engine::presets().len();
        let gd = crate::goodbyedpi::presets().len();
        let by = crate::byedpi::presets().len();
        assert_eq!(m.len(), zapret + gd + by, "matrix = zapret strategies+presets + gd presets + byedpi presets");
        assert!(m.iter().any(|c| c.engine == "zapret" && c.strategy == "tt"), "ISP preset present");
        assert!(m.iter().any(|c| c.engine == "goodbyedpi"));
        assert!(m.iter().any(|c| c.engine == "byedpi"));
        // composite routing modes and tunnels are excluded
        assert!(!m.iter().any(|c| c.engine.contains("proxifyre") || c.engine == "warp"));
    }

    /// Item 6.2: the ClientHello is a well-formed TLS handshake record carrying the host as SNI.
    #[test]
    fn client_hello_is_valid_tls_with_sni() {
        let h = client_hello("discord.com");
        assert_eq!(h[0], 0x16, "TLS handshake record");
        assert_eq!(&h[1..3], &[0x03, 0x01], "record version TLS 1.0");
        // record length matches the rest of the buffer
        let rec_len = u16::from_be_bytes([h[3], h[4]]) as usize;
        assert_eq!(rec_len, h.len() - 5, "record length consistent");
        assert_eq!(h[5], 0x01, "ClientHello handshake type");
        // the SNI host appears verbatim in the extensions
        let needle = b"discord.com";
        assert!(h.windows(needle.len()).any(|w| w == needle), "SNI host present");
    }

    /// Item 6.2: a TLS response (ServerHello/Alert) counts as reachable; silence/RST counts as blocked.
    #[test]
    fn tls_response_classification() {
        assert!(tls_responded(Some(0x16)), "ServerHello → reachable");
        assert!(tls_responded(Some(0x15)), "Alert → reachable (server answered)");
        assert!(!tls_responded(None), "no bytes → blocked");
        assert!(!tls_responded(Some(0x00)), "junk → blocked");
    }

    /// Item 6.2: scoring distinguishes open from blocked — an all-open candidate outranks an all-blocked one.
    #[test]
    fn score_distinguishes_open_from_blocked() {
        let open = vec![("a".into(), true, 50), ("b".into(), true, 60), ("c".into(), true, 40)];
        let blocked = vec![("a".into(), false, 0), ("b".into(), false, 0), ("c".into(), false, 0)];
        let partial = vec![("a".into(), true, 50), ("b".into(), false, 0), ("c".into(), false, 0)];
        assert!(compute_score(&open) > compute_score(&partial));
        assert!(compute_score(&partial) > compute_score(&blocked));
        assert_eq!(compute_score(&blocked), 0);
    }

    /// Item 6.3: depth parses leniently and picks the right candidate set + early-stop behavior.
    #[test]
    fn depth_levels_differ() {
        assert_eq!(Depth::from_str_lenient("quick"), Depth::Quick);
        assert_eq!(Depth::from_str_lenient("fast"), Depth::Quick);
        assert_eq!(Depth::from_str_lenient("force"), Depth::Force);
        assert_eq!(Depth::from_str_lenient("full"), Depth::Force);
        assert_eq!(Depth::from_str_lenient("standard"), Depth::Standard);
        assert_eq!(Depth::from_str_lenient("anything"), Depth::Standard);

        let q = candidates_for(Depth::Quick);
        let f = candidates_for(Depth::Force);
        assert!(!q.is_empty());
        assert!(q.len() < f.len(), "quick is a strict shortlist of the full matrix");
        assert!(q.iter().all(|c| c.engine == "zapret"), "quick = zapret shortlist");
        assert_eq!(candidates_for(Depth::Standard).len(), f.len(), "standard == force set (differ in early-stop)");
        assert!(!Depth::Force.early_stop());
        assert!(Depth::Quick.early_stop() && Depth::Standard.early_stop());
    }

    /// Item 6.4: ISP detection is opt-in (None without consent); ISP→preset mapping + prioritization work.
    #[test]
    fn isp_detection_opt_in_and_mapping() {
        assert_eq!(detect_isp(false), None, "no consent → no lookup → None");
        assert_eq!(isp_preset_id("AS9121 Turk Telekom"), Some("tt"));
        assert_eq!(isp_preset_id("Turkcell Superonline"), Some("superonline"), "superonline before turkcell");
        assert_eq!(isp_preset_id("Turkcell Iletisim"), Some("turkcell-hotspot"));
        assert_eq!(isp_preset_id("Vodafone Telekomunikasyon"), Some("vodafone-hotspot"));
        assert_eq!(isp_preset_id("Kablonet"), Some("kablonet"));
        assert_eq!(isp_preset_id("Some Other ISP"), None);

        // prioritization moves the ISP preset to the front, stable otherwise
        let cands = candidate_matrix();
        let pri = prioritize_for_isp(cands.clone(), Some("Turk Telekom"));
        assert!(pri[0].engine == "zapret" && pri[0].strategy == "tt", "tt preset first for TT");
        // no ISP → unchanged
        assert_eq!(prioritize_for_isp(cands.clone(), None)[0].strategy, cands[0].strategy);
    }

    /// Item 6.5: run_with streams each row via on_progress immediately and honors early-stop.
    #[test]
    fn run_with_streams_incrementally() {
        let cands = vec![
            Candidate { engine: "zapret".into(), strategy: "a".into() },
            Candidate { engine: "zapret".into(), strategy: "b".into() },
            Candidate { engine: "zapret".into(), strategy: "c".into() },
        ];
        // deterministic scorer: only "b" opens all targets (non-capturing → Copy → reusable)
        let score = |c: &Candidate| ScoreRow {
            engine: c.engine.clone(),
            strategy: c.strategy.clone(),
            per_target: vec![("x".into(), c.strategy == "b", 10)],
            score: if c.strategy == "b" { 100 } else { 0 },
        };
        // Standard early-stops at the first all-open ("b") → streams a, b; c never scored.
        let mut streamed = Vec::new();
        let rows = run_with(&cands, Depth::Standard, score, |r| streamed.push(r.strategy.clone()));
        assert_eq!(streamed, vec!["a", "b"], "rows arrive incrementally; stop at first all-open");
        assert_eq!(rows[0].strategy, "b", "best first after sort");
        // Force scores every candidate (no early stop).
        let mut all = Vec::new();
        run_with(&cands, Depth::Force, score, |r| all.push(r.strategy.clone()));
        assert_eq!(all, vec!["a", "b", "c"]);
    }
}
