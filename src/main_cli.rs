mod pdf_processor;

use std::env;
use std::path::{Path, PathBuf};
use notify::{Watcher, RecursiveMode, RecommendedWatcher, Config};
use std::sync::mpsc::channel;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;


fn check_path(path: &PathBuf) {
    if !path.is_file() { return; }
    if let Some(ext) = path.extension() {
        if ext != "pdf" { return; }
        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if filename.starts_with("transfer_") {
                println!("Wykryto plik: {}", path.display());
                // Przetwórz plik
                let _ = pdf_processor::process_pdf_file(&PathBuf::from(&path));
            }
        }
        
    }
}

fn main() {
    // Flaga do zakończenia programu
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        println!("\nZamykam program na żądanie użytkownika (Ctrl+C)");
    }).expect("Nie można ustawić handlera Ctrl+C");
    // Pobierz argumenty z linii poleceń
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Użycie: {} <ścieżka_do_katalogu>", args[0]);
        std::process::exit(1);
    }
    let dir_path = &args[1];
    let dir = Path::new(dir_path);

    // Najpierw przetwórz wszystkie istniejące pliki w katalogu
    println!("Sprawdzam istniejące pliki w katalogu: {}", dir_path);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            check_path(&path);
        }
    }
    println!("Zakończono sprawdzanie istniejących plików.");
    println!("Obserwuję katalog: {}", dir_path);

    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(
        move |res| {
            tx.send(res).unwrap();
        },
            Config::default(),
    ).expect("Nie można utworzyć obserwatora");
        watcher.watch(dir, RecursiveMode::NonRecursive).expect("Nie można obserwować katalogu");

    while running.load(Ordering::SeqCst) {
        match rx.recv() {
            Ok(Ok(event)) => {
                for path in event.paths {
                    if event.kind.is_create() || event.kind.is_modify() {
                        check_path(&path);
                    }
                }
            }
            Ok(Err(e)) => eprintln!("Błąd notify: {}", e),
            Err(e) => eprintln!("Błąd odbioru: {}", e),
        }
    }
}
