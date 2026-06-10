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
        _ => match client::command_status(Command::Status) {
            Ok(s) => println!("STATUS running={} strategy={} dns={}", s.running, s.strategy, s.dns),
            Err(e) => { eprintln!("STATUS HATA: {e}"); std::process::exit(1); }
        },
    }
}
