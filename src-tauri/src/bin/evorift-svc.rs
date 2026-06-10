//! evorift ayrıcalıklı servisi (LocalSystem) — named-pipe IPC sunucusu (docs/05 §1).
//!
//! İki mod:
//!   - SCM (Windows servisi) tarafından başlatılınca: `service_dispatcher` üzerinden çalışır.
//!   - `--console` ile veya SCM olmadan: ön planda `serve_blocking()` (geliştirme).
//!
//! Kurulum (üretim, installer yapar): `sc create EvoriftSvc binPath= "...\evorift-svc.exe" start= auto`
//! ardından `sc start EvoriftSvc`. Bu iskelette gerçek WinDivert işleri henüz yok (bkz. service.rs).

#[cfg(windows)]
mod svc {
    use std::ffi::OsString;
    use std::sync::mpsc;
    use std::time::Duration;
    use windows_service::service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    };
    use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
    use windows_service::{define_windows_service, service_dispatcher};

    const SERVICE_NAME: &str = "EvoriftSvc";

    define_windows_service!(ffi_service_main, service_main);

    fn service_main(_args: Vec<OsString>) {
        if let Err(e) = run_service() {
            eprintln!("[evorift-svc] servis hatası: {e}");
        }
    }

    fn run_service() -> windows_service::Result<()> {
        let (stop_tx, stop_rx) = mpsc::channel();

        let status_handle = service_control_handler::register(SERVICE_NAME, move |control| {
            match control {
                ServiceControl::Stop => {
                    let _ = stop_tx.send(());
                    ServiceControlHandlerResult::NoError
                }
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        })?;

        let running = |state: ServiceState, accept: ServiceControlAccept| ServiceStatus {
            service_type: ServiceType::OWN_PROCESS,
            current_state: state,
            controls_accepted: accept,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        };

        status_handle.set_service_status(running(ServiceState::Running, ServiceControlAccept::STOP))?;

        // pipe sunucusunu arka planda çalıştır; süreç sonlanınca biter
        std::thread::spawn(|| {
            let _ = crate::serve();
        });

        let _ = stop_rx.recv(); // STOP gelene kadar bekle
        status_handle
            .set_service_status(running(ServiceState::Stopped, ServiceControlAccept::empty()))?;
        Ok(())
    }

    pub fn main_impl() {
        let console = std::env::args().any(|a| a == "--console");
        if console {
            let _ = crate::serve();
            return;
        }
        // SCM ile başlatmayı dene; konsoldan çalıştırıldıysa (Err) ön plana düş
        if service_dispatcher::start(SERVICE_NAME, ffi_service_main).is_err() {
            eprintln!("[evorift-svc] SCM yok — konsol modunda çalışıyor (--console)");
            let _ = crate::serve();
        }
    }
}

fn serve() -> std::io::Result<()> {
    // Bu süreç ayrıcalıklı servistir → gerçek OS komutlarını çalıştırmaya yetkili (docs/05 §5).
    std::env::set_var("EVORIFT_PRIVILEGED", "1");
    evorift_lib::service::serve_blocking()
}

fn main() {
    #[cfg(windows)]
    {
        svc::main_impl();
    }
    #[cfg(not(windows))]
    {
        let _ = serve();
    }
}
