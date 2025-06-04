use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

#[async_trait]
pub trait LspConnection: Send + Sync {
    async fn hover(&self, file: &Path, line: u32, symbol: &str) -> Result<Option<String>>;
    async fn implementation(&self, file: &Path, line: u32, symbol: &str) -> Result<Option<String>>;
    async fn references(&self, file: &Path, line: u32, symbol: &str) -> Result<Vec<String>>;
    async fn resolve_symbol(&self, file: &Path, symbol: &str) -> Result<Option<String>>;
}

/// Mock implementation with hard-coded responses for testing
pub struct MockLspConnection;

impl MockLspConnection {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl LspConnection for MockLspConnection {
    async fn hover(&self, file: &Path, line: u32, symbol: &str) -> Result<Option<String>> {
        // Simulate some async work
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let file_name = file.file_name().unwrap_or_default().to_string_lossy();
        
        Ok(Some(format!(
            "```rust\n{}: String\n```\n\n---\n\nDocumentation for `{}` at {}:{}",
            symbol,
            symbol,
            file_name,
            line
        )))
    }
    
    async fn implementation(&self, file: &Path, line: u32, symbol: &str) -> Result<Option<String>> {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let file_name = file.file_name().unwrap_or_default().to_string_lossy();
        
        Ok(Some(format!(
            "{}:{}:{}:\n```rust\nimpl {} {{\n    fn new() -> Self {{\n        todo!()\n    }}\n}}\n```",
            file_name,
            line,
            line + 5,
            symbol
        )))
    }
    
    async fn references(&self, file: &Path, line: u32, symbol: &str) -> Result<Vec<String>> {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let file_name = file.file_name().unwrap_or_default().to_string_lossy();
        
        Ok(vec![
            format!("{}:{}: let x = {};", file_name, line + 10, symbol),
            format!("{}:{}: {}.method();", file_name, line + 20, symbol),
            format!("{}:{}: println!(\"{{}}\", {});", file_name, line + 30, symbol),
        ])
    }
    
    async fn resolve_symbol(&self, file: &Path, symbol: &str) -> Result<Option<String>> {
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        
        let file_name = file.file_name().unwrap_or_default().to_string_lossy();
        
        Ok(Some(format!(
            "Found symbol `{}` in {}:\n\n```rust\nstruct {} {{\n    field: String,\n}}\n```\n\nA mock struct for testing.",
            symbol,
            file_name,
            symbol
        )))
    }
}