# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based PDF file monitoring and renaming tool that watches directories for PDF files starting with "transfer_" and automatically renames them based on their content. It extracts text from PDFs and uses regex patterns to identify tax forms (PIT-5, VAT-7) and ZUS (Polish social insurance) payment confirmations, then renames files to a standardized format.

The application comes in two versions:
- **GUI Tray Application** (default) - Runs in the Windows system tray with a menu for managing watched directories
- **CLI Application** - Command-line version that watches a single directory

## Common Commands

### Build and Run

#### GUI Tray Application (Default)
```bash
# Build the tray application
cargo build

# Build release version (runs without console window)
cargo build --release

# Run the tray application (debug mode, with console)
cargo run

# Run release version (no console window)
cargo run --release
```

#### CLI Application
```bash
# Build CLI version
cargo build --bin invoices-renamer-cli

# Run CLI version (requires directory path argument)
cargo run --bin invoices-renamer-cli -- <path_to_directory>

# Run CLI release version
cargo run --release --bin invoices-renamer-cli -- <path_to_directory>
```

### Testing
```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run a specific test
cargo test <test_name>
```

### Other
```bash
# Check code without building
cargo check

# Format code
cargo fmt

# Run clippy linter
cargo clippy
```

## Architecture

### Core Components

1. **main.rs** - Windows tray GUI application
   - Creates system tray icon with menu
   - Manages multiple directory watchers
   - Provides menu options:
     - Add directory to watch
     - Manage directories (view current list)
     - Reload configuration
     - Quit application
   - Stores configuration in user config directory
   - Runs without console window in release mode

2. **main_cli.rs** - CLI version entry point
   - Sets up single directory monitoring using `notify` crate
   - Filters for PDF files starting with "transfer_"
   - Handles Ctrl+C gracefully with atomic boolean flag
   - Delegates PDF processing to pdf_processor module

3. **config.rs** - Configuration management
   - Serializes/deserializes watched directories list
   - Stores config in `%APPDATA%/invoices-renamer/config.json` (Windows)
   - Provides methods to add/remove directories
   - Auto-creates config directory if needed

4. **pdf_processor.rs** - PDF text extraction and renaming logic
   - Extracts text from PDF files using `pdf-extract` crate
   - Uses regex patterns to identify document types:
     - **Tax forms**: Pattern `OKR/ YYMmm/SFP/(PIT-5|VAT-7)` extracts year, month, and form type
       - Renames to: `{FORM}{MM}{YY}.pdf` (e.g., "PIT5-0925.pdf")
     - **ZUS payments**: Pattern matching "DANE ODBIORCY Zakład Ubezpieczeń Społecznych" with date extraction
       - Renames to: `ZUS-{MM}{YYYY}.pdf` using previous month from payment date
   - Returns `Result<(), Box<dyn std::error::Error>>` for error handling

5. **directory_checker.rs** - Batch directory processing utility
   - Processes all PDF files in a directory at once
   - Useful for batch operations on existing files
   - Not actively used in current application flow

### Key Dependencies

- **regex**: Pattern matching for document identification
- **pdf-extract**: PDF text extraction
- **notify**: File system event monitoring
- **ctrlc**: Graceful shutdown handling (CLI version)
- **tray-icon**: Windows system tray icon and menu
- **winit**: Event loop for GUI application
- **serde/serde_json**: Configuration serialization
- **dirs**: Platform-specific directory paths
- **native-dialog**: File picker and message dialogs

### Program Flow

#### GUI Tray Application
1. Application starts and loads configuration from `config.json`
2. Creates system tray icon with menu
3. For each configured directory:
   - Scans all existing PDF files starting with "transfer_"
   - Starts file system watcher for new files
4. User can interact via tray menu:
   - Add new directories (opens folder picker dialog)
   - View managed directories (shows message dialog)
   - Reload configuration (restarts all watchers)
   - Quit application
5. When files are detected (existing or new):
   - Extracts PDF text and matches against known patterns
   - Renames file to standardized format
6. Runs until user selects "Quit" from tray menu

#### CLI Application
1. User provides directory path as command-line argument
2. Program scans all existing files in the directory on startup
3. Processes any existing PDFs starting with "transfer_"
4. Starts file system watcher to monitor directory for new file creation/modification events
5. On event, checks if file is PDF and starts with "transfer_"
6. Extracts PDF text and attempts to match against known patterns
7. If match found, renames file to standardized format
8. Continues monitoring until Ctrl+C

## Important Notes

- The code contains Polish language comments and console output
- Files must start with "transfer_" prefix to be processed
- Each directory is monitored non-recursively (subdirectories are not watched)
- Configuration file location: `%APPDATA%\invoices-renamer\config.json` on Windows
- The GUI version runs without a console window in release mode (uses `windows_subsystem = "windows"`)
- The application uses separate threads for each directory watcher
- Two binary targets are built:
  - `invoices-renamer` - GUI tray application
  - `invoices-renamer-cli` - CLI version
