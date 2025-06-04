use std::path::{Path, PathBuf};
use anyhow::Result;

use crate::lsp::{LspConnection, MockLspConnection};

pub struct LanguageQueryService {
    lsp: Box<dyn LspConnection>,
    workspace: PathBuf,
}

impl LanguageQueryService {
    pub async fn new(workspace: &Path) -> Result<Self> {
        // For now, use mock implementation
        let lsp = Box::new(MockLspConnection::new());
        
        Ok(Self {
            lsp,
            workspace: workspace.to_path_buf(),
        })
    }
    
    #[cfg(test)]
    pub fn new_with_lsp(lsp: Box<dyn LspConnection>) -> Self {
        Self {
            lsp,
            workspace: PathBuf::from("/test"),
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
        
        let service = LanguageQueryService::new(temp_dir.path()).await.unwrap();
        let result = service.get_docs(&test_file, 3, "field").await.unwrap();
        
        assert!(result.is_some());
        let docs = result.unwrap();
        assert!(docs.contains("field: String"));
        assert!(docs.contains("Documentation for `field`"));
        
        insta::assert_snapshot!(docs);
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
        
        let service = LanguageQueryService::new(temp_dir.path()).await.unwrap();
        let result = service.get_impl(&test_file, 2, "TestStruct").await.unwrap();
        
        assert!(result.is_some());
        let impl_code = result.unwrap();
        assert!(impl_code.contains("impl TestStruct"));
        
        insta::assert_snapshot!(impl_code);
    }
    
    #[tokio::test]
    async fn test_symbol_references() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("lib.rs");
        
        std::fs::write(&test_file, r#"
            pub fn helper() -> i32 { 42 }
        "#).unwrap();
        
        let service = LanguageQueryService::new(temp_dir.path()).await.unwrap();
        let result = service.get_refs(&test_file, 2, "helper").await.unwrap();
        
        assert_eq!(result.len(), 3); // Mock returns 3 references
        for reference in &result {
            assert!(reference.contains("helper"));
        }
        
        insta::assert_yaml_snapshot!(result);
    }
    
    #[tokio::test]
    async fn test_symbol_resolve() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.rs");
        std::fs::write(&test_file, "struct Parser {}").unwrap();
        
        let service = LanguageQueryService::new(temp_dir.path()).await.unwrap();
        let result = service.resolve_symbol(&test_file, "Parser").await.unwrap();
        
        assert!(result.is_some());
        let resolved = result.unwrap();
        assert!(resolved.contains("Parser"));
        assert!(resolved.contains("struct Parser"));
        
        insta::assert_snapshot!(resolved);
    }
}