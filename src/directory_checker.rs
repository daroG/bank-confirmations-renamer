use crate::pdf_processor::process_pdf_file;

use std::fs;

pub fn process_directory(dir_path: &str) -> Result<(), std::io::Error> {
    let entries = fs::read_dir(dir_path)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Upewniamy się, że to plik z rozszerzeniem .pdf
        if path.is_file() && path.extension().map_or(false, |ext| ext == "pdf") {
            // Tutaj wywołamy funkcję do parsowania i zmiany nazwy
            match process_pdf_file(&path) {
                Ok(_) => println!("Pomyślnie przetworzono: {}", path.display()),
                Err(e) => eprintln!("Błąd przetwarzania {}: {}", path.display(), e),
            }
        }
    }
    Ok(())
}