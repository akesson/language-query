use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
        Commands::Docs { location, symbol } => {
            println!("Not implemented: docs for {} at {}:{}", symbol, location.file.display(), location.line);
        }
        Commands::Impl { location, symbol } => {
            println!("Not implemented: impl for {} at {}:{}", symbol, location.file.display(), location.line);
        }
        Commands::Refs { location, symbol } => {
            println!("Not implemented: refs for {} at {}:{}", symbol, location.file.display(), location.line);
        }
        Commands::Resolve { symbol, file } => {
            println!("Not implemented: resolve {} in {}", symbol, file.display());
        }
        Commands::Status => {
            println!("Not implemented: status");
        }
        Commands::Stop => {
            println!("Not implemented: stop");
        }
        Commands::Logs { lines } => {
            println!("Not implemented: logs (last {} lines)", lines);
        }
        Commands::Daemon { workspace } => {
            println!("Not implemented: daemon for workspace {}", workspace.display());
        }
    }
    
    Ok(())
}