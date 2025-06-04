# Language Query Tech Stack Proposal

**Note: This implementation targets Linux and macOS only. Windows support is not planned.**

## Core Dependencies

### CLI & Argument Parsing
- **`clap`** (v4) - Industry standard CLI framework with derive macros
  - Subcommand support for `lq docs`, `lq impl`, etc.
  - Auto-generated help and shell completions

### Daemon Management
- **`daemonize`** (v0.5) - Unix daemon creation with PID file management
  - Handles fork, setsid, and file descriptor management
- **`nix`** (v0.29) - For additional Unix process control (signals, process groups)

### IPC (Inter-Process Communication)
- **`interprocess`** (v2) - Cross-platform IPC library
  - Unix domain sockets on Linux/macOS
  - Built-in async support with Tokio
  ```rust
  // Example usage
  let socket = LocalSocketStream::connect("/tmp/language-query/workspace.sock")?;
  ```

### Async Runtime
- **`tokio`** (v1) - Already used in mcp-rust-analyzer
  - Features: `["full"]` for all async primitives
  - Reuse existing async LSP code

### Protocol & Serialization
- **`serde`** (v1) + **`serde_json`** (v1) - JSON serialization
- **Custom JSON-RPC implementation** - Keep it simple
  ```rust
  #[derive(Serialize, Deserialize)]
  struct Request {
      id: String,
      method: String,
      params: serde_json::Value,
  }
  ```

### Configuration
- **`toml`** (v0.8) - TOML parsing for config files
- **`directories`** (v5) - Platform-specific config directories
  - `~/.config/language-query/` on Linux
  - `~/Library/Application Support/language-query/` on macOS

### Logging & Diagnostics
- **`tracing`** (v0.1) + **`tracing-subscriber`** - Structured logging
  - Better async support than env_logger
  - Log levels, spans for request tracking
  - File rotation for daemon logs

### File Watching
- **`notify`** (v6) - Already used in mcp-rust-analyzer
  - Cross-platform file system notifications
  - Debouncing built-in

### Error Handling
- **`anyhow`** (v1) - For application errors (already in mcp-rust-analyzer)
- **`thiserror`** (v1) - For library error types

### Process Management
- **`sysinfo`** (v0.31) - Check if daemon is already running
- **`signal-hook`** (v0.3) - Graceful shutdown handling

## Architecture-Specific Crates

### Daemon Process
```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
interprocess = { version = "2", features = ["tokio"] }
daemonize = "0.5"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"  # For log rotation
notify = "6"
anyhow = "1"
```

### CLI Client
```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
interprocess = { version = "2", features = ["tokio"] }
tokio = { version = "1", features = ["rt", "macros"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
directories = "5"
```

## Implementation Patterns

### 1. Daemon Lifecycle
```rust
use daemonize::Daemonize;

let daemonize = Daemonize::new()
    .pid_file(pid_path)
    .working_directory(workspace_root)
    .stdout(log_file.try_clone()?)
    .stderr(log_file);

match daemonize.start() {
    Ok(_) => run_daemon_server().await,
    Err(e) => eprintln!("Daemon error: {}", e),
}
```

### 2. IPC Server (Daemon)
```rust
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};

let listener = LocalSocketListener::bind(socket_path)?;
loop {
    let stream = listener.accept().await?;
    tokio::spawn(handle_client(stream));
}
```

### 3. IPC Client (CLI)
```rust
use interprocess::local_socket::LocalSocketStream;

let mut stream = LocalSocketStream::connect(socket_path).await?;
let request = Request { id, method, params };
stream.write_all(&serde_json::to_vec(&request)?).await?;
```

### 4. Auto-start Daemon
```rust
// In CLI, check if daemon is running
if !daemon_is_running(&socket_path) {
    Command::new(std::env::current_exe()?)
        .arg("daemon")
        .arg("--workspace")
        .arg(&workspace_root)
        .spawn()?;
    
    // Wait for daemon to be ready
    tokio::time::timeout(Duration::from_secs(10), async {
        while LocalSocketStream::connect(&socket_path).await.is_err() {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }).await?;
}
```

## Platform Considerations

### Unix (Linux/macOS)
- Use Unix domain sockets at `/tmp/language-query/<workspace-hash>.sock`
- PID files at `/tmp/language-query/<workspace-hash>.pid`
- Daemon fork() with proper signal handling

## Security Considerations
- Set appropriate permissions on socket files (0600)
- Validate workspace paths to prevent directory traversal
- Use workspace-specific sockets to isolate projects