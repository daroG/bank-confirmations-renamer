# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Rust-based PDF file monitoring and renaming tool that watches a directory for PDF files starting with "transfer_" and automatically renames them based on their content. It extracts text from PDFs and uses regex patterns to identify tax forms (PIT-5, VAT-7) and ZUS (Polish social insurance) payment confirmations, then renames files to a standardized format.

## Common Commands

### Build and Run
```bash
# Build the project
cargo build

# Build release version
cargo build --release

# Run the program (requires directory path argument)
cargo run -- <path_to_directory>

# Run release version
cargo run --release -- <path_to_directory>
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

1. **main.rs** - Entry point with file system watcher
   - Sets up directory monitoring using `notify` crate
   - Filters for PDF files starting with "transfer_"
   - Handles Ctrl+C gracefully with atomic boolean flag
   - Delegates PDF processing to pdf_processor module

2. **pdf_processor.rs** - PDF text extraction and renaming logic
   - Extracts text from PDF files using `pdf-extract` crate
   - Uses regex patterns to identify document types:
     - **Tax forms**: Pattern `OKR/ YYMmm/SFP/(PIT-5|VAT-7)` extracts year, month, and form type
       - Renames to: `{FORM}{MM}{YY}.pdf` (e.g., "PIT5-0925.pdf")
     - **ZUS payments**: Pattern matching "DANE ODBIORCY Zakład Ubezpieczeń Społecznych" with date extraction
       - Renames to: `ZUS-{MM}{YYYY}.pdf` using previous month from payment date
   - Returns `Result<(), Box<dyn std::error::Error>>` for error handling

3. **directory_checker.rs** - Batch directory processing utility
   - Processes all PDF files in a directory at once (not actively used in main flow)
   - Useful for batch operations on existing files

### Key Dependencies

- **regex**: Pattern matching for document identification
- **pdf-extract**: PDF text extraction
- **notify**: File system event monitoring
- **ctrlc**: Graceful shutdown handling

### Program Flow

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
- The program monitors only the specified directory (non-recursive)
- Edition is set to "2024" in Cargo.toml (should likely be "2021")
