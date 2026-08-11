//! Tauri UI tarafı IPC istemcisi: ayrıcalıklı servise pipe üzerinden komut yollar (docs/05 §2).
//!
//! Her çağrı: bağlan → token handshake → komut → yanıt. Düşük frekanslı olduğu için
//! bağlantı komut başına açılır/kapanır (iskelet).

use crate::ipc::{self, Command, EngineStatus, Metrics, Request, Response, PIPE_NAME};
use interprocess::local_socket::{prelude::*, GenericNamespaced, Stream};
use std::io::BufReader;

fn read_token() -> Result<String, String> {
    // başlangıç yarışına karşı küçük yeniden deneme
    for _ in 0..20 {
        if let Ok(t) = std::fs::read_to_string(ipc::token_path()) {
            let t = t.trim().to_string();
            if !t.is_empty() {
                return Ok(t);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    Err("servis token'ı bulunamadı".into())
}

/// Inner: one connection attempt — connect, handshake, send command, receive response.
fn command_once(cmd: &Command) -> Result<Response, String> {
    let token = read_token()?;
    let name = PIPE_NAME
        .to_ns_name::<GenericNamespaced>()
        .map_err(|e| e.to_string())?;
    let conn = Stream::connect(name).map_err(|e| format!("servise ulaşılamadı: {e}"))?;
    let mut reader = BufReader::new(&conn);

    // handshake
    ipc::write_msg(&conn, &Request::Hello { token }).map_err(|e| e.to_string())?;
    match ipc::read_msg::<Response>(&mut reader)? {
        Response::Ok => {}
        Response::Error { message } => return Err(message),
        _ => return Err("beklenmeyen handshake yanıtı".into()),
    }

    // komut
    ipc::write_msg(&conn, &Request::Command { cmd: cmd.clone() }).map_err(|e| e.to_string())?;
    ipc::read_msg::<Response>(&mut reader)
}

/// Servise tek komut gönder, yanıtı döndür.
/// On REJECT (token stale — service restarted and wrote a new token) retries once
/// after a short pause so a single service restart doesn't surface as a UI error.
pub fn command(cmd: Command) -> Result<Response, String> {
    match command_once(&cmd) {
        Err(e) if e.contains("reddedildi") || e.contains("rejected") => {
            // The service may have restarted and written a new token. Re-read and retry once.
            std::thread::sleep(std::time::Duration::from_millis(150));
            command_once(&cmd)
        }
        r => r,
    }
}

/// Kısayol: komutu gönder, `EngineStatus` çıkar (yanıt Status değilse hata).
pub fn command_status(cmd: Command) -> Result<EngineStatus, String> {
    match command(cmd)? {
        Response::Status(s) => Ok(s),
        Response::Ok => Ok(EngineStatus::default()),
        Response::Error { message } => Err(message),
        Response::Telemetry(_) => Err("beklenmeyen telemetri yanıtı".into()),
        Response::Data(_) => Err("beklenmeyen veri yanıtı".into()),
    }
}

/// Kısayol: yapılandırılmış sorgu komutu gönder, `Data` JSON string'ini çıkar (profil/katalog/
/// preflight/teşhis/autopilot). Ok → boş JSON; Status → hata (sorgu değil).
pub fn command_data(cmd: Command) -> Result<String, String> {
    match command(cmd)? {
        Response::Data(json) => Ok(json),
        Response::Ok => Ok(String::new()),
        Response::Error { message } => Err(message),
        Response::Status(_) => Err("beklenmeyen durum yanıtı".into()),
        Response::Telemetry(_) => Err("beklenmeyen telemetri yanıtı".into()),
    }
}

/// Auto-Pilot streaming (item 6.5): connect, run the scan, and invoke `on_row` for each `ScoreRow` JSON
/// as it arrives (live UI). Returns when the server sends its terminal `Ok` (or on error/disconnect).
pub fn autopilot_stream<F: FnMut(String)>(targets: Vec<String>, depth: String, mut on_row: F) -> Result<(), String> {
    let token = read_token()?;
    let name = PIPE_NAME
        .to_ns_name::<GenericNamespaced>()
        .map_err(|e| e.to_string())?;
    let conn = Stream::connect(name).map_err(|e| format!("servise ulaşılamadı: {e}"))?;
    let mut reader = BufReader::new(&conn);

    ipc::write_msg(&conn, &Request::Hello { token }).map_err(|e| e.to_string())?;
    match ipc::read_msg::<Response>(&mut reader)? {
        Response::Ok => {}
        Response::Error { message } => return Err(message),
        _ => return Err("beklenmeyen handshake yanıtı".into()),
    }

    ipc::write_msg(&conn, &Request::AutoPilotStream { targets, depth }).map_err(|e| e.to_string())?;
    loop {
        match ipc::read_msg::<Response>(&mut reader) {
            Ok(Response::Data(j)) => on_row(j), // a streamed ScoreRow (JSON)
            Ok(Response::Ok) => break,          // terminal done marker
            Ok(Response::Error { message }) => return Err(message),
            Ok(_) => {}
            Err(_) => break, // disconnected
        }
    }
    Ok(())
}

/// Telemetri akışına abone ol; her gelen `Metrics` için `on_metrics` çağrılır.
/// Bağlantı kapanınca döner (çağıran yeniden bağlanabilir).
pub fn subscribe<F: FnMut(Metrics)>(mut on_metrics: F) -> Result<(), String> {
    let token = read_token()?;
    let name = PIPE_NAME
        .to_ns_name::<GenericNamespaced>()
        .map_err(|e| e.to_string())?;
    let conn = Stream::connect(name).map_err(|e| format!("servise ulaşılamadı: {e}"))?;
    let mut reader = BufReader::new(&conn);

    ipc::write_msg(&conn, &Request::Hello { token }).map_err(|e| e.to_string())?;
    match ipc::read_msg::<Response>(&mut reader)? {
        Response::Ok => {}
        Response::Error { message } => return Err(message),
        _ => return Err("beklenmeyen handshake yanıtı".into()),
    }

    ipc::write_msg(&conn, &Request::Subscribe).map_err(|e| e.to_string())?;
    loop {
        match ipc::read_msg::<Response>(&mut reader) {
            Ok(Response::Telemetry(m)) => on_metrics(m),
            Ok(_) => {}
            Err(_) => break, // bağlantı kapandı
        }
    }
    Ok(())
}
