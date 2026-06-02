use regex::Regex;
use std::path::PathBuf;
use std::fs;
use std::path::Path;
use std::panic;
use log::{warn, info, error};

/// Określa znormalizowaną nazwę pliku na podstawie tekstu z PDF.
///
/// Zwraca `Ok(Some(nazwa))`, gdy tekst pasuje do znanego wzoru dokumentu
/// (formularz podatkowy PIT-5/VAT-7 lub potwierdzenie przelewu ZUS), albo
/// `Ok(None)`, gdy nie pasuje do żadnego wzoru. Jest to czysta (bez operacji
/// na plikach), testowalna część logiki zmiany nazwy.
pub fn determine_new_filename(text: &str) -> Result<Option<String>, Box<dyn std::error::Error>> {
    // Wzór dla OKR/ 25M09/SFP/PIT-5 lub VAT-7, z osobnymi grupami dla roku i miesiąca
    // Przykład: OKR/ 25M09/SFP/PIT-5
    let taxes_re = Regex::new(r"OKR/\s*(\d{2})M(\d{2})/SFP/(PIT-5|VAT-7)")?;
    if let Some(captures) = taxes_re.captures(text) {
        let year = &captures[1];      // np. 25
        let month = &captures[2];     // np. 09
        let form_type = &captures[3]; // np. PIT-5 lub VAT-7

        let form_type_clean = form_type.replace("-", ""); // PIT5 lub VAT7
        return Ok(Some(format!("{}-{}{}.pdf", form_type_clean, month, year)));
    }

    // Wzór dla potwierdzenia przelewu ZUS.
    // Flaga (?s) sprawia, że `.` obejmuje znaki nowej linii — pola w tekście
    // wyekstrahowanym z PDF są zwykle na osobnych liniach. `.*?` jest leniwe,
    // by dopasować najbliższą datę operacji.
    let zus_re = Regex::new(r"(?s)DANE ODBIORCY\s*Zakład Ubezpieczeń Społecznych.*?DATA OPERACJI\s*(\d{2})-(\d{2})-(\d{4})")?;
    if let Some(captures) = zus_re.captures(text) {
        // grupa 1 = dzień (nieużywany), grupa 2 = miesiąc, grupa 3 = rok
        let month: i16 = captures[2].parse().unwrap_or(0); // np. 09
        let year: i16 = captures[3].parse().unwrap_or(0);  // np. 2025

        // Składkę ZUS opłaca się za poprzedni miesiąc
        let previous_month = if month == 1 { 12 } else { month - 1 };
        let previous_year = if month == 1 { year - 1 } else { year };

        return Ok(Some(format!("ZUS-{:02}{}.pdf", previous_month, previous_year)));
    }

    Ok(None)
}

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

    // 2. Określenie nowej nazwy na podstawie zawartości i zmiana nazwy pliku
    #[allow(non_snake_case)]
    match determine_new_filename(&text)? {
        Some(new_filename) => {
            let new_path = parent_dir.join(&new_filename);
            fs::rename(path, &new_path)?;
            info!("Zmieniono nazwę pliku {} na: {}", path.display(), new_path.display());
        }
        None => {
            warn!("Nie znaleziono znanego wzoru w pliku: {}", path.display());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_pit5_tax_form() {
        let text = "blah blah OKR/ 25M09/SFP/PIT-5 blah";
        assert_eq!(
            determine_new_filename(text).unwrap(),
            Some("PIT5-0925.pdf".to_string())
        );
    }

    #[test]
    fn matches_vat7_tax_form() {
        let text = "OKR/25M09/SFP/VAT-7";
        assert_eq!(
            determine_new_filename(text).unwrap(),
            Some("VAT7-0925.pdf".to_string())
        );
    }

    #[test]
    fn zus_uses_previous_month_and_correct_year() {
        // Regresja: wcześniej `year` czytał grupę miesiąca zamiast roku,
        // przez co w nazwie pliku rok był zastępowany numerem miesiąca.
        let text = "DANE ODBIORCY Zakład Ubezpieczeń Społecznych foo DATA OPERACJI 15-09-2025";
        assert_eq!(
            determine_new_filename(text).unwrap(),
            Some("ZUS-082025.pdf".to_string())
        );
    }

    #[test]
    fn zus_matches_when_fields_span_multiple_lines() {
        // Realistyczny tekst z PDF: pola są na osobnych liniach. Bez flagi (?s)
        // `.` nie obejmuje znaku nowej linii, więc wzór ZUS nigdy by nie pasował.
        let text = "DANE ODBIORCY\nZakład Ubezpieczeń Społecznych\nul. Szamocka 3, 5\n01-748 Warszawa\nTYTUŁ\nskładka\nDATA OPERACJI 15-09-2025";
        assert_eq!(
            determine_new_filename(text).unwrap(),
            Some("ZUS-082025.pdf".to_string())
        );
    }

    #[test]
    fn zus_january_rolls_back_to_previous_december() {
        let text = "DANE ODBIORCY Zakład Ubezpieczeń Społecznych foo DATA OPERACJI 10-01-2025";
        assert_eq!(
            determine_new_filename(text).unwrap(),
            Some("ZUS-122024.pdf".to_string())
        );
    }

    #[test]
    fn returns_none_when_no_pattern_matches() {
        assert_eq!(determine_new_filename("jakiś losowy tekst").unwrap(), None);
    }
}
