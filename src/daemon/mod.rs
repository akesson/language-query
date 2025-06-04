pub mod server;

use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use sha2::{Sha256, Digest};

pub use server::DaemonServer;

/// Get the socket path for a given workspace
pub fn get_socket_path(workspace: &Path) -> Result<PathBuf> {
    let socket_dir = std::env::temp_dir().join("language-query");
    std::fs::create_dir_all(&socket_dir)
        .context("Failed to create socket directory")?;
    
    // Create a unique socket name based on workspace path
    let mut hasher = Sha256::new();
    hasher.update(workspace.as_os_str().as_encoded_bytes());
    let hash = format!("{:x}", hasher.finalize());
    let socket_name = format!("lq-{}.sock", &hash[..8]);
    
    Ok(socket_dir.join(socket_name))
}

/// Check if daemon is running by checking if socket exists and is connectable
pub async fn is_daemon_running(socket_path: &Path) -> bool {
    if !socket_path.exists() {
        return false;
    }
    
    // Try to connect
    match tokio::net::UnixStream::connect(socket_path).await {
        Ok(_) => true,
        Err(_) => {
            // Socket exists but can't connect, clean it up
            let _ = std::fs::remove_file(socket_path);
            false
        }
    }
}