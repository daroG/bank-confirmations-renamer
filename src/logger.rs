use log::{info, error};
use std::path::PathBuf;

pub fn init_logger() -> PathBuf {
    // Initialize logger - logs to file in config directory
    let log_file_path = {
        let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push("invoices-renamer");
        std::fs::create_dir_all(&path).ok();
        path.push("app.log");
        path
    };

    // Configure logging
    use std::io::Write;
    let target = Box::new(std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .expect("Nie można utworzyć pliku logu"));

    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(target))
        .filter_level(log::LevelFilter::Info)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {} {}:{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();

    // Route panics to the log file instead of the raw console.
    // Panics from pdf-extract (e.g. "missing unicode map and encoding") are
    // caught by panic::catch_unwind in pdf_processor, but the default panic
    // hook still prints an alarming "thread 'main' panicked" message to stderr
    // before the unwind is caught. This hook records the panic in the log so
    // handled panics are documented rather than looking like a crash.
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown".to_string());

        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "nieznany powód paniki".to_string()
        };

        error!("Przechwycono panikę w {}: {}", location, message);
    }));

    info!("=== Aplikacja Invoice Renamer uruchomiona ===");
    info!("Plik logu: {}", log_file_path.display());

    log_file_path
}
