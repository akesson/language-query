use std::path::{Path, PathBuf};
use anyhow::{Result, bail};

use crate::lsp::{LspConnection, RustAnalyzerConnection};

pub struct LanguageQueryService {
    lsp: Box<dyn LspConnection>,
    workspace: PathBuf,
}

impl LanguageQueryService {
    pub async fn new(workspace: &Path) -> Result<Self> {
        // Check if we have a Rust project
        if workspace.join("Cargo.toml").exists() {
            let lsp = Box::new(RustAnalyzerConnection::new(workspace).await?);
            Ok(Self {
                lsp,
                workspace: workspace.to_path_buf(),
            })
        } else {
            bail!("Not a Rust project (no Cargo.toml found). Only Rust projects are currently supported.")
        }
    }
    
    pub fn workspace_path(&self) -> &Path {
        &self.workspace
    }
    
    pub async fn get_docs(&self, file: &Path, line: u32, symbol: &str) -> Result<Option<String>> {
        self.lsp.hover(file, line, symbol).await
    }
    
    pub async fn get_impl(&self, file: &Path, line: u32, symbol: &str) -> Result<Option<String>> {
        self.lsp.implementation(file, line, symbol).await
    }
    
    pub async fn get_refs(&self, file: &Path, line: u32, symbol: &str) -> Result<Vec<String>> {
        self.lsp.references(file, line, symbol).await
    }
    
    pub async fn resolve_symbol(&self, file: &Path, symbol: &str) -> Result<Option<String>> {
        self.lsp.resolve_symbol(file, symbol).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    fn ensure_rust_analyzer() {
        // Ensure rust-analyzer is available
        std::process::Command::new("rust-analyzer")
            .arg("--version")
            .output()
            .expect("rust-analyzer not found. Please install rust-analyzer to run tests.");
    }
    
    async fn create_test_project() -> Result<(TempDir, PathBuf)> {
        let temp_dir = TempDir::new()?;
        
        // Create a minimal Cargo.toml
        std::fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"[package]
name = "test_project"
version = "0.1.0"
edition = "2021"
"#
        )?;
        
        // Create src directory
        std::fs::create_dir_all(temp_dir.path().join("src"))?;
        
        // Create a simple lib.rs
        let lib_file = temp_dir.path().join("src/lib.rs");
        std::fs::write(&lib_file, r#"//! Test library

/// A test struct
pub struct TestStruct {
    /// The value field
    pub value: String,
}

impl TestStruct {
    /// Creates a new TestStruct
    pub fn new(value: String) -> Self {
        Self { value }
    }
}

/// Test function using TestStruct
pub fn use_test_struct(ts: &TestStruct) -> &str {
    &ts.value
}

use std::collections::HashMap;

/// Creates a test map
pub fn create_map() -> HashMap<String, i32> {
    let mut map = HashMap::new();
    map.insert("test".to_string(), 42);
    map
}
"#)?;
        
        Ok((temp_dir, lib_file))
    }
    
    fn redact_temp_path(content: &str, temp_path: &Path) -> String {
        content.replace(&temp_path.to_string_lossy().to_string(), "[TEMP_DIR]")
    }
    
    #[tokio::test]
    async fn test_service_creation() {
        ensure_rust_analyzer();
        
        let (temp_dir, _) = create_test_project().await.unwrap();
        
        // Test that we can create a service
        let service = LanguageQueryService::new(temp_dir.path()).await.unwrap();
        assert_eq!(service.workspace_path(), temp_dir.path());
    }
    
    #[tokio::test]
    async fn test_non_rust_project_fails() {
        ensure_rust_analyzer();
        
        let temp_dir = TempDir::new().unwrap();
        
        // Should fail without Cargo.toml
        let result = LanguageQueryService::new(temp_dir.path()).await;
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("Not a Rust project"));
    }
    
    #[tokio::test]
    async fn test_docs_command() {
        ensure_rust_analyzer();
        
        let (temp_dir, lib_file) = create_test_project().await.unwrap();
        let service = LanguageQueryService::new(temp_dir.path()).await.unwrap();
        
        // No need to wait - the service waits for ready internally
        
        // Get docs for TestStruct
        let result = service.get_docs(&lib_file, 4, "TestStruct").await;
        
        match result {
            Ok(Some(docs)) => {
                // Redact the temp directory path
                let redacted = redact_temp_path(&docs, temp_dir.path());
                insta::assert_snapshot!("test_docs_command", redacted);
            }
            Ok(None) => panic!("Expected documentation for TestStruct"),
            Err(e) => panic!("Failed to get docs: {}", e),
        }
    }
    
    #[tokio::test]
    async fn test_impl_command() {
        ensure_rust_analyzer();
        
        let (temp_dir, lib_file) = create_test_project().await.unwrap();
        let service = LanguageQueryService::new(temp_dir.path()).await.unwrap();
        
        // Try to get implementation of TestStruct at the struct definition
        let result = service.get_impl(&lib_file, 4, "TestStruct").await.unwrap();
        
        if let Some(implementation) = result {
            let redacted = redact_temp_path(&implementation, temp_dir.path());
            insta::assert_snapshot!("test_impl_command", redacted);
        } else {
            // It's OK if go-to-definition returns None for struct definition
            println!("No implementation found (acceptable for struct definition)");
        }
    }
    
    #[tokio::test]
    async fn test_refs_command() {
        ensure_rust_analyzer();
        
        let (temp_dir, lib_file) = create_test_project().await.unwrap();
        let service = LanguageQueryService::new(temp_dir.path()).await.unwrap();
        
        // Find references to TestStruct
        let result = service.get_refs(&lib_file, 4, "TestStruct").await;
        
        match result {
            Ok(refs) => {
                assert!(!refs.is_empty(), "Expected at least one reference to TestStruct");
                
                // Redact temp paths in all references
                let redacted_refs: Vec<String> = refs.iter()
                    .map(|r| redact_temp_path(r, temp_dir.path()))
                    .collect();
                
                insta::assert_snapshot!("test_refs_command", redacted_refs.join("\n"));
            }
            Err(e) => panic!("Failed to get references: {}", e),
        }
    }
    
    #[tokio::test]
    async fn test_resolve_command() {
        ensure_rust_analyzer();
        
        let (temp_dir, lib_file) = create_test_project().await.unwrap();
        let service = LanguageQueryService::new(temp_dir.path()).await.unwrap();
        
        // Resolve HashMap
        let result = service.resolve_symbol(&lib_file, "HashMap").await.unwrap();
        
        if let Some(resolved) = result {
            let redacted = redact_temp_path(&resolved, temp_dir.path());
            insta::assert_snapshot!("test_resolve_command", redacted);
        } else {
            panic!("Expected to resolve HashMap");
        }
    }
}