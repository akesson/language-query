# Language Query

A fast, language-agnostic CLI tool for querying code intelligence from Language Server Protocol (LSP) servers.
Built on the proven architecture of mcp-rust-analyzer, this tool provides instant access to code navigation,
documentation, and analysis features directly from your terminal.

## Overview

Language Query automatically manages LSP server daemons and provides a simple command-line interface for common
code intelligence operations. Instead of requiring a full IDE or editor integration, you can query your codebase
directly from the terminal.

The tool uses a daemon architecture where:
- The daemon process manages the LSP server lifecycle
- The CLI communicates with the daemon via IPC (Unix domain sockets)
- Multiple CLI invocations reuse the same daemon for performance
- Daemons are workspace-specific and auto-terminate when idle

## Features

### Core Commands

- **`lq docs <file>:<line> <symbol>`** - Get documentation/hover information for a symbol at a specific position
- **`lq impl <file>:<line> <symbol>`** - Show the implementation location or full source code of a symbol
- **`lq refs <file>:<line> <symbol>`** - Find all references to a symbol with usage previews
- **`lq resolve <symbol> <file>`** - Search for a symbol by name in a file context (fuzzy matching)

### Daemon Management

- **`lq status`** - Check daemon status and indexing progress
- **`lq stop`** - Stop the daemon for current workspace
- **`lq logs`** - View daemon logs

## Usage Examples

```bash
# Get documentation for a function at line 42
lq docs src/main.rs:42 process_data

# Find all references to a struct at line 15
lq refs src/types.rs:15 Config

# Search for symbols by name (fuzzy matching)
lq resolve Parser src/main.rs

# Show implementation of a method at line 120
lq impl src/parser.rs:120 parse_expression
```

## Architecture

### CLI Client (`lq`)
- Lightweight command parser
- IPC client for daemon communication
- JSON-based request/response protocol
- Automatic daemon startup if not running

### Daemon Process
- One daemon per workspace root
- Manages LSP server lifecycle
- Handles file watching and change notifications
- Maintains document state and synchronization
- Provides request queuing and deduplication

### LSP Integration
- Language-agnostic design supporting any LSP server
- Automatic server discovery based on file types
- Configurable server commands and initialization options
- Support for workspace and document capabilities

## Configuration

Configuration is stored in `~/.config/language-query/config.toml`:

```toml
[servers.rust]
command = "rust-analyzer"
file_patterns = ["*.rs", "Cargo.toml"]

[servers.typescript]
command = "typescript-language-server"
args = ["--stdio"]
file_patterns = ["*.ts", "*.tsx", "*.js", "*.jsx"]

[servers.python]
command = "pylsp"
file_patterns = ["*.py"]

[daemon]
idle_timeout_minutes = 30
socket_dir = "/tmp/language-query"
log_level = "info"
```

## Installation

```bash
cargo install language-query
```

Or build from source:

```bash
git clone https://github.com/yourusername/language-query
cd language-query
cargo build --release
cargo install --path .
```

### Using as a Cargo Alias

If you're working within a Rust project, you can use the provided cargo alias:

```bash
# In your project directory with .cargo/config.toml
cargo lq docs src/main.rs:42 MyStruct
cargo lq refs src/lib.rs:10 MyTrait
```

To add the alias to your own project, add this to `.cargo/config.toml`:

```toml
[alias]
lq = "run --quiet --"
```

## Implementation Notes

The initial version will directly port the four core tools from mcp-rust-analyzer:

1. **symbol_docs** → `lq docs` - Retrieves hover documentation for symbols
2. **symbol_impl** → `lq impl` - Shows implementation source code or location
3. **symbol_references** → `lq refs` - Lists all references with context
4. **symbol_resolve** → `lq resolve` - Fuzzy symbol search within a file

Future versions may add additional commands like jump-to-definition and list-symbols based on user needs.

## Requirements

- Rust toolchain (for building)
- LSP server for your target language(s)
- Unix-like OS: **Linux or macOS only** (Windows support not planned)

## Technical Details

### IPC Protocol

The CLI and daemon communicate using a simple JSON-RPC inspired protocol:

```json
// Request
{
  "id": "unique-id",
  "method": "symbol_docs",
  "params": {
    "file": "/path/to/file.rs",
    "line": 42,
    "symbol": "my_function"
  }
}

// Response
{
  "id": "unique-id",
  "result": {
    "documentation": "Function documentation here...",
    "type": "fn my_function(x: i32) -> String"
  }
}
```

### Performance Optimizations

- Daemon reuse eliminates LSP startup overhead
- Document caching reduces redundant file reads
- Debounced file watching for efficient change tracking
- Request deduplication for concurrent queries
- Lazy indexing updates

## License

MIT License - same as mcp-rust-analyzer
