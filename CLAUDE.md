# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based PDF file monitoring and renaming tool that watches directories for PDF files starting with "transfer_" and automatically renames them based on their content. It extracts text from PDFs and matches it against a set of data-driven rules (regex + filename template, see `rules.rs`) to identify documents such as tax forms (PIT-5, VAT-7) and ZUS (Polish social insurance) payment confirmations, then renames files to a standardized format. Rules live in `rules.json` and can be edited without recompiling.

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

### Code Organization

The codebase is organized into focused modules with clear responsibilities:

**Entry Points:**
- `main.rs` - GUI tray application (minimal, delegates to modules)
- `main_cli.rs` - CLI application

**Core Logic:**
- `tray_app.rs` - Tray icon and menu handling
- `file_watcher.rs` - Directory monitoring and file detection
- `pdf_processor.rs` - PDF text extraction and file renaming (delegates naming to `rules.rs`)
- `rules.rs` - Data-driven rename rules engine (regex + filename templates, loaded from `rules.json`)

**Supporting Modules:**
- `config.rs` - Configuration persistence
- `logger.rs` - Logging setup
- `autostart.rs` - Windows auto-start management
- `icon_generator.rs` - Tray icon generation
- `directory_checker.rs` - Batch processing utility (legacy)

This separation ensures:
- Each module has a single, clear purpose
- main.rs is minimal (~25 lines)
- Easy to test individual components
- Clear dependency flow

### Core Components

1. **main.rs** - Application entry point
   - Initializes logging system
   - Creates winit event loop
   - Launches tray application
   - Clean and minimal (~25 lines)

2. **tray_app.rs** - Windows tray GUI application logic
   - TrayApp struct implementing ApplicationHandler
   - Creates system tray icon with menu
   - Manages multiple directory watchers
   - Handles menu events:
     - Add directory to watch
     - Manage directories (view current list)
     - Reload configuration
     - Quit application
   - Integrates with config and file_watcher modules

3. **file_watcher.rs** - Directory monitoring logic
   - WatcherHandle struct wrapping RecommendedWatcher
   - process_existing_files() - scans directory for existing PDFs
   - start_watching() - creates watchers for multiple directories
   - check_path() - filters and processes PDF files
   - Spawns separate thread for each watched directory

4. **logger.rs** - Logging initialization
   - init_logger() - sets up file-based logging
   - Configures env_logger with custom format
   - Creates log file in user config directory
   - Returns log file path for informational purposes

5. **main_cli.rs** - CLI version entry point
   - Sets up single directory monitoring using `notify` crate
   - Filters for PDF files starting with "transfer_"
   - Handles Ctrl+C gracefully with atomic boolean flag
   - Delegates PDF processing to pdf_processor module

6. **config.rs** - Configuration management
   - Serializes/deserializes watched directories list
   - Stores config in `%APPDATA%/invoices-renamer/config.json` (Windows)
   - Provides methods to add/remove directories
   - Auto-creates config directory if needed

7. **autostart.rs** - Windows auto-start management
   - setup_autostart() - creates AutoLaunch configuration
   - is_enabled() - checks if auto-start is enabled
   - enable()/disable() - manages Windows registry entry
   - toggle() - toggles auto-start state
   - Platform-specific: Windows only (stubs for other platforms)
   - Uses `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`

8. **icon_generator.rs** - Tray icon generation
   - Generates a 32x32 PNG icon programmatically
   - Creates a simple document/PDF icon representation
   - Used for the system tray icon

9. **pdf_processor.rs** - PDF text extraction and renaming orchestration
   - Extracts text from PDF files using `pdf-extract` crate (with panic recovery)
   - Delegates the new filename decision to `rules::determine_new_filename()`
   - Renames the file, with a **no-overwrite guard**: if the target name already
     exists, the file is skipped (a log entry is written) rather than letting
     `fs::rename` silently clobber the existing file
   - Returns `Result<(), Box<dyn std::error::Error>>` for error handling

10. **rules.rs** - Data-driven rename rules engine
   - Rename rules live in `%APPDATA%\invoices-renamer\rules.json`, not in code.
     Adding a new document type is a JSON edit — no recompile.
   - `Rule { name, pattern, template }` and `RulesConfig` are (de)serialized via serde.
     Defaults are written on first run and reproduce the original PIT-5/VAT-7 and ZUS behavior.
   - Default rules:
     - **Tax forms**: `OKR/\s*(?P<year>\d{2})M(?P<month>\d{2})/SFP/(?P<form>PIT-5|VAT-7)`
       → template `{nodash(form)}-{month}{year}.pdf` (e.g., "PIT5-0925.pdf")
     - **ZUS payments**: `(?s)DANE ODBIORCY ... Zakład Ubezpieczeń Społecznych ... DATA OPERACJI (?P<day>..)-(?P<month>..)-(?P<year>....)`
       → template `ZUS-{prevmonthyear(month, year)}.pdf` (previous month, e.g., "ZUS-082025.pdf").
       Note the `(?s)` flag so `.` matches newlines (extracted PDF text spans multiple lines).
   - **Template engine** (recursive-descent parser in `rules.rs`): text outside `{...}` is
     literal; inside, an expression is `{group}` (a named capture), a string literal `"..."`,
     or a function call `{func(arg, ...)}` whose args are themselves expressions — so calls
     **nest** (e.g. `{upper(nodash(form))}`). Literal braces are escaped by doubling
     (`{{` → `{`, `}}` → `}`). Helpers: `nodash`, `upper`, `lower`, `pad(x, "n")`,
     `prevmonthyear`. New transforms go in `apply_helper()` (also update `helper_arity()`).
   - **Robustness**: at compile time each rule is validated — template syntax, that every
     referenced capture exists in the regex, and that helper names/arity are valid; an invalid
     rule is logged and skipped (one typo can't disable the app, and misuse fails loudly rather
     than silently at render). A corrupt `rules.json` falls back to defaults; active rules are
     compiled once via `OnceLock`. (Caveat: editing `rules.json` requires an app restart —
     "Przeładuj" does not yet re-read it.)

11. **directory_checker.rs** - Batch directory processing utility
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
- **log/env_logger**: Logging framework with file output
- **chrono**: Timestamps for log entries
- **image**: Icon generation and loading

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

## Error Handling

The application handles PDF processing errors gracefully:
- **Panic recovery**: Catches panics from pdf-extract library (common with malformed PDFs)
- **Unicode/encoding errors**: PDFs with missing unicode maps are logged and skipped
- **File operations**: All file rename operations are logged with success/failure status
- **Non-blocking**: Errors in one file don't stop processing of other files

Common PDF issues:
- **Missing unicode map/encoding**: Some older or malformed PDFs lack proper text encoding
- **Solution**: Re-save PDF in newer format, use PDF converter, or print-to-PDF

## Logging

The GUI application logs all activity to a file for debugging and monitoring:
- **Log file location**: `%APPDATA%\invoices-renamer\app.log` (Windows)
- **Log level**: INFO (can be changed via `RUST_LOG` environment variable)
- **Logged events**:
  - Application startup/shutdown
  - Configuration loading/saving
  - Directory scanning and watching
  - File detection and processing
  - PDF parsing and renaming operations
  - Errors and warnings

To view logs in real-time (for debugging):
```bash
# Windows PowerShell
Get-Content $env:APPDATA\invoices-renamer\app.log -Wait -Tail 50
```

## Important Notes

- The code contains Polish language comments and console output
- Files must start with "transfer_" prefix to be processed
- Each directory is monitored non-recursively (subdirectories are not watched)
- Configuration file location: `%APPDATA%\invoices-renamer\config.json` on Windows (watched directories)
- Rename rules file location: `%APPDATA%\invoices-renamer\rules.json` on Windows (regex + templates; auto-created with defaults; restart required after editing)
- Log file location: `%APPDATA%\invoices-renamer\app.log` on Windows
- The GUI version runs without a console window in release mode (uses `windows_subsystem = "windows"`)
- The application uses separate threads for each directory watcher
- Two binary targets are built:
  - `invoices-renamer` - GUI tray application (with custom icon)
  - `invoices-renamer-cli` - CLI version
