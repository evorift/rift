//! evorift-ctl — küçük IPC kontrol/teşhis aracı (hostlist + start/stop/status).
//!
//! Normal kullanıcı çalıştırabilir: pipe SDDL'i IU'ya GRGW, token DACL'i IU'ya FR verir → servise
//! bağlanıp komut gönderebilir (yönetici GEREKMEZ). Asıl ayrıcalıklı iş (WinDivert) servis tarafında.
//! Kullanım:  evorift-ctl on   (çekirdek hostlist'i gönder + korumayı başlat)
//!            evorift-ctl off  (korumayı durdur)
//!            evorift-ctl status

use evorift_lib::client;
use evorift_lib::ipc::Command;

/// Servis yeniden başlayınca hostlist boşalır → desync hiçbir siteye uygulanmaz. "on" bunu yeniden
/// doldurur. (state.svelte.ts CORE_SITES + YouTube ile birebir.)
const CORE: &[&str] = &[
    "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
    "gateway.discord.gg", "cdn.discordapp.com", "roblox.com", "www.roblox.com", "rbxcdn.com",
    "youtube.com", "googlevideo.com",
];

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "status".to_string());
    match arg.as_str() {
        "on" => {
            // No SetHostlist here any more. The protection MODE owns its scope now (service.rs
            // plan_for_mode), so pushing a hostlist first would either be redundant or, worse,
            // silently widen/narrow what the mode says it covers.
            match client::command_status(Command::Start) {
                Ok(s) => println!(
                    "START OK running={} mode={} strategy={} dns={} verify={}",
                    s.running, s.mode, s.strategy, s.dns, s.verify
                ),
                Err(e) => { eprintln!("START HATA: {e}"); std::process::exit(1); }
            }
        }
        // Measure the strategy ladder on this line and deploy the winner. Takes up to a few minutes
        // on a badly blocked line; prints the whole score table so the choice is inspectable.
        "tune" => {
            let targets: Vec<String> = std::env::args().skip(2).collect();
            println!("olculuyor (bu dakikalar surebilir)...");
            match client::command_data(Command::Tune { targets }) {
                Ok(json) => match serde_json::from_str::<serde_json::Value>(&json) {
                    Ok(v) => {
                        println!(
                            "TUNE -> secilen={} network={} gave_up={}",
                            v["strategy"].as_str().unwrap_or("?"),
                            v["network"].as_str().unwrap_or("?"),
                            v["gave_up"].as_bool().unwrap_or(false)
                        );
                        if let Some(rows) = v["rows"].as_array() {
                            println!("  {:<20} {:>7} {:>7} {:>8}", "strategy", "opened", "broke", "avg_ms");
                            for r in rows {
                                println!(
                                    "  {:<20} {:>3}/{:<3} {:>7} {:>8}",
                                    r["strategy"].as_str().unwrap_or("?"),
                                    r["opened"].as_u64().unwrap_or(0),
                                    r["total"].as_u64().unwrap_or(0),
                                    r["broke"].as_u64().unwrap_or(0),
                                    r["avg_ms"].as_u64().unwrap_or(0),
                                );
                            }
                        }
                    }
                    Err(e) => println!("TUNE ham cikti ({e}): {json}"),
                },
                Err(e) => { eprintln!("TUNE HATA: {e}"); std::process::exit(1); }
            }
        }
        // Structured engine events — the log the UI shows. Useful headless when something failed
        // and the only other evidence would be a file on a machine you are not sitting at.
        "events" => {
            let since: u64 = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(0);
            match client::command_data(Command::Events { since }) {
                Ok(json) => match serde_json::from_str::<serde_json::Value>(&json) {
                    Ok(v) => {
                        for e in v["events"].as_array().into_iter().flatten() {
                            println!(
                                "[{}] {:<5} {}/{}: {}",
                                e["at"].as_str().unwrap_or(""),
                                e["level"].as_str().unwrap_or(""),
                                e["module"].as_str().unwrap_or(""),
                                e["code"].as_str().unwrap_or(""),
                                e["message"].as_str().unwrap_or("")
                            );
                        }
                        println!("watermark={}", v["watermark"].as_u64().unwrap_or(0));
                    }
                    Err(e) => println!("EVENTS ham cikti ({e}): {json}"),
                },
                Err(e) => { eprintln!("EVENTS HATA: {e}"); std::process::exit(1); }
            }
        }
        // Delete everything stored locally — the headless equivalent of Settings → "Tüm verileri
        // sil". Prints the service's own {removed, remaining} report rather than "done": a wipe
        // that silently left files behind is exactly the failure this command exists to detect.
        "wipe" => match client::command_data(Command::WipeLocalData) {
            Ok(json) => match serde_json::from_str::<serde_json::Value>(&json) {
                Ok(v) => {
                    let list = |k: &str| {
                        v[k].as_array()
                            .map(|a| a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(", "))
                            .unwrap_or_default()
                    };
                    let remaining = list("remaining");
                    println!("WIPE removed=[{}]", list("removed"));
                    if remaining.is_empty() {
                        println!("WIPE remaining=[] (nothing left behind)");
                    } else {
                        println!("WIPE remaining=[{remaining}]  <-- NOT deleted");
                        std::process::exit(1);
                    }
                }
                Err(e) => println!("WIPE ham cikti ({e}): {json}"),
            },
            Err(e) => { eprintln!("WIPE HATA: {e}"); std::process::exit(1); }
        },
        // Should protection come back on its own after a reboot?
        "autostart" => {
            let enable = !matches!(std::env::args().nth(2).as_deref(), Some("off" | "false" | "0"));
            match client::command_status(Command::SetAutoStart { enable }) {
                Ok(_) => println!("AUTOSTART -> {}", if enable { "on" } else { "off" }),
                Err(e) => { eprintln!("AUTOSTART HATA: {e}"); std::process::exit(1); }
            }
        }
        "off" => match client::command_status(Command::Stop) {
            Ok(s) => println!("STOP OK running={}", s.running),
            Err(e) => { eprintln!("STOP HATA: {e}"); std::process::exit(1); }
        },
        // Güvenli DNS'i uygula (cloudflare|quad9|adguard|google). Headless doğrulama için şart:
        // DNS zehirlenmiş bir hatta desync tek başına hiçbir şey çözmez (yanlış IP'ye bağlanılır).
        "dns" => {
            let profile = std::env::args().nth(2).unwrap_or_else(|| "cloudflare".to_string());
            match client::command_status(Command::SetDns { profile: profile.clone() }) {
                Ok(s) => println!("DNS OK -> {profile} (status dns={})", s.dns),
                Err(e) => { eprintln!("DNS HATA: {e}"); std::process::exit(1); }
            }
        }
        // Proof-of-protection probe'unu TEK BAŞINA çalıştır (servis/IPC/yönetici GEREKMEZ).
        // UI "Doğrulanmadı"da takılırsa asıl soru "probe bitiyor mu, ne kadar sürüyor" — bunu
        // servisin içinden göremiyoruz; burada hedef hedef süre + sonuç basılır.
        "probe" => {
            let t0 = std::time::Instant::now();
            let out = evorift_lib::verify::probe_discord();
            println!(
                "PROBE ok={} sure={}ms reason={}",
                out.ok,
                t0.elapsed().as_millis(),
                if out.reason.is_empty() { "-" } else { &out.reason }
            );
        }
        // Kullanıcıya görünen koruma modu (hafif|guclu) — UI'daki kartların tam karşılığı, tek
        // atomik komut. Headless test için: canlı doğrulamada modu UI olmadan değiştirebilmek şart.
        // NOTE: not aliased to "mode" — that verb is already taken below by the live-verification
        // dpi/warp-split/warp-full switch, and shadowing it would have made one of the two silently
        // unreachable (the compiler caught exactly that).
        "protmode" => {
            let mode = std::env::args().nth(2).unwrap_or_else(|| "hafif".to_string());
            match client::command_status(Command::SetProtectionMode { mode: mode.clone() }) {
                Ok(s) => println!(
                    "PROTMODE OK -> {mode} (running={} strategy={} verify={})",
                    s.running, s.strategy, s.verify
                ),
                Err(e) => { eprintln!("PROTMODE HATA: {e}"); std::process::exit(1); }
            }
        }
        "strat" => {
            // Stratejiyi canlı değiştir (servis çalışırken anında uygular). "multidisorder" = UDP/QUIC
            // desync YOK (sadece TCP) → QUIC kullanan masaüstü uygulamalarını (Discord/Electron) bozmaz.
            //
            // --repeats=N (live-verification dpi-desync-repeats sweep): overrides the resolved strategy's
            // PRIMARY TLS/443 repeat count before it reaches Strategy::tls_profile_args() (service.rs
            // current_strategy()) — still the engine's own arg builder, never a hand-written winws line.
            let rest: Vec<String> = std::env::args().skip(2).collect();
            let id = rest.iter().find(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "auto".to_string());
            let repeats_override = rest.iter().find_map(|a| a.strip_prefix("--repeats=")).map(|v| {
                v.parse::<u32>().unwrap_or_else(|_| {
                    eprintln!("STRAT HATA: gecersiz --repeats degeri: {v}");
                    std::process::exit(1);
                })
            });
            match client::command_status(Command::SetStrategy { id, repeats_override }) {
                Ok(s) => println!(
                    "STRAT OK -> {} repeats_override={:?} (running={})",
                    s.strategy, repeats_override, s.running
                ),
                Err(e) => { eprintln!("STRAT HATA: {e}"); std::process::exit(1); }
            }
        }
        "mode" => {
            // Live-verification only: force a verifiable DPI-only / WARP-split / WARP-full state (today's
            // `evorift-ctl on` alone does NOT guarantee DPI-only — service.rs want_warp() defaults to true
            // when app_modes is empty, so WARP split-tunnel silently engages alongside winws).
            let sub = std::env::args().nth(2).unwrap_or_default();
            match sub.as_str() {
                "dpi" => {
                    // want_warp() is false only when app_modes is NON-empty and contains no "warp" entry
                    // (service.rs Engine::want_warp). full_warp must also be off, or warp_target() still
                    // returns Some(true) from a prior warp-full run.
                    if let Err(e) = client::command(Command::SetFullWarp { enable: false }) {
                        eprintln!("MODE HATA (full_warp off): {e}"); std::process::exit(1);
                    }
                    let modes = vec![("evoriftctl".to_string(), "dpi".to_string(), String::new())];
                    match client::command(Command::SetAppModes { modes }) {
                        Ok(_) => println!("MODE OK -> dpi (warp forced off)"),
                        Err(e) => { eprintln!("MODE HATA: {e}"); std::process::exit(1); }
                    }
                }
                "warp-split" | "warp-full" => {
                    let full = sub == "warp-full";
                    let domains: Vec<String> = CORE.iter().map(|s| s.to_string()).collect();
                    let json = warp_profile_json(&sub, full, &domains);
                    if let Err(e) = client::command(Command::SaveProfile { json }) {
                        eprintln!("MODE HATA (save): {e}"); std::process::exit(1);
                    }
                    match client::command_status(Command::ApplyProfile { id: sub.clone() }) {
                        Ok(s) => println!("MODE OK -> {sub} (engine={} running={})", s.engine, s.running),
                        Err(e) => { eprintln!("MODE HATA (apply): {e}"); std::process::exit(1); }
                    }
                }
                _ => {
                    eprintln!("kullanim: evorift-ctl mode dpi|warp-split|warp-full");
                    std::process::exit(1);
                }
            }
        }
        // ---- Blueprint genişletmesi (docs/07) ----
        "strats" => {
            // Local data dump (no IPC): generic strategies + per-ISP presets and the args each builds.
            use evorift_lib::engine;
            println!("# Generic strategies:");
            for s in engine::strategies() {
                println!("  {:<18} {}", s.id, s.tls_profile_args().join(" "));
            }
            println!("# ISP presets (docs/03 §4.1):");
            for s in engine::presets() {
                println!(
                    "  {:<18} wf-tcp={} wf-udp={} | {}",
                    s.id,
                    s.wf_tcp,
                    s.wf_udp,
                    s.tls_profile_args().join(" ")
                );
            }
        }
        "gd-presets" => {
            // Local data dump (no IPC): GoodbyeDPI preset catalog (docs/03 §5.1) + the args each builds.
            use evorift_lib::goodbyedpi::{self, GoodbyeDpiEngine};
            use evorift_lib::engine::BypassEngine;
            for p in goodbyedpi::presets() {
                let mut eng = GoodbyeDpiEngine::new();
                eng.apply_preset(&p);
                let args = eng.build_args(&evorift_lib::engine::strategy_by_id("auto"), &[]);
                println!("  {:<12} {}", p.id, args.join(" "));
            }
        }
        "services" => {
            // Local read-only dump of managed DPI/tunnel service states (docs/02 §4).
            for s in evorift_lib::services::list() {
                println!("  {:<26} {}", s.name, s.state);
            }
        }
        "engines" => print_data(Command::EngineCatalog),
        "profiles" => print_data(Command::ListProfiles),
        "preflight" => print_data(Command::Preflight),
        "verifydns" => print_data(Command::VerifyDns),
        "apply" => {
            let id = std::env::args().nth(2).unwrap_or_default();
            match client::command_status(Command::ApplyProfile { id }) {
                Ok(s) => println!("APPLY OK engine={} strategy={} running={}", s.engine, s.strategy, s.running),
                Err(e) => { eprintln!("APPLY HATA: {e}"); std::process::exit(1); }
            }
        }
        "engine" => {
            let id = std::env::args().nth(2).unwrap_or_else(|| "zapret".to_string());
            match client::command_status(Command::SetEngine { id }) {
                Ok(s) => println!("ENGINE OK -> {} (running={})", s.engine, s.running),
                Err(e) => { eprintln!("ENGINE HATA: {e}"); std::process::exit(1); }
            }
        }
        "diag" => {
            let targets: Vec<String> = CORE.iter().map(|s| s.to_string()).collect();
            print_data(Command::Diagnose { targets });
        }
        "auto" => {
            // Targets from args (core list if none). A depth word (quick/standard/force/full/fast) anywhere
            // in the args sets the scan depth; default standard.
            const DEPTHS: &[&str] = &["quick", "standard", "force", "full", "fast"];
            let depth = std::env::args()
                .skip(2)
                .find(|a| DEPTHS.contains(&a.as_str()))
                .unwrap_or_else(|| "standard".to_string());
            let extra: Vec<String> = std::env::args().skip(2).filter(|a| !DEPTHS.contains(&a.as_str())).collect();
            let targets = if extra.is_empty() { CORE.iter().map(|s| s.to_string()).collect() } else { extra };
            print_data(Command::AutoPilot { targets, depth });
        }
        "rollback" => match client::command(Command::RollbackAll) {
            Ok(_) => println!("ROLLBACK OK"),
            Err(e) => { eprintln!("ROLLBACK HATA: {e}"); std::process::exit(1); }
        },
        // verify/verify_reason DAHİL: "çalışıyor" ile "gerçekten geçiyor" farkı bu alanda; canlı
        // doğrulamada bunu görmeden durum hakkında konuşulamaz (UI'ın gördüğü alanın aynısı).
        _ => match client::command_status(Command::Status) {
            Ok(s) => {
                println!(
                    "STATUS running={} state={} mode={} strategy={} dns={} verify={} harm={} \
                     targets={}/{} tuning={} tuned={} reason={}",
                    s.running, s.state, s.mode, s.strategy, s.dns, s.verify, s.harm,
                    s.targets_ok, s.targets_total, s.tuning, s.tuned_strategy, s.verify_reason
                );
                // Per-site detail: the whole reason a "verified" status could be a lie before was
                // that nobody could see WHICH sites the verdict was based on.
                for site in &s.sites {
                    println!(
                        "  {:<26} {:<7} {:>5}ms {}{}",
                        site.host,
                        if site.ok { "OK" } else { "FAIL" },
                        site.ms,
                        if site.control { "[control] " } else { "" },
                        site.reason
                    );
                }
                for p in &s.problems {
                    println!("  ! [{}] {}/{}: {}", p.at, p.module, p.code, p.message);
                }
            }
            Err(e) => { eprintln!("STATUS HATA: {e}"); std::process::exit(1); }
        },
    }
}

/// Build a WARP tunnel profile (live-verification `mode warp-split|warp-full`) through the real
/// `profile::Profile` struct — serialized, never a hand-written JSON string — so it always matches
/// profile.rs's schema and passes `Command::SaveProfile`'s validation.
fn warp_profile_json(id: &str, full: bool, hostlist: &[String]) -> String {
    use evorift_lib::profile::{DnsCfg, Profile, Scope, ScopeMode, SCHEMA_VERSION};
    let prof = Profile {
        schema_version: SCHEMA_VERSION,
        id: id.to_string(),
        name: format!("Live-verification {id}"),
        engine: "warp".to_string(),
        isp: String::new(),
        scope: Scope {
            mode: if full { ScopeMode::System } else { ScopeMode::Split },
            apps: vec![],
            browsers: false,
            folders: vec![],
        },
        dns: DnsCfg::default(),
        strategy: String::new(),
        hostlist: hostlist.to_vec(),
        engine_params: serde_json::Value::Null,
    };
    serde_json::to_string(&prof).unwrap_or_else(|e| {
        eprintln!("MODE HATA (profil serileştirilemedi): {e}");
        std::process::exit(1);
    })
}

/// Yapılandırılmış sorgu komutu gönder, JSON sonucunu yazdır.
fn print_data(cmd: Command) {
    match client::command_data(cmd) {
        Ok(json) => println!("{json}"),
        Err(e) => { eprintln!("HATA: {e}"); std::process::exit(1); }
    }
}
