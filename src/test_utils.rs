#[cfg(test)]
use crate::core::LanguageQueryService;
#[cfg(test)]
use crate::lsp::{LspConnection, MockLspConnection};
#[cfg(test)]
use std::path::Path;

#[cfg(test)]
pub async fn create_test_service(_workspace: &Path) -> LanguageQueryService {
    // For tests, we directly create the service with mock LSP
    let lsp: Box<dyn LspConnection> = Box::new(MockLspConnection::new());
    LanguageQueryService::new_with_lsp(lsp)
}