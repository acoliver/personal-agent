//! Domain models for `PersonalAgent`

mod conversation;
pub mod profile;

pub use conversation::{Conversation, Message, MessageRole};
pub use profile::{AuthConfig, ModelParameters, ModelProfile};
