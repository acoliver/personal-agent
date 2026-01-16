//! `PersonalAgent` - A macOS menu bar AI assistant
//!
//! This library provides the core functionality for the `PersonalAgent` application,
//! including configuration management, model profiles, conversation storage, and more.

pub mod agent;
pub mod config;
pub mod error;
pub mod llm;
pub mod mcp;
pub mod models;
pub mod registry;
pub mod storage;

// Re-export commonly used types
pub use config::{Config, ContextManagement};
pub use error::{AppError, Result};
pub use llm::{LlmClient, LlmError, Message as LlmMessage, Role as LlmRole, StreamEvent};
pub use models::{AuthConfig, Conversation, Message, MessageRole, ModelParameters, ModelProfile};
pub use registry::{ModelInfo, ModelRegistry, RegistryManager};
pub use storage::ConversationStorage;
