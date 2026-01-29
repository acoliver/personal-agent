//! Service layer traits for the personal-agent application
//!
//! This module defines the core service interfaces used throughout the application.
//! All services use async traits and return a unified `ServiceError`.
//!
//! # Architecture
//!
//! The service layer provides the business logic of the application:
//! - Services are defined as traits for testability and flexibility
//! - Implementations handle specific concerns (chat, MCP, profiles, etc.)
//! - Services emit events to report state changes
//! - Presenters coordinate service calls and handle events
//!
//! @plan PLAN-20250125-REFACTOR.P09

pub mod app_settings;
pub mod app_settings_impl;
pub mod chat;
pub mod chat_impl;
pub mod conversation;
pub mod conversation_impl;
pub mod mcp;
pub mod mcp_impl;
pub mod mcp_registry;
pub mod mcp_registry_impl;
pub mod models_registry;
pub mod models_registry_impl;
pub mod profile;
pub mod profile_impl;
pub mod secrets;
pub mod secrets_impl;

use thiserror::Error;

/// Unified error type for all service operations
#[derive(Error, Debug, Clone)]
pub enum ServiceError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for service operations
pub type ServiceResult<T> = std::result::Result<T, ServiceError>;

// Re-export service traits
pub use app_settings::AppSettingsService;
pub use chat::ChatService;
pub use conversation::ConversationService;
pub use mcp::McpService;
pub use mcp_registry::McpRegistryService;
pub use models_registry::ModelsRegistryService;
pub use profile::ProfileService;
pub use secrets::SecretsService;

// Re-export service implementations
pub use app_settings_impl::AppSettingsServiceImpl;
pub use chat_impl::ChatServiceImpl;
pub use conversation_impl::ConversationServiceImpl;
pub use mcp_impl::McpServiceImpl;
pub use mcp_registry_impl::McpRegistryServiceImpl;
pub use models_registry_impl::ModelsRegistryServiceImpl;
pub use profile_impl::ProfileServiceImpl;
pub use secrets_impl::SecretsServiceImpl;

// Re-export types used by service traits
pub use chat::ChatStreamEvent;
pub use mcp::{McpServerStatus, McpTool};
pub use mcp_registry::McpRegistryEntry;
