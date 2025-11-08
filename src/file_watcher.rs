use log::{info, error, debug};
use std::path::PathBuf;
use notify::{Watcher, RecursiveMode, RecommendedWatcher, Config as NotifyConfig};
use std::sync::mpsc::{channel, Sender};
use std::thread;
use crate::pdf_processor;

pub struct WatcherHandle {
    _watcher: RecommendedWatcher,
}

fn check_path(path: &PathBuf) {
    if !path.is_file() { return; }
    if let Some(ext) = path.extension() {
        if ext != "pdf" { return; }
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if filename.starts_with("transfer_") {
                info!("Wykryto plik do przetworzenia: {}", path.display());
                match pdf_processor::process_pdf_file(path) {
                    Ok(_) => info!("Pomyślnie przetworzono plik: {}", path.display()),
                    Err(e) => error!("Błąd podczas przetwarzania pliku {}: {}", path.display(), e),
                }
            }
        }
    }
}

pub fn process_existing_files(dir_path: &str) {
    info!("Sprawdzam istniejące pliki w katalogu: {}", dir_path);
    if let Ok(entries) = std::fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            check_path(&path);
        }
    }
    info!("Zakończono sprawdzanie istniejących plików w: {}", dir_path);
}

pub fn start_watching(directories: Vec<String>) -> Vec<WatcherHandle> {
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
                    error!("Nie można obserwować katalogu {}: {}", dir_path, e);
                    continue;
                }

                let dir_clone = dir_path.clone();
                thread::spawn(move || {
                    info!("Rozpoczęto obserwację katalogu: {}", dir_clone);
                    loop {
                        match rx.recv() {
                            Ok(Ok(event)) => {
                                for path in event.paths {
                                    if event.kind.is_create() || event.kind.is_modify() {
                                        debug!("Wykryto zmianę w pliku: {}", path.display());
                                        check_path(&path);
                                    }
                                }
                            }
                            Ok(Err(e)) => error!("Błąd notify: {}", e),
                            Err(_) => {
                                info!("Zakończono obserwację katalogu: {}", dir_clone);
                                break;
                            }
                        }
                    }
                });

                handles.push(WatcherHandle { _watcher: watcher });
            }
            Err(e) => error!("Nie można utworzyć obserwatora dla {}: {}", dir_path, e),
        }
    }

    handles
}
