#![allow(clippy::unwrap_used)]

use personal_agent::config::ContextManagement;
use personal_agent::error::{AppError, Result};
use personal_agent::llm::tools::{Tool, ToolResult, ToolUse};
use personal_agent::models::{AuthConfig, MessageRole, ModelParameters};
use personal_agent::storage::ConversationStorage;
use personal_agent::{Config, Conversation, LlmMessage, LlmRole, Message, ModelProfile};
use serde_json::json;
use uuid::Uuid;

#[test]
fn config_round_trip_and_defaults() -> Result<()> {
    let config = Config::default();
    let serialized = serde_json::to_string(&config)?;
    let deserialized: Config = serde_json::from_str(&serialized)?;
    assert_eq!(config.profiles.len(), deserialized.profiles.len());
    Ok(())
}

#[test]
fn model_profile_construction() {
    let profile = ModelProfile {
        id: Uuid::new_v4(),
        name: "Test".to_string(),
        provider_id: "openai".to_string(),
        model_id: "gpt-4o".to_string(),
        base_url: String::new(),
        auth: AuthConfig::Key {
            value: "key".to_string(),
        },
        parameters: ModelParameters {
            temperature: 0.7,
            top_p: 0.95,
            max_tokens: 1024,
            thinking_budget: None,
            enable_thinking: false,
            show_thinking: false,
        },
        system_prompt: "Be concise".to_string(),
    };

    assert_eq!(profile.provider_id, "openai");
    assert_eq!(profile.model_id, "gpt-4o");
}

#[test]
fn conversation_message_round_trip() -> Result<()> {
    let mut conversation = Conversation::new(Uuid::new_v4());
    conversation.add_message(Message::user("Hello".to_string()));
    conversation.add_message(Message::assistant("Hi".to_string()));

    let serialized = serde_json::to_string(&conversation)?;
    let deserialized: Conversation = serde_json::from_str(&serialized)?;
    assert_eq!(deserialized.messages.len(), 2);
    Ok(())
}

#[test]
fn conversation_filename_is_stable() {
    let conversation = Conversation::new(Uuid::new_v4());
    assert!(conversation.filename().ends_with(".json"));
}

#[test]
fn llm_message_helpers() {
    let system = LlmMessage::system("sys");
    let user = LlmMessage::user("hi");
    let assistant = LlmMessage::assistant("hello");
    assert_eq!(system.role, LlmRole::System);
    assert_eq!(user.role, LlmRole::User);
    assert_eq!(assistant.role, LlmRole::Assistant);
}

#[test]
fn message_role_round_trip() -> Result<()> {
    let roles = [
        MessageRole::User,
        MessageRole::Assistant,
        MessageRole::System,
    ];
    for role in roles {
        let serialized = serde_json::to_string(&role)?;
        let deserialized: MessageRole = serde_json::from_str(&serialized)?;
        assert_eq!(role, deserialized);
    }
    Ok(())
}

#[test]
fn context_management_defaults() {
    let management = ContextManagement::default();
    assert!(management.trigger_threshold > 0.0);
}

#[test]
fn tool_use_and_result_round_trip() {
    let use_request = ToolUse::new("toolu_1", "search", json!({"query": "hi"}));
    let result = ToolResult::success("toolu_1", "ok");

    let use_json = serde_json::to_string(&use_request).unwrap();
    let result_json = serde_json::to_string(&result).unwrap();

    let use_back: ToolUse = serde_json::from_str(&use_json).unwrap();
    let result_back: ToolResult = serde_json::from_str(&result_json).unwrap();

    assert_eq!(use_request, use_back);
    assert_eq!(result, result_back);
}

#[test]
fn tool_definition_builder() {
    let tool = Tool::new(
        "tool",
        "desc",
        json!({
            "type": "object",
            "properties": {"query": {"type": "string"}},
            "required": ["query"]
        }),
    );
    assert_eq!(tool.name, "tool");
    assert_eq!(tool.description, "desc");
}

#[test]
fn app_error_debug() {
    let err = AppError::Storage("oops".to_string());
    let message = format!("{err:?}");
    assert!(message.contains("oops"));
}

#[test]
fn conversation_storage_default_path_error() {
    // We only check that the error type is returned on unsupported platforms.
    // On macOS, this should succeed, so we accept either outcome.
    let result = ConversationStorage::default_path();
    if let Err(err) = result {
        let message = format!("{err:?}");
        assert!(!message.is_empty());
    }
}
