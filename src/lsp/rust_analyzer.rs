use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::ops::ControlFlow;
use std::collections::HashSet;
use std::time::Duration;

use anyhow::{Result, Context, bail};
use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use async_lsp::{LanguageServer, MainLoop, ServerSocket};
use async_lsp::router::Router;
use async_process::Command;
use async_trait::async_trait;
use lsp_types::{
    ClientCapabilities, DidOpenTextDocumentParams, GotoDefinitionParams,
    GotoDefinitionResponse, Hover, HoverClientCapabilities, HoverParams, InitializeParams,
    InitializedParams, MarkupKind, Position, ReferenceContext, ReferenceParams,
    TextDocumentClientCapabilities, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, WindowClientCapabilities, WorkDoneProgressParams,
    WorkspaceFolder,
};
use tokio::sync::{Mutex, RwLock};
use tokio::task;
use tower::ServiceBuilder;
use tracing::{info, error};

use crate::lsp::LspConnection;

pub struct RustAnalyzerConnection {
    workspace: PathBuf,
    server: Arc<Mutex<ServerSocket>>,
    opened_files: Arc<Mutex<HashSet<PathBuf>>>,
    is_ready: Arc<RwLock<bool>>,
    #[allow(dead_code)]
    _mainloop_handle: tokio::task::JoinHandle<()>,
    #[allow(dead_code)]
    _child: async_process::Child,
}

impl RustAnalyzerConnection {
    pub async fn new(workspace: &Path) -> Result<Self> {
        let rust_analyzer_path = find_rust_analyzer()?;
        
        info!("Starting rust-analyzer at: {:?}", rust_analyzer_path);
        
        let mut child = Command::new(&rust_analyzer_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(workspace)
            .kill_on_drop(true)
            .spawn()
            .context("Failed to spawn rust-analyzer")?;
        
        let stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take().unwrap();
        
        // Create the main loop for LSP communication
        let (mainloop, server) = MainLoop::new_client(|_server| {
            ServiceBuilder::new()
                .layer(TracingLayer::default())
                .layer(LifecycleLayer::default())
                .layer(CatchUnwindLayer::default())
                .layer(ConcurrencyLayer::default())
                .service(ClientState::new_router())
        });
        
        let server = Arc::new(Mutex::new(server));
        
        // Run the main loop in a background task
        let mainloop_handle = task::spawn(async move {
            if let Err(e) = mainloop.run_buffered(stdout, stdin).await {
                error!("Language server mainloop error: {}", e);
            }
        });
        
        let is_ready = Arc::new(RwLock::new(false));
        
        let connection = Self {
            workspace: workspace.to_path_buf(),
            server,
            opened_files: Arc::new(Mutex::new(HashSet::new())),
            is_ready: is_ready.clone(),
            _mainloop_handle: mainloop_handle,
            _child: child,
        };
        
        // Initialize the LSP server
        connection.initialize().await?;
        
        // Wait for the server to be ready
        connection.wait_until_ready().await?;
        
        Ok(connection)
    }
    
    async fn initialize(&self) -> Result<()> {
        info!("Initializing rust-analyzer for workspace: {:?}", self.workspace);
        
        let initialize_params = InitializeParams {
            workspace_folders: Some(vec![WorkspaceFolder {
                uri: url::Url::from_file_path(&self.workspace)
                    .map_err(|_| anyhow::anyhow!("Invalid workspace path: {:?}", self.workspace))?,
                name: self.workspace.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
            }]),
            capabilities: ClientCapabilities {
                text_document: Some(TextDocumentClientCapabilities {
                    hover: Some(HoverClientCapabilities {
                        content_format: Some(vec![MarkupKind::Markdown]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                window: Some(WindowClientCapabilities {
                    work_done_progress: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        
        let mut server = self.server.lock().await;
        let _response = server.initialize(initialize_params).await?;
        server.initialized(InitializedParams {})?;
        
        Ok(())
    }
    
    async fn wait_until_ready(&self) -> Result<()> {
        info!("Waiting for rust-analyzer to be ready...");
        
        // For rust-analyzer, we need to wait for it to index the project
        // The best approach is to wait a reasonable amount of time based on project size
        // For test projects, 2-3 seconds should be enough
        
        // In a production system, we could:
        // 1. Use workspace/symbol requests to check if symbols are indexed
        // 2. Monitor progress notifications from the server
        // 3. Use experimental/serverStatus if available
        
        // For now, use a simple time-based approach
        let wait_time = if cfg!(test) {
            Duration::from_secs(3) // Longer wait in tests
        } else {
            Duration::from_secs(2) // Normal wait
        };
        
        info!("Waiting {:?} for rust-analyzer to initialize...", wait_time);
        tokio::time::sleep(wait_time).await;
        
        *self.is_ready.write().await = true;
        info!("rust-analyzer marked as ready");
        
        Ok(())
    }
    
    async fn ensure_ready(&self) -> Result<()> {
        if !*self.is_ready.read().await {
            bail!("LSP server is not ready yet");
        }
        Ok(())
    }
    
    async fn open_file(&self, file: &Path) -> Result<()> {
        // Make the path absolute if it's relative
        let absolute_path = if file.is_absolute() {
            file.to_path_buf()
        } else {
            std::env::current_dir()?.join(file)
        };
        
        let canonical_path = absolute_path.canonicalize()
            .unwrap_or_else(|_| absolute_path.clone());
        
        // Check if file is already open
        let mut opened = self.opened_files.lock().await;
        if opened.contains(&canonical_path) {
            return Ok(());
        }
        
        let uri = url::Url::from_file_path(&canonical_path)
            .map_err(|_| {
                error!("Failed to create URI from path: {:?}", canonical_path);
                anyhow::anyhow!("Invalid file path: {:?}", canonical_path)
            })?;
        
        let contents = tokio::fs::read_to_string(&canonical_path)
            .await
            .context("Failed to read file")?;
        
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: "rust".to_string(),
                version: 0,
                text: contents,
            },
        };
        
        let mut server = self.server.lock().await;
        server.did_open(params)?;
        
        // Mark file as opened
        opened.insert(canonical_path);
        
        Ok(())
    }
    
    /// Find the position of a symbol in a file starting from the given line
    async fn find_symbol_position(&self, file: &Path, line: u32, symbol: &str) -> Result<Position> {
        let contents = tokio::fs::read_to_string(file)
            .await
            .context("Failed to read file")?;
        
        let lines: Vec<&str> = contents.lines().collect();
        
        // Convert 1-based line to 0-based
        let line_index = (line as usize).saturating_sub(1);
        
        if line_index >= lines.len() {
            bail!("Line {} is out of bounds (file has {} lines)", line, lines.len());
        }
        
        // Search for the symbol in the specified line
        if let Some(char_index) = lines[line_index].find(symbol) {
            Ok(Position {
                line: line_index as u32,
                character: char_index as u32,
            })
        } else {
            // If not found on the exact line, search nearby lines
            for offset in 1..=2 {
                // Check lines before
                if line_index >= offset {
                    let check_line = line_index - offset;
                    if let Some(char_index) = lines[check_line].find(symbol) {
                        return Ok(Position {
                            line: check_line as u32,
                            character: char_index as u32,
                        });
                    }
                }
                
                // Check lines after
                let check_line = line_index + offset;
                if check_line < lines.len() {
                    if let Some(char_index) = lines[check_line].find(symbol) {
                        return Ok(Position {
                            line: check_line as u32,
                            character: char_index as u32,
                        });
                    }
                }
            }
            
            bail!("Symbol '{}' not found near line {}", symbol, line);
        }
    }
}

#[async_trait]
impl LspConnection for RustAnalyzerConnection {
    async fn hover(&self, file: &Path, line: u32, symbol: &str) -> Result<Option<String>> {
        // Ensure server is ready
        self.ensure_ready().await?;
        
        // Ensure file is open
        self.open_file(file).await?;
        
        let position = self.find_symbol_position(file, line, symbol).await?;
        
        // Make the path absolute if it's relative
        let absolute_path = if file.is_absolute() {
            file.to_path_buf()
        } else {
            std::env::current_dir()?.join(file)
        };
        
        let uri = url::Url::from_file_path(&absolute_path)
            .map_err(|_| anyhow::anyhow!("Invalid file path: {:?}", absolute_path))?;
        
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };
        
        // Retry logic for content modified errors
        let mut attempts = 0;
        loop {
            let mut server = self.server.lock().await;
            match server.hover(params.clone()).await {
                Ok(response) => {
                    if let Some(hover) = response {
                        let content = format_hover_content(&hover);
                        return Ok(Some(content));
                    } else {
                        return Ok(None);
                    }
                }
                Err(e) if e.to_string().contains("content modified") && attempts < 3 => {
                    drop(server); // Release the lock
                    attempts += 1;
                    info!("Retrying hover request due to content modified error (attempt {})", attempts);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
    
    async fn implementation(&self, file: &Path, line: u32, symbol: &str) -> Result<Option<String>> {
        // Ensure server is ready
        self.ensure_ready().await?;
        
        // Ensure file is open
        self.open_file(file).await?;
        
        let position = self.find_symbol_position(file, line, symbol).await?;
        
        // Make the path absolute if it's relative
        let absolute_path = if file.is_absolute() {
            file.to_path_buf()
        } else {
            std::env::current_dir()?.join(file)
        };
        
        let uri = url::Url::from_file_path(&absolute_path)
            .map_err(|_| anyhow::anyhow!("Invalid file path: {:?}", absolute_path))?;
        
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: Default::default(),
        };
        
        let mut server = self.server.lock().await;
        
        // Try to get the definition first
        let response = server.definition(params).await?;
        
        if let Some(GotoDefinitionResponse::Scalar(location)) = response {
            // Read the implementation from the file
            let impl_path = location.uri.to_file_path()
                .map_err(|_| anyhow::anyhow!("Invalid URI"))?;
            let contents = tokio::fs::read_to_string(&impl_path).await?;
            let lines: Vec<&str> = contents.lines().collect();
            
            let start_line = location.range.start.line as usize;
            let end_line = location.range.end.line as usize;
            
            // Extract more context around the definition
            let context_start = start_line.saturating_sub(1);
            let context_end = (end_line + 10).min(lines.len() - 1);
            
            let impl_lines: Vec<String> = lines[context_start..=context_end]
                .iter()
                .map(|s| s.to_string())
                .collect();
            
            let relative_path = impl_path.strip_prefix(&self.workspace).unwrap_or(&impl_path);
            
            Ok(Some(format!(
                "{}:{}:{}:\n```rust\n{}\n```",
                relative_path.display(),
                location.range.start.line + 1,
                location.range.end.line + 1,
                impl_lines.join("\n")
            )))
        } else {
            Ok(None)
        }
    }
    
    async fn references(&self, file: &Path, line: u32, symbol: &str) -> Result<Vec<String>> {
        // Ensure server is ready
        self.ensure_ready().await?;
        
        // Ensure file is open
        self.open_file(file).await?;
        
        let position = self.find_symbol_position(file, line, symbol).await?;
        
        // Make the path absolute if it's relative
        let absolute_path = if file.is_absolute() {
            file.to_path_buf()
        } else {
            std::env::current_dir()?.join(file)
        };
        
        let uri = url::Url::from_file_path(&absolute_path)
            .map_err(|_| anyhow::anyhow!("Invalid file path: {:?}", absolute_path))?;
        
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: Default::default(),
            context: ReferenceContext {
                include_declaration: false,
            },
        };
        
        // Retry logic for content modified errors
        let mut attempts = 0;
        loop {
            let mut server = self.server.lock().await;
            match server.references(params.clone()).await {
                Ok(response) => {
                    drop(server); // Release lock before doing I/O
                    
                    let mut results = Vec::new();
                    
                    if let Some(locations) = response {
                        for location in locations {
                            let ref_path = location.uri.to_file_path()
                                .map_err(|_| anyhow::anyhow!("Invalid URI"))?;
                            
                            let relative_path = ref_path.strip_prefix(&self.workspace).unwrap_or(&ref_path);
                            
                            // Read the line to show context
                            let contents = tokio::fs::read_to_string(&ref_path).await?;
                            let lines: Vec<&str> = contents.lines().collect();
                            let line_num = location.range.start.line as usize;
                            
                            if line_num < lines.len() {
                                let line_content = lines[line_num].trim();
                                results.push(format!(
                                    "{}:{}: {}",
                                    relative_path.display(),
                                    line_num + 1,
                                    line_content
                                ));
                            }
                        }
                    }
                    
                    return Ok(results);
                }
                Err(e) if e.to_string().contains("content modified") && attempts < 3 => {
                    drop(server); // Release the lock
                    attempts += 1;
                    info!("Retrying references request due to content modified error (attempt {})", attempts);
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
    
    async fn resolve_symbol(&self, file: &Path, symbol: &str) -> Result<Option<String>> {
        // Ensure server is ready
        self.ensure_ready().await?;
        
        // For symbol resolution, we'll use hover at the first occurrence
        info!("Attempting to resolve symbol '{}' in file: {:?}", symbol, file);
        let contents = tokio::fs::read_to_string(file)
            .await
            .with_context(|| format!("Failed to read file: {:?}", file))?;
        
        // Find the first occurrence of the symbol
        for (line_num, line) in contents.lines().enumerate() {
            if line.contains(symbol) {
                if let Ok(Some(hover)) = self.hover(file, (line_num + 1) as u32, symbol).await {
                    return Ok(Some(format!(
                        "Found symbol `{}` in {}:\n\n{}",
                        symbol,
                        file.file_name().unwrap_or_default().to_string_lossy(),
                        hover
                    )));
                }
            }
        }
        
        Ok(None)
    }
}

fn find_rust_analyzer() -> Result<PathBuf> {
    // Try to find rust-analyzer in PATH
    if let Ok(output) = std::process::Command::new("which")
        .arg("rust-analyzer")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            return Ok(PathBuf::from(path));
        }
    }
    
    // Common locations to check
    let common_paths = [
        "/usr/local/bin/rust-analyzer",
        "/usr/bin/rust-analyzer",
        "/opt/homebrew/bin/rust-analyzer",
        "~/.cargo/bin/rust-analyzer",
    ];
    
    for path in &common_paths {
        let expanded = shellexpand::tilde(path);
        let path = PathBuf::from(expanded.as_ref());
        if path.exists() {
            return Ok(path);
        }
    }
    
    bail!("Could not find rust-analyzer. Please ensure it is installed and in your PATH.")
}

fn format_hover_content(hover: &Hover) -> String {
    use lsp_types::HoverContents;
    use lsp_types::MarkedString;
    
    match &hover.contents {
        HoverContents::Scalar(marked) => match marked {
            MarkedString::String(s) => s.clone(),
            MarkedString::LanguageString(ls) => {
                format!("```{}\n{}\n```", ls.language, ls.value)
            }
        },
        HoverContents::Array(marked_strings) => {
            marked_strings
                .iter()
                .map(|ms| match ms {
                    MarkedString::String(s) => s.clone(),
                    MarkedString::LanguageString(ls) => {
                        format!("```{}\n{}\n```", ls.language, ls.value)
                    }
                })
                .collect::<Vec<_>>()
                .join("\n\n")
        }
        HoverContents::Markup(markup) => markup.value.clone(),
    }
}

// Minimal client state to handle LSP notifications
#[derive(Clone)]
struct ClientState;

impl ClientState {
    fn new_router() -> Router<Self> {
        let mut router = Router::new(ClientState);
        
        router.notification::<lsp_types::notification::ShowMessage>(|_state, _params| {
            ControlFlow::Continue(())
        });
        
        router.notification::<lsp_types::notification::LogMessage>(|_state, _params| {
            ControlFlow::Continue(())
        });
        
        router.notification::<lsp_types::notification::PublishDiagnostics>(|_state, _params| {
            ControlFlow::Continue(())
        });
        
        router.notification::<lsp_types::notification::Progress>(|_state, _params| {
            ControlFlow::Continue(())
        });
        
        router
    }
}

impl async_lsp::LanguageClient for ClientState {
    type Error = async_lsp::Error;
    type NotifyResult = ControlFlow<Result<(), Self::Error>>;

    fn progress(&mut self, _params: lsp_types::ProgressParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }

    fn publish_diagnostics(&mut self, _params: lsp_types::PublishDiagnosticsParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }

    fn log_message(&mut self, _params: lsp_types::LogMessageParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }

    fn show_message(&mut self, _params: lsp_types::ShowMessageParams) -> Self::NotifyResult {
        ControlFlow::Continue(())
    }
}