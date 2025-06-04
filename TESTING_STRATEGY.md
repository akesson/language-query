# Testing Strategy for Language Query

## Integration Testing Approach

The integration tests will use a dual-mode LSP connection strategy:
- **Production**: LSP server runs in separate process, communication via IPC bridge
- **Testing**: LSP server runs on a thread in the same process

This allows integration tests to run as standard Rust unit tests with snapshot testing via insta.

## Architecture

### LSP Connection Trait

```rust
// src/lsp/connection.rs
#[async_trait]
pub trait LspConnection: Send + Sync {
    async fn initialize(&self, params: InitializeParams) -> Result<()>;
    async fn hover(&self, file: &Path, position: Position) -> Result<Option<Hover>>;
    async fn definition(&self, file: &Path, position: Position) -> Result<Option<GotoDefinitionResponse>>;
    async fn references(&self, file: &Path, position: Position) -> Result<Option<Vec<Location>>>;
    async fn document_symbols(&self, file: &Path) -> Result<Option<Vec<DocumentSymbol>>>;
}

// Production implementation - IPC bridge to separate process
pub struct IpcLspConnection {
    bridge: IpcBridge,
}

// Test implementation - in-process tokio task
pub struct ThreadLspConnection {
    handle: tokio::task::JoinHandle<()>,
    client: Arc<Mutex<ServerSocket>>,
}
```

### Service Layer

```rust
// src/core/service.rs
pub struct LanguageQueryService {
    lsp: Box<dyn LspConnection>,
}

impl LanguageQueryService {
    pub fn new(lsp: Box<dyn LspConnection>) -> Self {
        Self { lsp }
    }
    
    pub async fn get_docs(&self, file: &Path, line: u32, symbol: &str) -> Result<String> {
        // Implementation using self.lsp
    }
}
```

### Test Configuration

```rust
// src/test_utils.rs
#[cfg(test)]
pub async fn create_test_service() -> LanguageQueryService {
    // Create ThreadLspConnection
    let (mainloop, client) = MainLoop::new_client(|_| {
        // Configure LSP server
    });
    
    // Run mainloop on tokio task
    let handle = tokio::spawn(async move {
        // Run LSP server
        mainloop.run_buffered(stdin, stdout).await
    });
    
    let lsp = Box::new(ThreadLspConnection { handle, client });
    LanguageQueryService::new(lsp)
}
```

## Integration Tests with Insta

### Test Structure

```rust
// src/core/tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_symbol_docs() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        std::fs::write(&test_file, r#"
            struct TestStruct {
                /// This is a test field
                field: String,
            }
        "#).unwrap();
        
        let service = create_test_service().await;
        service.initialize(&temp_dir.path()).await.unwrap();
        
        let result = service.get_docs(&test_file, 3, "field").await.unwrap();
        assert_snapshot!(result);
    }
    
    #[tokio::test]
    async fn test_symbol_impl() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        std::fs::write(&test_file, r#"
            impl TestStruct {
                fn new() -> Self {
                    Self { field: String::new() }
                }
            }
        "#).unwrap();
        
        let service = create_test_service().await;
        service.initialize(&temp_dir.path()).await.unwrap();
        
        let result = service.get_impl(&test_file, 2, "new").await.unwrap();
        assert_snapshot!(result);
    }
    
    #[tokio::test]
    async fn test_symbol_references() {
        let temp_dir = TempDir::new().unwrap();
        // Create multiple files with references
        let lib_file = temp_dir.path().join("lib.rs");
        let main_file = temp_dir.path().join("main.rs");
        
        std::fs::write(&lib_file, r#"
            pub fn helper() -> i32 { 42 }
        "#).unwrap();
        
        std::fs::write(&main_file, r#"
            use crate::helper;
            
            fn main() {
                let x = helper();
                println!("{}", helper());
            }
        "#).unwrap();
        
        let service = create_test_service().await;
        service.initialize(&temp_dir.path()).await.unwrap();
        
        let result = service.get_references(&lib_file, 2, "helper").await.unwrap();
        assert_snapshot!(result);
    }
}
```

### Snapshot Management

Snapshots will be stored in `src/core/snapshots/` and managed by insta:

```
src/core/snapshots/
├── language_query__core__tests__symbol_docs.snap
├── language_query__core__tests__symbol_impl.snap
└── language_query__core__tests__symbol_references.snap
```

## Benefits

1. **Fast Tests**: No process spawning, everything runs in-process
2. **Reliable**: No IPC flakiness or timing issues
3. **Standard Rust Tests**: Run with `cargo test`
4. **Snapshot Testing**: Easy to review changes with `cargo insta review`
5. **Real LSP Server**: Tests use actual rust-analyzer, not mocks

## Dependencies to Add

```toml
[dev-dependencies]
insta = { version = "1.39", features = ["yaml"] }
tempfile = "3.10"
```

## Running Tests

```bash
# Run all tests
cargo test

# Review snapshot changes
cargo insta review

# Update snapshots
cargo insta accept
```