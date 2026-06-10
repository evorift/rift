//! evorift servis kontrol CLI — EvoriftSvc'ye pipe ile Start/Stop/Status yollar (test/tanılama).
//! UI olmadan servisin gerçekten çalışıp çalışmadığını doğrular.
//!
//! Çalıştırma:  cargo run --example svc_ctl -- start | stop | status
//! "start" önce bypass hostlist'ini (discord/roblox) yollar, sonra motoru başlatır.

use evorift_lib::client;
use evorift_lib::ipc::Command;

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "status".into());
    // opsiyonel 2. argüman = strateji (start için): `svc_ctl start c1|multidisorder|fake|auto`
    let strat = std::env::args().nth(2);

    // `svc_ctl strat <id>` → yalnız stratejiyi değiştir (motor çalışırken anında yansır)
    if arg == "strat" {
        if let Some(id) = strat {
            match client::command_status(Command::SetStrategy { id: id.clone() }) {
                Ok(s) => println!("[svc_ctl] strateji={} · running={}", id, s.running),
                Err(e) => { eprintln!("[svc_ctl] HATA: {e}"); std::process::exit(1); }
            }
        } else {
            eprintln!("kullanım: svc_ctl strat <c1|multidisorder|fake|auto>");
            std::process::exit(1);
        }
        return;
    }

    if arg == "start" {
        // strateji verildiyse start'tan önce uygula
        if let Some(id) = &strat {
            let _ = client::command_status(Command::SetStrategy { id: id.clone() });
            eprintln!("[svc_ctl] strateji ayarlandı: {id}");
        }
        // motor yalnız listedeki domainleri parçalar (yanlış-pozitif yok) — app.toggle ile aynı akış
        let domains: Vec<String> = [
            "discord.com", "discordapp.com", "discord.gg", "discordapp.net", "discord.media",
            "gateway.discord.gg", "cdn.discordapp.com", "roblox.com", "www.roblox.com", "rbxcdn.com",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        match client::command(Command::SetHostlist { domains }) {
            Ok(_) => eprintln!("[svc_ctl] hostlist gönderildi (10 domain)"),
            Err(e) => {
                eprintln!("[svc_ctl] hostlist HATA: {e}");
                std::process::exit(1);
            }
        }
    }

    let cmd = match arg.as_str() {
        "start" => Command::Start,
        "stop" => Command::Stop,
        _ => Command::Status,
    };

    match client::command_status(cmd) {
        Ok(s) => println!("[svc_ctl] OK · running={} strateji={} dns={}", s.running, s.strategy, s.dns),
        Err(e) => {
            eprintln!("[svc_ctl] HATA: {e}");
            std::process::exit(1);
        }
    }
}
