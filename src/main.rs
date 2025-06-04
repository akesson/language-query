use anyhow::{Result, Context};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command;
use tokio::net::UnixStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use language_query::{
    daemon::{get_socket_path, is_daemon_running, DaemonServer},
    ipc::{Request, Response, Method, ResponseResult},
};

#[derive(Parser)]
#[command(name = "lq")]
#[command(about = "Language Query - Fast CLI for LSP code intelligence", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Get documentation/hover information for a symbol
    Docs {
        /// File path and line number (e.g., src/main.rs:42)
        #[arg(value_parser = parse_file_location)]
        location: FileLocation,
        /// Symbol name to query
        symbol: String,
    },
    /// Show the implementation of a symbol
    Impl {
        /// File path and line number (e.g., src/main.rs:42)
        #[arg(value_parser = parse_file_location)]
        location: FileLocation,
        /// Symbol name to query
        symbol: String,
    },
    /// Find all references to a symbol
    Refs {
        /// File path and line number (e.g., src/main.rs:42)
        #[arg(value_parser = parse_file_location)]
        location: FileLocation,
        /// Symbol name to query
        symbol: String,
    },
    /// Search for symbols by name (fuzzy matching)
    Resolve {
        /// Symbol name to search for
        symbol: String,
        /// File to search within
        file: PathBuf,
    },
    /// Check daemon status and indexing progress
    Status,
    /// Stop the daemon for current workspace
    Stop,
    /// View daemon logs
    Logs {
        /// Number of lines to show (default: 50)
        #[arg(short = 'n', long = "lines", default_value = "50")]
        lines: usize,
    },
    /// Start the daemon process (usually called automatically)
    #[command(hide = true)]
    Daemon {
        /// Workspace root directory
        #[arg(long)]
        workspace: PathBuf,
    },
}

#[derive(Debug, Clone)]
struct FileLocation {
    file: PathBuf,
    line: u32,
}

fn parse_file_location(s: &str) -> Result<FileLocation, String> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err("Expected format: file:line (e.g., src/main.rs:42)".to_string());
    }
    
    let file = PathBuf::from(parts[0]);
    let line = parts[1]
        .parse::<u32>()
        .map_err(|_| "Line number must be a positive integer".to_string())?;
    
    if line == 0 {
        return Err("Line number must be greater than 0".to_string());
    }
    
    Ok(FileLocation { file, line })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Daemon { workspace } => {
            // Initialize logging for daemon
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_target(false)
                        .with_thread_ids(true)
                )
                .with(tracing_subscriber::EnvFilter::from_default_env())
                .init();
            
            run_daemon(workspace).await
        }
        _ => {
            // For client commands, find workspace and ensure daemon is running
            let workspace = std::env::current_dir()
                .context("Failed to get current directory")?;
            
            let socket_path = get_socket_path(&workspace)?;
            
            // Start daemon if not running
            if !is_daemon_running(&socket_path).await {
                start_daemon(&workspace)?;
                
                // Wait for daemon to be ready
                for _ in 0..50 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    if is_daemon_running(&socket_path).await {
                        break;
                    }
                }
                
                if !is_daemon_running(&socket_path).await {
                    anyhow::bail!("Failed to start daemon");
                }
            }
            
            // Send request to daemon
            send_request_to_daemon(&socket_path, cli.command).await
        }
    }
}

async fn run_daemon(workspace: PathBuf) -> Result<()> {
    let socket_path = get_socket_path(&workspace)?;
    let server = DaemonServer::new(&workspace, socket_path).await?;
    server.run().await
}

fn start_daemon(workspace: &PathBuf) -> Result<()> {
    let exe = std::env::current_exe()
        .context("Failed to get current executable")?;
    
    Command::new(&exe)
        .arg("daemon")
        .arg("--workspace")
        .arg(workspace)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn daemon")?;
    
    Ok(())
}

async fn send_request_to_daemon(socket_path: &PathBuf, command: Commands) -> Result<()> {
    let mut stream = UnixStream::connect(socket_path).await
        .context("Failed to connect to daemon")?;
    
    let request = match command {
        Commands::Docs { location, symbol } => Request {
            id: uuid::Uuid::new_v4().to_string(),
            method: Method::Docs {
                file: location.file,
                line: location.line,
                symbol,
            },
        },
        Commands::Impl { location, symbol } => Request {
            id: uuid::Uuid::new_v4().to_string(),
            method: Method::Impl {
                file: location.file,
                line: location.line,
                symbol,
            },
        },
        Commands::Refs { location, symbol } => Request {
            id: uuid::Uuid::new_v4().to_string(),
            method: Method::Refs {
                file: location.file,
                line: location.line,
                symbol,
            },
        },
        Commands::Resolve { symbol, file } => Request {
            id: uuid::Uuid::new_v4().to_string(),
            method: Method::Resolve { file, symbol },
        },
        Commands::Status => Request {
            id: uuid::Uuid::new_v4().to_string(),
            method: Method::Status,
        },
        Commands::Stop => Request {
            id: uuid::Uuid::new_v4().to_string(),
            method: Method::Shutdown,
        },
        Commands::Logs { lines } => {
            eprintln!("Log viewing not yet implemented (would show {} lines)", lines);
            return Ok(());
        }
        _ => unreachable!(),
    };
    
    // Send request
    let request_bytes = serde_json::to_vec(&request)?;
    let len_bytes = (request_bytes.len() as u32).to_be_bytes();
    stream.write_all(&len_bytes).await?;
    stream.write_all(&request_bytes).await?;
    stream.flush().await?;
    
    // Read response
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let msg_len = u32::from_be_bytes(len_buf) as usize;
    
    let mut buffer = vec![0; msg_len];
    stream.read_exact(&mut buffer).await?;
    
    let response: Response = serde_json::from_slice(&buffer)?;
    
    match response.result {
        ResponseResult::Success { result } => {
            // Format output based on method
            match request.method {
                Method::Docs { .. } => {
                    if let Some(docs) = result.get("docs").and_then(|v| v.as_str()) {
                        println!("{}", docs);
                    }
                }
                Method::Impl { .. } => {
                    if let Some(implementation) = result.get("implementation").and_then(|v| v.as_str()) {
                        println!("{}", implementation);
                    }
                }
                Method::Refs { .. } => {
                    if let Some(references) = result.get("references").and_then(|v| v.as_array()) {
                        for reference in references {
                            if let Some(ref_str) = reference.as_str() {
                                println!("{}", ref_str);
                            }
                        }
                    }
                }
                Method::Resolve { .. } => {
                    if let Some(resolved) = result.get("resolved").and_then(|v| v.as_str()) {
                        println!("{}", resolved);
                    }
                }
                Method::Status => {
                    println!("Status: {}", result.get("status").and_then(|v| v.as_str()).unwrap_or("unknown"));
                    println!("Workspace: {}", result.get("workspace").and_then(|v| v.as_str()).unwrap_or("unknown"));
                    println!("Indexing: {}", result.get("indexing").and_then(|v| v.as_bool()).unwrap_or(false));
                }
                Method::Shutdown => {
                    println!("Daemon stopped");
                }
            }
        }
        ResponseResult::Error { error } => {
            eprintln!("Error: {}", error);
            std::process::exit(1);
        }
    }
    
    Ok(())
}