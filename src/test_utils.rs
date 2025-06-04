#[cfg(test)]
use crate::core::LanguageQueryService;
#[cfg(test)]
use std::path::Path;

#[cfg(test)]
pub async fn create_test_service(workspace: &Path) -> LanguageQueryService {
    LanguageQueryService::new(workspace).await.expect("Failed to create test service")
}