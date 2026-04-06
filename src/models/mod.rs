//! Domain models for `PersonalAgent`

mod context_state;
mod conversation;
mod conversation_export;
pub mod profile;
mod search;

pub use context_state::{CompressionPhase, ContextState};
pub use conversation::{Conversation, ConversationMetadata, Message, MessageRole};
pub use conversation_export::ConversationExportFormat;
pub use profile::{AuthConfig, ModelParameters, ModelProfile};
pub use search::{SearchMatchType, SearchResult};
