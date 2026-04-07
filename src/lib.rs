//! `PersonalAgent` - A macOS menu bar AI assistant
//!
//! This library provides the core functionality for the `PersonalAgent` application,
//! including configuration management, model profiles, conversation storage, and more.

// GPUI render methods require `&mut Context<Self>` by convention even when
// the function body does not mutate through it.
#![allow(clippy::needless_pass_by_ref_mut)]
// Presenter start/stop/handler methods use async signatures for consistency
// with the async runtime even when individual implementations have no awaits.
#![allow(clippy::unused_async)]

pub mod agent;
pub mod backup;
pub mod compression;
pub mod config;
pub mod db;
pub mod error;
pub mod events;
pub mod llm;
pub mod mcp;
pub mod models;
pub mod presentation;
pub mod registry;
pub mod services;
pub mod ui_gpui;

// Re-export commonly used types
pub use config::{CompressionConfig, Config, ContextManagement};
pub use error::{AppError, Result};
pub use events::{emit, subscribe, AppEvent, EventBus, EventBusError};
pub use llm::{LlmClient, LlmError, Message as LlmMessage, Role as LlmRole, StreamEvent};
pub use models::{
    AuthConfig, Conversation, Message, MessageRole, ModelParameters, ModelProfile, Skill,
    SkillMetadata, SkillSource,
};
pub use registry::{ModelInfo, ModelRegistry, RegistryManager};
pub use services::{
    AppSettingsService, ChatService, ConversationService, McpRegistryService, McpService,
    ModelsRegistryService, ProfileService, SecretsService, ServiceError, ServiceResult,
    SkillsService,
};

// @plan PLAN-20250125-REFACTOR.P04
// Events module added for EventBus implementation
// @plan PLAN-20250127-REFACTOR.P07
// Services module added for service layer trait definitions
// @plan PLAN-20250127-REFACTOR.P10
// Presentation module added for presenter layer stubs
// @plan PLAN-20250127-REFACTOR.P13
// App and AppContext modules added for application bootstrap and shared state
