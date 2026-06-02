use regex::Regex;
use std::path::PathBuf;
use std::fs;
use std::path::Path;
use std::panic;
use log::{warn, info, error};

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
            return Err(format!("PDF ma problemy z kodowaniem (missing unicode map/encoding)").into());
        }
    };

    // 2. Definicja wzoru dla OKR/ 25M09/SFP/PIT-5 lub VAT-7, z osobnymi grupami dla roku i miesiąca
    // Przykład: OKR/ 25M09/SFP/PIT-5
    let taxes_re = Regex::new(r"OKR/\s*(\d{2})M(\d{2})/SFP/(PIT-5|VAT-7)")?;

    // 3. Wyszukiwanie wzoru
    if let Some(captures) = taxes_re.captures(&text) {
        let year = captures.get(1).map_or("", |m| m.as_str()); // np. 25
        let month = captures.get(2).map_or("", |m| m.as_str()); // np. 09
        let form_type = captures.get(3).map_or("", |m| m.as_str()); // np. PIT-5 lub VAT-7

        if year.is_empty() || month.is_empty() || form_type.is_empty() {
            warn!("Nie znaleziono wszystkich wymaganych informacji w pliku: {}", path.display());
            return Ok(());
        }

        let form_type_clean = form_type.replace("-", ""); // PIT5 lub VAT7
        let new_filename = format!("{}-{}{}.pdf", form_type_clean, month, year);

        let new_path = parent_dir.join(new_filename);

        let new_path_display = new_path.clone();
        fs::rename(path, new_path)?;
        info!("Zmieniono nazwę pliku {} na: {}", path.display(), new_path_display.display());
        return Ok(());
    }

    let zus_re = Regex::new(r"DANE ODBIORCY\s*Zakład Ubezpieczeń Społecznych.*DATA OPERACJI\s*(\d{2})-(\d{2})-(\d{4})")?;
    if let Some(captures) = zus_re.captures(&text) {
        let day = captures.get(1).map_or("", |m| m.as_str()); // np. 25
        let month = captures.get(2).map_or("", |m| m.as_str()); // np. 09
        let year = captures.get(2).map_or("", |m| m.as_str()); // np. 2025

        if year.is_empty() || month.is_empty() || day.is_empty() {
            warn!("Nie znaleziono wszystkich wymaganych informacji w pliku: {}", path.display());
            return Ok(());
        }

        let month_int: i16 = month.parse::<i16>().unwrap_or(0);
        let year_int: i16 = year.parse::<i16>().unwrap_or(0);

        let previous_month: i16 = if month_int == 1 { 12 } else { month_int - 1 };
        let previous_year: i16 = if month_int == 1 { year_int - 1 } else { year_int };

        let new_filename = format!("ZUS-{:02}{}.pdf", previous_month, previous_year);
        let new_path = parent_dir.join(new_filename);

        let new_path_display = new_path.clone();
        fs::rename(path, new_path)?;
        info!("Zmieniono nazwę pliku {} na: {}", path.display(), new_path_display.display());
    }
    Ok(())
}