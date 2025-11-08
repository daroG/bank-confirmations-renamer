#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod pdf_processor;
mod config;

use std::path::PathBuf;
use notify::{Watcher, RecursiveMode, RecommendedWatcher, Config as NotifyConfig};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use tray_icon::{TrayIconBuilder, menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem}};
use winit::event_loop::{EventLoop, ControlFlow};
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

fn main() {
    // Load configuration
    let config = Arc::new(Mutex::new(
        Config::load().unwrap_or_else(|_| Config::default())
    ));

    // Create event loop
    let event_loop = EventLoop::new().expect("Nie można utworzyć event loop");

    // Create tray menu
    let tray_menu = Menu::new();

    let add_dir_item = MenuItem::new("Dodaj katalog do obserwacji", true, None);
    let manage_dirs_item = MenuItem::new("Zarządzaj katalogami", true, None);
    let reload_item = MenuItem::new("Przeładuj", true, None);
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("Zakończ", true, None);

    tray_menu.append(&add_dir_item).ok();
    tray_menu.append(&manage_dirs_item).ok();
    tray_menu.append(&reload_item).ok();
    tray_menu.append(&separator).ok();
    tray_menu.append(&quit_item).ok();

    // Create tray icon
    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Invoice Renamer")
        .build()
        .expect("Nie można utworzyć ikony w zasobniku");

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

    let menu_channel = MenuEvent::receiver();
    let config_clone = config.clone();
    let watchers_clone = watchers.clone();

    event_loop.run(move |_event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == add_dir_item.id() {
                // Add directory dialog
                if let Ok(Some(path)) = native_dialog::FileDialog::new()
                    .show_open_single_dir()
                {
                    if let Some(path_str) = path.to_str() {
                        let mut config_lock = config_clone.lock().unwrap();
                        config_lock.add_directory(path_str.to_string());
                        if let Err(e) = config_lock.save() {
                            eprintln!("Błąd zapisu konfiguracji: {}", e);
                        }

                        // Start watching the new directory
                        let dirs = vec![path_str.to_string()];
                        let mut watchers_lock = watchers_clone.lock().unwrap();
                        watchers_lock.extend(start_watching(dirs));

                        println!("Dodano katalog: {}", path_str);
                    }
                }
            } else if event.id == manage_dirs_item.id() {
                // Show current directories
                let config_lock = config_clone.lock().unwrap();
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
            } else if event.id == reload_item.id() {
                // Reload configuration and restart watchers
                match Config::load() {
                    Ok(new_config) => {
                        let mut config_lock = config_clone.lock().unwrap();
                        *config_lock = new_config;
                        let dirs = config_lock.watched_directories.clone();
                        drop(config_lock);

                        // Restart watchers
                        let mut watchers_lock = watchers_clone.lock().unwrap();
                        *watchers_lock = start_watching(dirs);

                        println!("Przeładowano konfigurację");
                    }
                    Err(e) => eprintln!("Błąd ładowania konfiguracji: {}", e),
                }
            } else if event.id == quit_item.id() {
                println!("Zamykanie aplikacji...");
                elwt.exit();
            }
        }
    }).expect("Błąd event loop");
}
