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
            let domains: Vec<String> = CORE.iter().map(|s| s.to_string()).collect();
            match client::command(Command::SetHostlist { domains }) {
                Ok(_) => println!("hostlist: {} domain gonderildi", CORE.len()),
                Err(e) => eprintln!("hostlist HATA: {e}"),
            }
            match client::command_status(Command::Start) {
                Ok(s) => println!("START OK running={} strategy={} dns={}", s.running, s.strategy, s.dns),
                Err(e) => { eprintln!("START HATA: {e}"); std::process::exit(1); }
            }
        }
        "off" => match client::command_status(Command::Stop) {
            Ok(s) => println!("STOP OK running={}", s.running),
            Err(e) => { eprintln!("STOP HATA: {e}"); std::process::exit(1); }
        },
        "strat" => {
            // Stratejiyi canlı değiştir (servis çalışırken anında uygular). "multidisorder" = UDP/QUIC
            // desync YOK (sadece TCP) → QUIC kullanan masaüstü uygulamalarını (Discord/Electron) bozmaz.
            let id = std::env::args().nth(2).unwrap_or_else(|| "auto".to_string());
            match client::command_status(Command::SetStrategy { id }) {
                Ok(s) => println!("STRAT OK -> {} (running={})", s.strategy, s.running),
                Err(e) => { eprintln!("STRAT HATA: {e}"); std::process::exit(1); }
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
        _ => match client::command_status(Command::Status) {
            Ok(s) => println!(
                "STATUS running={} state={} engine={} strategy={} dns={}",
                s.running, s.state, s.engine, s.strategy, s.dns
            ),
            Err(e) => { eprintln!("STATUS HATA: {e}"); std::process::exit(1); }
        },
    }
}

/// Yapılandırılmış sorgu komutu gönder, JSON sonucunu yazdır.
fn print_data(cmd: Command) {
    match client::command_data(cmd) {
        Ok(json) => println!("{json}"),
        Err(e) => { eprintln!("HATA: {e}"); std::process::exit(1); }
    }
}
