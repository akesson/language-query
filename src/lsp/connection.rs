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