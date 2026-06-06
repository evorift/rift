use serde::Serialize;
use std::sync::Mutex;

/// Koruma motorunun durumu. (Faz 3'te gerçek WinDivert/servis entegrasyonu buraya gelir.)
#[derive(Default)]
struct Engine {
    running: bool,
    strategy: String,
}

struct AppState(Mutex<Engine>);

#[derive(Serialize, Clone)]
struct Status {
    running: bool,
    strategy: String,
}

impl Engine {
    fn status(&self) -> Status {
        Status {
            running: self.running,
            strategy: self.strategy.clone(),
        }
    }
}

#[tauri::command]
fn protection_status(state: tauri::State<AppState>) -> Status {
    state.0.lock().unwrap().status()
}

#[tauri::command]
fn start_protection(state: tauri::State<AppState>) -> Result<Status, String> {
    let mut e = state.0.lock().unwrap();
    // TODO(Faz 3): yükseltilmiş servise IPC ile "start" gönder →
    //   WinDivert aç + winws stratejisini (c1) + hostlist + UDP ses/QUIC kurallarını uygula.
    if e.strategy.is_empty() {
        e.strategy = "auto".into();
    }
    e.running = true;
    Ok(e.status())
}

#[tauri::command]
fn stop_protection(state: tauri::State<AppState>) -> Result<Status, String> {
    let mut e = state.0.lock().unwrap();
    // TODO(Faz 3): servise "stop" gönder → WinDivert handle kapat, kurallar geri al.
    e.running = false;
    Ok(e.status())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState(Mutex::new(Engine::default())))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            protection_status,
            start_protection,
            stop_protection
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
