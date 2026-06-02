use log::{info, error};
use std::env;

#[cfg(target_os = "windows")]
pub fn setup_autostart() -> auto_launch::AutoLaunch {
    let exe_path = env::current_exe()
        .expect("Nie można uzyskać ścieżki do pliku wykonywalnego");

    auto_launch::AutoLaunchBuilder::new()
        .set_app_name("InvoiceRenamer")
        .set_app_path(&exe_path.to_string_lossy())
        .build()
        .expect("Nie można utworzyć konfiguracji auto-startu")
}

#[cfg(target_os = "windows")]
pub fn is_enabled() -> bool {
    let auto = setup_autostart();
    match auto.is_enabled() {
        Ok(enabled) => enabled,
        Err(e) => {
            error!("Błąd sprawdzania auto-startu: {}", e);
            false
        }
    }
}

#[cfg(target_os = "windows")]
pub fn enable() -> Result<(), String> {
    info!("Włączanie auto-startu aplikacji");
    let auto = setup_autostart();

    match auto.enable() {
        Ok(_) => {
            info!("Auto-start został włączony");
            Ok(())
        }
        Err(e) => {
            error!("Nie można włączyć auto-startu: {}", e);
            Err(format!("Nie można włączyć auto-startu: {}", e))
        }
    }
}

#[cfg(target_os = "windows")]
pub fn disable() -> Result<(), String> {
    info!("Wyłączanie auto-startu aplikacji");
    let auto = setup_autostart();

    match auto.disable() {
        Ok(_) => {
            info!("Auto-start został wyłączony");
            Ok(())
        }
        Err(e) => {
            error!("Nie można wyłączyć auto-startu: {}", e);
            Err(format!("Nie można wyłączyć auto-startu: {}", e))
        }
    }
}

#[cfg(target_os = "windows")]
pub fn toggle() -> Result<bool, String> {
    if is_enabled() {
        disable()?;
        Ok(false)
    } else {
        enable()?;
        Ok(true)
    }
}

// Stub implementations for non-Windows platforms
#[cfg(not(target_os = "windows"))]
pub fn is_enabled() -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn enable() -> Result<(), String> {
    Err("Auto-start is only supported on Windows".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn disable() -> Result<(), String> {
    Err("Auto-start is only supported on Windows".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn toggle() -> Result<bool, String> {
    Err("Auto-start is only supported on Windows".to_string())
}
