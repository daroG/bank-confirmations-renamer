use log::info;
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

    info!("=== Aplikacja Invoice Renamer uruchomiona ===");
    info!("Plik logu: {}", log_file_path.display());

    log_file_path
}
