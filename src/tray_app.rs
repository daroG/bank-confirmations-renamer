use log::{info, warn, error};
use std::sync::{Arc, Mutex};
use tray_icon::{TrayIcon, TrayIconBuilder, Icon, menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, MenuId}};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use crate::config::Config;
use crate::file_watcher::{WatcherHandle, start_watching};
use crate::icon_generator;
use crate::autostart;

pub struct TrayApp {
    config: Arc<Mutex<Config>>,
    watchers: Arc<Mutex<Vec<WatcherHandle>>>,
    add_dir_id: MenuId,
    manage_dirs_id: MenuId,
    reload_id: MenuId,
    autostart_id: MenuId,
    quit_id: MenuId,
    _tray_icon: Option<TrayIcon>,
}

impl TrayApp {
    pub fn new() -> Self {
        // Load configuration
        info!("Ładowanie konfiguracji aplikacji");
        let config = Arc::new(Mutex::new(
            Config::load().unwrap_or_else(|e| {
                warn!("Nie można załadować konfiguracji: {}, używam domyślnej", e);
                Config::default()
            })
        ));

        // Create tray menu
        let tray_menu = Menu::new();

        let add_dir_item = MenuItem::new("Dodaj katalog do obserwacji", true, None);
        let manage_dirs_item = MenuItem::new("Zarządzaj katalogami", true, None);
        let reload_item = MenuItem::new("Przeładuj", true, None);

        // Auto-start menu item with checkmark
        let autostart_enabled = autostart::is_enabled();
        let autostart_label = if autostart_enabled {
            "✓ Uruchamiaj przy starcie systemu"
        } else {
            "Uruchamiaj przy starcie systemu"
        };
        let autostart_item = MenuItem::new(autostart_label, true, None);

        let separator = PredefinedMenuItem::separator();
        let quit_item = MenuItem::new("Zakończ", true, None);

        let add_dir_id = add_dir_item.id().clone();
        let manage_dirs_id = manage_dirs_item.id().clone();
        let reload_id = reload_item.id().clone();
        let autostart_id = autostart_item.id().clone();
        let quit_id = quit_item.id().clone();

        tray_menu.append(&add_dir_item).ok();
        tray_menu.append(&manage_dirs_item).ok();
        tray_menu.append(&reload_item).ok();
        tray_menu.append(&separator).ok();
        tray_menu.append(&autostart_item).ok();
        tray_menu.append(&separator).ok();
        tray_menu.append(&quit_item).ok();

        // Create tray icon
        let icon_rgba = icon_generator::generate_icon_data();
        let icon = Icon::from_rgba(
            image::load_from_memory(&icon_rgba)
                .expect("Failed to load icon")
                .to_rgba8()
                .into_raw(),
            32,
            32,
        )
        .expect("Failed to create icon");

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Invoice Renamer")
            .with_icon(icon)
            .build()
            .ok();

        // Start watching configured directories
        let watchers = Arc::new(Mutex::new(Vec::new()));
        {
            let config_lock = config.lock().unwrap();
            let dirs = config_lock.watched_directories.clone();
            drop(config_lock);

            if !dirs.is_empty() {
                let mut watchers_lock = watchers.lock().unwrap();
                *watchers_lock = start_watching(dirs);
            }
        }

        Self {
            config,
            watchers,
            add_dir_id,
            manage_dirs_id,
            reload_id,
            autostart_id,
            quit_id,
            _tray_icon: tray_icon,
        }
    }
}

impl ApplicationHandler for TrayApp {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        _event: WindowEvent,
    ) {
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.add_dir_id {
                // Add directory dialog
                if let Ok(Some(path)) = native_dialog::FileDialog::new()
                    .show_open_single_dir()
                {
                    if let Some(path_str) = path.to_str() {
                        let mut config_lock = self.config.lock().unwrap();
                        config_lock.add_directory(path_str.to_string());
                        if let Err(e) = config_lock.save() {
                            error!("Błąd zapisu konfiguracji: {}", e);
                        } else {
                            info!("Zapisano konfigurację");
                        }

                        // Start watching the new directory
                        let dirs = vec![path_str.to_string()];
                        let mut watchers_lock = self.watchers.lock().unwrap();
                        watchers_lock.extend(start_watching(dirs));

                        info!("Dodano katalog do obserwacji: {}", path_str);
                    }
                }
            } else if event.id == self.manage_dirs_id {
                // Show current directories
                let config_lock = self.config.lock().unwrap();
                let dirs = config_lock.watched_directories.clone();
                drop(config_lock);

                let message = if dirs.is_empty() {
                    "Brak skonfigurowanych katalogów".to_string()
                } else {
                    format!("Obserwowane katalogi:\n\n{}", dirs.join("\n"))
                };

                native_dialog::MessageDialog::new()
                    .set_title("Zarządzanie katalogami")
                    .set_text(&message)
                    .show_alert()
                    .ok();
            } else if event.id == self.reload_id {
                // Reload configuration and restart watchers
                info!("Przeładowywanie konfiguracji");
                match Config::load() {
                    Ok(new_config) => {
                        let mut config_lock = self.config.lock().unwrap();
                        *config_lock = new_config;
                        let dirs = config_lock.watched_directories.clone();
                        drop(config_lock);

                        // Restart watchers
                        let mut watchers_lock = self.watchers.lock().unwrap();
                        *watchers_lock = start_watching(dirs);

                        info!("Przeładowano konfigurację pomyślnie");
                    }
                    Err(e) => error!("Błąd ładowania konfiguracji: {}", e),
                }
            } else if event.id == self.autostart_id {
                // Toggle auto-start
                match autostart::toggle() {
                    Ok(enabled) => {
                        let status = if enabled { "włączony" } else { "wyłączony" };
                        info!("Auto-start został {}", status);

                        let message = if enabled {
                            "Aplikacja będzie uruchamiać się automatycznie przy starcie systemu."
                        } else {
                            "Aplikacja nie będzie już uruchamiać się automatycznie przy starcie systemu."
                        };

                        native_dialog::MessageDialog::new()
                            .set_title("Auto-start")
                            .set_text(message)
                            .set_type(native_dialog::MessageType::Info)
                            .show_alert()
                            .ok();
                    }
                    Err(e) => {
                        error!("Błąd podczas zmiany ustawienia auto-startu: {}", e);
                        native_dialog::MessageDialog::new()
                            .set_title("Błąd")
                            .set_text(&format!("Nie można zmienić ustawienia auto-startu:\n{}", e))
                            .set_type(native_dialog::MessageType::Error)
                            .show_alert()
                            .ok();
                    }
                }
            } else if event.id == self.quit_id {
                info!("Zamykanie aplikacji na żądanie użytkownika");
                event_loop.exit();
            }
        }
    }
}
