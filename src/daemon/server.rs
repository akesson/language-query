use std::path::{Path, PathBuf};
use std::sync::Arc;
use anyhow::{Result, Context};
use tokio::net::{UnixListener, UnixStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, error, debug};

use crate::core::LanguageQueryService;
use crate::ipc::{Request, Response, Method, ResponseResult};

pub struct DaemonServer {
    service: Arc<LanguageQueryService>,
    socket_path: PathBuf,
    listener: UnixListener,
}

impl DaemonServer {
    pub async fn new(workspace: &Path, socket_path: PathBuf) -> Result<Self> {
        // Remove existing socket if it exists
        if socket_path.exists() {
            std::fs::remove_file(&socket_path)
                .context("Failed to remove existing socket")?;
        }
        
        let listener = UnixListener::bind(&socket_path)
            .context("Failed to bind to socket")?;
        
        info!("Daemon listening on: {:?}", socket_path);
        
        let service = Arc::new(LanguageQueryService::new(workspace).await?);
        
        Ok(Self {
            service,
            socket_path,
            listener,
        })
    }
    
    pub async fn run(self) -> Result<()> {
        let service = self.service.clone();
        let socket_path = self.socket_path.clone();
        
        // Handle shutdown signal
        let shutdown = Arc::new(tokio::sync::Notify::new());
        let shutdown_clone = shutdown.clone();
        
        tokio::spawn(async move {
            use futures::stream::StreamExt;
            match signal_hook_tokio::Signals::new(&[signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT]) {
                Ok(mut signals) => {
                    if let Some(_signal) = signals.next().await {
                        info!("Received shutdown signal");
                        shutdown_clone.notify_one();
                    }
                }
                Err(e) => {
                    error!("Failed to register signal handler: {}", e);
                }
            }
        });
        
        loop {
            tokio::select! {
                result = self.listener.accept() => {
                    match result {
                        Ok((stream, _addr)) => {
                            let service = service.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_client(stream, service).await {
                                    error!("Error handling client: {}", e);
                                }
                            });
                        }
                        Err(e) => {
                            error!("Failed to accept connection: {}", e);
                        }
                    }
                }
                _ = shutdown.notified() => {
                    info!("Shutting down daemon");
                    break;
                }
            }
        }
        
        // Cleanup
        drop(self.listener);
        let _ = std::fs::remove_file(&socket_path);
        
        Ok(())
    }
}

async fn handle_client(mut stream: UnixStream, service: Arc<LanguageQueryService>) -> Result<()> {
    let mut buffer = vec![0; 65536]; // 64KB buffer
    
    loop {
        // Read length prefix (4 bytes)
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {},
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                debug!("Client disconnected");
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        }
        
        let msg_len = u32::from_be_bytes(len_buf) as usize;
        if msg_len > buffer.len() {
            buffer.resize(msg_len, 0);
        }
        
        // Read message
        stream.read_exact(&mut buffer[..msg_len]).await?;
        
        // Parse request
        let request: Request = serde_json::from_slice(&buffer[..msg_len])
            .context("Failed to parse request")?;
        
        debug!("Received request: {:?}", request.method);
        
        let is_shutdown = matches!(request.method, Method::Shutdown);
        
        // Handle request
        let response = match handle_request(request.id.clone(), request.method, &service).await {
            Ok(result) => Response {
                id: request.id,
                result: ResponseResult::Success { result },
            },
            Err(e) => Response {
                id: request.id,
                result: ResponseResult::Error { error: e.to_string() },
            },
        };
        
        // Send response
        let response_bytes = serde_json::to_vec(&response)?;
        let len_bytes = (response_bytes.len() as u32).to_be_bytes();
        stream.write_all(&len_bytes).await?;
        stream.write_all(&response_bytes).await?;
        stream.flush().await?;
        
        // Check if this was a shutdown request
        if matches!(response.result, ResponseResult::Success { .. }) && is_shutdown {
            info!("Received shutdown request");
            // Give client time to receive response
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            std::process::exit(0);
        }
    }
}

async fn handle_request(
    _id: String,
    method: Method,
    service: &LanguageQueryService,
) -> Result<serde_json::Value> {
    match method {
        Method::Docs { file, line, symbol } => {
            let result = service.get_docs(&file, line, &symbol).await?;
            Ok(serde_json::json!({ "docs": result }))
        }
        Method::Impl { file, line, symbol } => {
            let result = service.get_impl(&file, line, &symbol).await?;
            Ok(serde_json::json!({ "implementation": result }))
        }
        Method::Refs { file, line, symbol } => {
            let result = service.get_refs(&file, line, &symbol).await?;
            Ok(serde_json::json!({ "references": result }))
        }
        Method::Resolve { file, symbol } => {
            let result = service.resolve_symbol(&file, &symbol).await?;
            Ok(serde_json::json!({ "resolved": result }))
        }
        Method::Status => {
            Ok(serde_json::json!({
                "status": "ready",
                "workspace": service.workspace_path().display().to_string(),
                "indexing": false,
            }))
        }
        Method::Shutdown => {
            Ok(serde_json::json!({ "shutdown": true }))
        }
    }
}