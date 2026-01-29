//! `PersonalAgent` - A macOS menu bar AI assistant
//!
//! This library provides the core functionality for the `PersonalAgent` application,
//! including configuration management, model profiles, conversation storage, and more.

pub mod agent;
pub mod app;
pub mod app_context;
pub mod config;
pub mod error;
pub mod events;
pub mod llm;
pub mod main_utils;
pub mod mcp;
pub mod migration;
pub mod models;
pub mod presentation;
pub mod registry;
pub mod services;
pub mod storage;

// Re-export commonly used types
pub use app::{App, AppContext, ServiceRegistry};
pub use app_context::AppContext as AppContextExt;
pub use config::{Config, ContextManagement};
pub use error::{AppError, Result};
pub use events::{emit, subscribe, AppEvent, EventBus, EventBusError};
pub use llm::{LlmClient, LlmError, Message as LlmMessage, Role as LlmRole, StreamEvent};
pub use models::{AuthConfig, Conversation, Message, MessageRole, ModelParameters, ModelProfile};
pub use registry::{ModelInfo, ModelRegistry, RegistryManager};
pub use services::{
    AppSettingsService, ChatService, ConversationService, McpRegistryService, McpService,
    ModelsRegistryService, ProfileService, SecretsService, ServiceError, ServiceResult,
};
pub use storage::ConversationStorage;

// @plan PLAN-20250125-REFACTOR.P04
// Events module added for EventBus implementation
// @plan PLAN-20250127-REFACTOR.P07
// Services module added for service layer trait definitions
// @plan PLAN-20250127-REFACTOR.P10
// Presentation module added for presenter layer stubs
// @plan PLAN-20250127-REFACTOR.P13
// App and AppContext modules added for application bootstrap and shared state
// @plan PLAN-20250125-REFACTOR.P14
// Migration module added for data compatibility and migration paths
