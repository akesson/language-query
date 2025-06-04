# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Language Query (`lq`) is a fast CLI tool that provides Language Server Protocol (LSP) code intelligence features directly from the terminal. It uses a daemon/client architecture where a persistent daemon manages LSP servers for each workspace, eliminating startup overhead.

## Common Development Commands

```bash
# Build the project
cargo build

# Run tests
cargo test

# Run a specific test
cargo test test_name

# Check code without building
cargo check

# Format code
cargo fmt

# Run linter
cargo clippy

# Build release version
cargo build --release

# Install locally
cargo install --path .

# Run directly during development
cargo run -- <args>

# Use the cargo alias (requires .cargo/config.toml)
cargo lq docs src/main.rs:42 main
```

## Architecture Overview

### Module Structure

- **`src/main.rs`**: CLI entry point that handles argument parsing and daemon management
- **`src/core/`**: Contains `LanguageQueryService` which orchestrates LSP interactions
- **`src/daemon/`**: Implements the persistent background daemon that maintains LSP connections
- **`src/ipc/`**: Defines the JSON-based IPC protocol between client and daemon
- **`src/lsp/`**: Abstracts LSP server interactions with a trait-based design

### Key Architectural Concepts

1. **Daemon/Client Split**: Each workspace gets its own daemon process that maintains a warm LSP connection. Clients communicate via Unix domain sockets.

2. **IPC Protocol**: Length-prefixed JSON messages over Unix sockets. Requests have unique IDs and methods like `symbol_docs`, `symbol_impl`, etc.

3. **LSP Abstraction**: The `LspConnection` trait allows supporting multiple language servers. Currently implements `RustAnalyzerConnection` with a mock for testing.

4. **Workspace Isolation**: Daemons are workspace-specific, identified by a hash of the workspace path.

### Testing Strategy

The project uses snapshot testing with `insta` for validating LSP responses. Mock LSP implementations are available for testing without a real language server.

## Project Context

This is a port of the mcp-rust-analyzer tools to a standalone CLI, focusing on the four core commands:
- `docs` (hover/documentation)
- `impl` (go to implementation)
- `refs` (find references)
- `resolve` (fuzzy symbol search)