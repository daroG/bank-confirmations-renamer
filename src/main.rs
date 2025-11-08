#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pdf_processor;
mod config;

use std::path::PathBuf;
use notify::{Watcher, RecursiveMode, RecommendedWatcher, Config as NotifyConfig};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use tray_icon::{TrayIcon, TrayIconBuilder, menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, MenuId}};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, ControlFlow};
use config::Config;

fn check_path(path: &PathBuf) {
    if !path.is_file() { return; }
    if let Some(ext) = path.extension() {
        if ext != "pdf" { return; }
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if filename.starts_with("transfer_") {
                println!("Wykryto plik: {}", path.display());
                let _ = pdf_processor::process_pdf_file(path);
            }
        }
    }
}

fn process_existing_files(dir_path: &str) {
    println!("Sprawdzam istniejące pliki w katalogu: {}", dir_path);
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            check_path(&path);
        }
    }
    println!("Zakończono sprawdzanie istniejących plików w: {}", dir_path);
}

struct WatcherHandle {
    _watcher: RecommendedWatcher,
}

fn start_watching(directories: Vec<String>) -> Vec<WatcherHandle> {
    let mut handles = Vec::new();

    for dir_path in directories {
        let dir = std::path::Path::new(&dir_path);

        // Process existing files first
        process_existing_files(&dir_path);

        // Create watcher
        let (tx, rx): (Sender<notify::Result<notify::Event>>, _) = channel();

        match RecommendedWatcher::new(
            move |res| {
                tx.send(res).ok();
            },
            NotifyConfig::default(),
        ) {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(dir, RecursiveMode::NonRecursive) {
                    eprintln!("Nie można obserwować katalogu {}: {}", dir_path, e);
                    continue;
                }

                let dir_clone = dir_path.clone();
                thread::spawn(move || {
                    println!("Obserwuję katalog: {}", dir_clone);
                    loop {
                        match rx.recv() {
                            Ok(Ok(event)) => {
                                for path in event.paths {
                                    if event.kind.is_create() || event.kind.is_modify() {
                                        check_path(&path);
                                    }
                                }
                            }
                            Ok(Err(e)) => eprintln!("Błąd notify: {}", e),
                            Err(_) => break,
                        }
                    }
                });

                handles.push(WatcherHandle { _watcher: watcher });
            }
            Err(e) => eprintln!("Nie można utworzyć obserwatora dla {}: {}", dir_path, e),
        }
    }

    handles
}

struct TrayApp {
    config: Arc<Mutex<Config>>,
    watchers: Arc<Mutex<Vec<WatcherHandle>>>,
    add_dir_id: MenuId,
    manage_dirs_id: MenuId,
    reload_id: MenuId,
    quit_id: MenuId,
    _tray_icon: Option<TrayIcon>,
}

impl TrayApp {
    fn new() -> Self {
        // Load configuration
        let config = Arc::new(Mutex::new(
            Config::load().unwrap_or_else(|_| Config::default())
        ));

        // Create tray menu
        let tray_menu = Menu::new();

        let add_dir_item = MenuItem::new("Dodaj katalog do obserwacji", true, None);
        let manage_dirs_item = MenuItem::new("Zarządzaj katalogami", true, None);
        let reload_item = MenuItem::new("Przeładuj", true, None);
        let separator = PredefinedMenuItem::separator();
        let quit_item = MenuItem::new("Zakończ", true, None);

        let add_dir_id = add_dir_item.id().clone();
        let manage_dirs_id = manage_dirs_item.id().clone();
        let reload_id = reload_item.id().clone();
        let quit_id = quit_item.id().clone();

        tray_menu.append(&add_dir_item).ok();
        tray_menu.append(&manage_dirs_item).ok();
        tray_menu.append(&reload_item).ok();
        tray_menu.append(&separator).ok();
        tray_menu.append(&quit_item).ok();

        // Create tray icon
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("Invoice Renamer")
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
                            eprintln!("Błąd zapisu konfiguracji: {}", e);
                        }

                        // Start watching the new directory
                        let dirs = vec![path_str.to_string()];
                        let mut watchers_lock = self.watchers.lock().unwrap();
                        watchers_lock.extend(start_watching(dirs));

                        println!("Dodano katalog: {}", path_str);
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
                match Config::load() {
                    Ok(new_config) => {
                        let mut config_lock = self.config.lock().unwrap();
                        *config_lock = new_config;
                        let dirs = config_lock.watched_directories.clone();
                        drop(config_lock);

                        // Restart watchers
                        let mut watchers_lock = self.watchers.lock().unwrap();
                        *watchers_lock = start_watching(dirs);

                        println!("Przeładowano konfigurację");
                    }
                    Err(e) => eprintln!("Błąd ładowania konfiguracji: {}", e),
                }
            } else if event.id == self.quit_id {
                println!("Zamykanie aplikacji...");
                event_loop.exit();
            }
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("Nie można utworzyć event loop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = TrayApp::new();

    event_loop.run_app(&mut app).expect("Błąd event loop");
}
