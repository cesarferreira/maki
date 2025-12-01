# maki

A cross-platform fuzzy Makefile task finder and runner.

![Rust](https://img.shields.io/badge/rust-stable-orange.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)

## Features

- **Fuzzy search** - Quickly find and run Makefile targets using an interactive fuzzy finder
- **Cross-platform** - Works on Linux, macOS, and Windows
- **Smart caching** - Caches parsed Makefiles using SHA256 checksums for instant subsequent lookups
- **Comment extraction** - Automatically extracts target descriptions from comments
- **JSON output** - Machine-readable output for scripting and integrations
- **Recursive scanning** - Find Makefiles in subdirectories

## Installation

### From source

```bash
cargo install --path .
```

### Build from source

```bash
git clone https://github.com/cesarferreira/maki
cd maki
cargo build --release
# Binary will be at ./target/release/maki
```

## Usage

### Interactive Mode (default)

Simply run `maki` in a directory with a Makefile to start the interactive fuzzy finder:

```bash
maki
```

Use the arrow keys to navigate, type to filter, and press Enter to run the selected target.

### Commands

```bash
# Interactive fuzzy search (default)
maki

# List all targets
maki list

# Run a specific target directly
maki run build

# Interactive picker (explicit)
maki pick
```

### Options

| Flag | Description |
|------|-------------|
| `-f, --file <FILE>` | Use a custom Makefile |
| `--all` | Include private targets (starting with `_`) |
| `--patterns` | Include pattern rules (e.g., `%.o: %.c`) |
| `--json` | Output results as JSON |
| `--no-ui` | Skip the fuzzy finder UI |
| `-r, --recursive` | Scan subdirectories for Makefiles |
| `--dry-run` | Print command without executing |
| `--cwd <DIR>` | Set the working directory |
| `--no-cache` | Skip the cache and re-parse Makefiles |

### Examples

```bash
# List all targets in JSON format
maki list --json

# Include private targets (those starting with _)
maki list --all

# Run a target without actually executing it
maki run deploy --dry-run

# Use a custom Makefile
maki -f build/Makefile list

# Scan all subdirectories for Makefiles
maki -r list

# Force re-parsing (skip cache)
maki --no-cache list
```

## Caching

Maki caches parsed Makefiles to improve performance. The cache:

- Uses SHA256 checksums to detect file changes
- Is stored in your system's cache directory:
  - **macOS**: `~/Library/Caches/maki/`
  - **Linux**: `~/.cache/maki/`
  - **Windows**: `%LOCALAPPDATA%\maki\`
- Is automatically invalidated when the Makefile content changes
- Can be bypassed with `--no-cache`

## Target Detection

Maki detects targets using the pattern:

```
target_name: [dependencies]
```

### Comment Extraction

Maki extracts descriptions from:

1. **Inline comments** using `##`:
   ```makefile
   build: ## Build the project
   	cargo build
   ```

2. **Preceding comments**:
   ```makefile
   # Build the project with optimizations
   build:
   	cargo build --release
   ```

### Skipped Lines

Maki automatically skips:

- Variable assignments (`VAR := value`, `VAR ?= value`, `VAR += value`)
- Target-specific variables (`target: VAR := value`)
- Pattern rules (unless `--patterns` is used)
- Private targets starting with `_` (unless `--all` is used)
- Comment lines
- Blank lines

## JSON Output

The `--json` flag outputs targets in this format:

```json
[
  {
    "name": "build",
    "description": "Build the project",
    "file": "/path/to/Makefile",
    "line": 42
  }
]
```

## Development

### Building

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Running with Debug Output

```bash
cargo run -- list
```

## Project Structure

```
src/
├── main.rs       # Application entry point
├── cli.rs        # CLI argument parsing (clap)
├── target.rs     # Target struct definition
├── makefile.rs   # Makefile parsing logic
├── fuzzy.rs      # Fuzzy finder UI (skim)
├── executor.rs   # Task execution
└── cache.rs      # SHA-based caching
```

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgements

- [skim](https://github.com/lotabout/skim) - Fuzzy finder library
- [clap](https://github.com/clap-rs/clap) - Command line argument parser
