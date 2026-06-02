use std::path::PathBuf;
use std::fs;
use std::path::Path;
use std::panic;
use log::{warn, info, error};
use crate::rules;

// Właściwa funkcja do parsowania pliku
pub fn process_pdf_file(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {

    // Pre-check: process only files starting with 'transfer_'
    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
        if !filename.starts_with("transfer_") {
            info!("Pomijam plik: {} (nazwa nie zaczyna się od 'transfer_')", filename);
            return Ok(());
        }
    }
    let parent_dir = path.parent().unwrap_or(Path::new("."));

    // 1. Ekstrakcja tekstu - with panic handling for problematic PDFs
    let path_clone = path.clone();
    let text = match panic::catch_unwind(panic::AssertUnwindSafe(|| pdf_extract::extract_text(&path_clone))) {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            error!("Błąd podczas ekstrakcji tekstu z PDF {}: {}", path.display(), e);
            return Err(format!("Nie można wyodrębnić tekstu z PDF: {}", e).into());
        }
        Err(_) => {
            error!("PDF {} ma problemy z kodowaniem - prawdopodobnie brakuje mapy unicode lub encoding. Plik został pominięty.", path.display());
            warn!("Aby przetworzyć ten plik, spróbuj przekonwertować go do nowszej wersji PDF lub wyeksportować tekst ręcznie.");
            return Err("PDF ma problemy z kodowaniem (missing unicode map/encoding)".into());
        }
    };

    // 2. Określenie nowej nazwy na podstawie reguł (sterowanych danymi)
    match rules::determine_new_filename(&text) {
        Some(new_filename) => {
            let new_path = parent_dir.join(&new_filename);

            // Nie nadpisuj istniejącego pliku — `fs::rename` na Windows
            // zastąpiłby go po cichu (utrata danych).
            if new_path.exists() {
                warn!(
                    "Plik docelowy już istnieje, pomijam aby nie nadpisać: {}",
                    new_path.display()
                );
                return Ok(());
            }

            fs::rename(path, &new_path)?;
            info!("Zmieniono nazwę pliku {} na: {}", path.display(), new_path.display());
        }
        None => {
            warn!("Nie znaleziono znanego wzoru w pliku: {}", path.display());
        }
    }

    Ok(())
}
