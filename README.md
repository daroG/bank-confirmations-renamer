# Invoice Renamer

Automatyczne narzędzie do monitorowania katalogów i zmiany nazw plików PDF zawierających faktury i dokumenty podatkowe.

## Funkcje

- Automatyczne przetwarzanie plików PDF zaczynających się od "transfer_"
- Rozpoznawanie dokumentów:
  - Formularze podatkowe PIT-5 i VAT-7
  - Potwierdzenia płatności ZUS
- Dwie wersje aplikacji:
  - **GUI** - ikona w zasobniku systemowym Windows
  - **CLI** - aplikacja konsolowa

## Instalacja

### Wymagania

- Rust 1.70 lub nowszy
- Windows (dla wersji GUI z zasobnikiem systemowym)

### Budowanie

```bash
# Sklonuj repozytorium
git clone <repository-url>
cd invoices-renamer

# Zbuduj wersję release (GUI)
cargo build --release

# Zbuduj wersję CLI
cargo build --release --bin invoices-renamer-cli
```

Skompilowane pliki znajdziesz w `target/release/`:
- `invoices-renamer.exe` - wersja GUI
- `invoices-renamer-cli.exe` - wersja CLI

## Użycie

### Wersja GUI (Zasobnik systemowy)

1. Uruchom `invoices-renamer.exe`
2. Ikona pojawi się w zasobniku systemowym Windows
3. Kliknij prawym przyciskiem myszy na ikonę, aby otworzyć menu:
   - **Dodaj katalog do obserwacji** - wybierz katalog do monitorowania
   - **Zarządzaj katalogami** - wyświetl listę obserwowanych katalogów
   - **Przeładuj** - odśwież konfigurację i uruchom ponownie obserwatory
   - **Zakończ** - zamknij aplikację

#### Konfiguracja

Katalogi są zapisywane w pliku konfiguracyjnym:
- Windows: `%APPDATA%\invoices-renamer\config.json`

Możesz ręcznie edytować ten plik, a następnie użyć opcji "Przeładuj" w menu.

#### Logi

Aplikacja zapisuje wszystkie operacje do pliku logu:
- Lokalizacja: `%APPDATA%\invoices-renamer\app.log`
- Zawiera informacje o:
  - Uruchamianiu i zamykaniu aplikacji
  - Dodawaniu/usuwaniu katalogów
  - Wykrywaniu i przetwarzaniu plików
  - Błędach i ostrzeżeniach

Aby przeglądać logi w czasie rzeczywistym (PowerShell):
```powershell
Get-Content $env:APPDATA\invoices-renamer\app.log -Wait -Tail 50
```

### Wersja CLI (Linia poleceń)

```bash
# Uruchom z podaniem ścieżki do katalogu
invoices-renamer-cli.exe "C:\Path\To\Your\Directory"

# Zatrzymaj aplikację używając Ctrl+C
```

## Jak to działa

1. Aplikacja skanuje wskazany katalog w poszukiwaniu plików PDF zaczynających się od "transfer_"
2. Wyodrębnia tekst z pliku PDF
3. Używa wyrażeń regularnych do identyfikacji typu dokumentu:
   - **PIT-5/VAT-7**: `OKR/ YYMmm/SFP/(PIT-5|VAT-7)`
   - **ZUS**: `DANE ODBIORCY Zakład Ubezpieczeń Społecznych` + data operacji
4. Zmienia nazwę pliku według wzoru:
   - PIT-5: `PIT5-MMYY.pdf` (np. "PIT5-0925.pdf")
   - VAT-7: `VAT7-MMYY.pdf` (np. "VAT7-0925.pdf")
   - ZUS: `ZUS-MMYYYY.pdf` (np. "ZUS-092024.pdf")

## Rozwój

### Struktura projektu

- `src/main.rs` - Wersja GUI z zasobnikiem systemowym
- `src/main_cli.rs` - Wersja CLI
- `src/config.rs` - Zarządzanie konfiguracją
- `src/pdf_processor.rs` - Logika przetwarzania PDF
- `src/directory_checker.rs` - Narzędzie do przetwarzania wsadowego

### Testowanie

```bash
# Uruchom testy
cargo test

# Sprawdź kod
cargo check

# Uruchom clippy
cargo clippy
```

### Więcej informacji

Zobacz [CLAUDE.md](CLAUDE.md) dla szczegółowych informacji o architekturze i rozwoju.

## Licencja

[Dodaj informacje o licencji]
