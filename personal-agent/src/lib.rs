//! `PersonalAgent` - A macOS menu bar AI assistant
//!
//! This library provides the core functionality for the `PersonalAgent` application,
//! including configuration management, model profiles, conversation storage, and more.

pub mod config;
pub mod error;
pub mod llm;
pub mod models;
pub mod registry;
pub mod storage;

// Re-export commonly used types
pub use config::{Config, ContextManagement};
pub use error::{AppError, Result};
pub use llm::{ChatStreamEvent, LLMClient};
pub use models::{AuthConfig, Conversation, Message, MessageRole, ModelParameters, ModelProfile};
pub use registry::{ModelInfo, ModelRegistry, RegistryManager};
pub use storage::ConversationStorage;
