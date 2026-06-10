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

/// Servise tek komut gönder, yanıtı döndür.
pub fn command(cmd: Command) -> Result<Response, String> {
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
    ipc::write_msg(&conn, &Request::Command { cmd }).map_err(|e| e.to_string())?;
    ipc::read_msg::<Response>(&mut reader)
}

/// Kısayol: komutu gönder, `EngineStatus` çıkar (yanıt Status değilse hata).
pub fn command_status(cmd: Command) -> Result<EngineStatus, String> {
    match command(cmd)? {
        Response::Status(s) => Ok(s),
        Response::Ok => Ok(EngineStatus::default()),
        Response::Error { message } => Err(message),
        Response::Telemetry(_) => Err("beklenmeyen telemetri yanıtı".into()),
    }
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
