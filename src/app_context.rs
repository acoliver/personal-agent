//! Application Context - Shared application state
//!
//! Provides shared access to services, event bus, and configuration
//! throughout the application.
//!
//! @plan PLAN-20250125-REFACTOR.P13

use std::sync::Arc;

use crate::services::{AppSettingsService, ChatService, ConversationService, McpService, ProfileService};

/// Application context - shared state accessible throughout the app
///
/// This type is re-exported from app.rs for convenience.
///
/// @plan PLAN-20250125-REFACTOR.P13
pub use crate::app::AppContext;

/// Extension methods for AppContext
///
/// @plan PLAN-20250125-REFACTOR.P13
impl AppContext {
    /// Get conversation service
    ///
    /// @plan PLAN-20250125-REFACTOR.P13
    pub fn conversation_service(&self) -> Arc<dyn ConversationService> {
        Arc::clone(&self.services.conversation)
    }

    /// Get profile service
    ///
    /// @plan PLAN-20250125-REFACTOR.P13
    pub fn profile_service(&self) -> Arc<dyn ProfileService> {
        Arc::clone(&self.services.profile)
    }

    /// Get chat service
    ///
    /// @plan PLAN-20250125-REFACTOR.P13
    pub fn chat_service(&self) -> Arc<dyn ChatService> {
        Arc::clone(&self.services.chat)
    }

    /// Get MCP service
    ///
    /// @plan PLAN-20250125-REFACTOR.P13
    pub fn mcp_service(&self) -> Arc<dyn McpService> {
        Arc::clone(&self.services.mcp)
    }

    /// Get app settings service
    ///
    /// @plan PLAN-20250125-REFACTOR.P13
    pub fn app_settings_service(&self) -> Arc<dyn AppSettingsService> {
        Arc::clone(&self.services.app_settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_app_context_service_accessors() {
        // Given: a running app
        let temp_dir = TempDir::new().unwrap();
        let base_dir = temp_dir.path().to_path_buf();
        let app = App::new(base_dir).await.unwrap();
        let context = app.context();

        // When: accessing services through context methods
        let conversation_svc = context.conversation_service();
        let profile_svc = context.profile_service();
        let chat_svc = context.chat_service();
        let mcp_svc = context.mcp_service();
        let app_settings_svc = context.app_settings_service();

        // Then: services should be accessible
        assert!(Arc::ptr_eq(&conversation_svc, &context.services.conversation));
        assert!(Arc::ptr_eq(&profile_svc, &context.services.profile));
        assert!(Arc::ptr_eq(&chat_svc, &context.services.chat));
        assert!(Arc::ptr_eq(&mcp_svc, &context.services.mcp));
        assert!(Arc::ptr_eq(&app_settings_svc, &context.services.app_settings));
    }
}
