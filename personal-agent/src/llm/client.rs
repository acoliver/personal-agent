//! LLM client implementation

use crate::models::{AuthConfig, Conversation, Message, MessageRole, ModelProfile};
use super::error::{LlmError, LlmResult};

/// LLM client wrapping `SerdesAI` Agent
///
/// This struct provides a bridge between `PersonalAgent`'s data structures
/// and the `SerdesAI` library.
#[derive(Debug, Clone)]
pub struct LLMClient {
    profile: ModelProfile,
}

impl LLMClient {
    /// Create a new LLM client from a model profile
    ///
    /// # Errors
    ///
    /// Returns an error if the profile configuration is invalid or the provider is unsupported.
    pub fn new(profile: ModelProfile) -> LlmResult<Self> {
        // Validate provider
        validate_provider(&profile.provider_id)?;
        
        // Validate auth
        validate_auth(&profile.auth)?;

        Ok(Self { profile })
    }

    /// Get the model profile
    #[must_use]
    pub const fn profile(&self) -> &ModelProfile {
        &self.profile
    }

    /// Get the API key from the profile
    ///
    /// # Errors
    ///
    /// Returns an error if the auth config is not a key type or keyfile cannot be read.
    pub fn get_api_key(&self) -> LlmResult<String> {
        match &self.profile.auth {
            AuthConfig::Key { value } => {
                if value.is_empty() {
                    Err(LlmError::Auth("API key is empty".to_string()))
                } else {
                    Ok(value.clone())
                }
            }
            AuthConfig::Keyfile { path } => {
                std::fs::read_to_string(path)
                    .map(|s| s.trim().to_string())
                    .map_err(|e| LlmError::Auth(format!("Failed to read keyfile: {e}")))
            }
        }
    }

    /// Build the model specification string for `SerdesAI`
    ///
    /// Format: "`provider:model_id`"
    #[must_use]
    pub fn model_spec(&self) -> String {
        format!("{}:{}", self.profile.provider_id, self.profile.model_id)
    }
}

/// Convert a `Conversation` to `SerdesAI` `ModelRequest` messages
///
/// Convert a `Conversation` to a `SerdesAI` `ModelRequest` with all messages as parts.
///
/// This builds a single request with proper message alternation for multi-turn conversations.
#[must_use]
#[allow(dead_code)]
pub fn conversation_to_request(conversation: &Conversation) -> serdes_ai::ModelRequest {
    use serdes_ai::ModelRequest;
    
    let parts: Vec<_> = conversation
        .messages
        .iter()
        .map(message_to_request_part)
        .collect();
    
    ModelRequest::with_parts(parts)
}

/// Convert a single `Message` to a `SerdesAI` `ModelRequestPart`
#[allow(dead_code)]
fn message_to_request_part(message: &Message) -> serdes_ai::ModelRequestPart {
    use serdes_ai::core::messages::{
        ModelRequestPart, ModelResponse, ModelResponsePart,
        SystemPromptPart, UserPromptPart, TextPart,
    };
    
    match message.role {
        MessageRole::System => {
            ModelRequestPart::SystemPrompt(SystemPromptPart::new(message.content.clone()))
        }
        MessageRole::User => {
            ModelRequestPart::UserPrompt(UserPromptPart::new(message.content.clone()))
        }
        MessageRole::Assistant => {
            // For assistant messages, wrap in ModelResponse for proper multi-turn conversation
            let mut response = ModelResponse::new();
            response.add_part(ModelResponsePart::Text(TextPart::new(message.content.clone())));
            ModelRequestPart::ModelResponse(Box::new(response))
        }
    }
}

/// Validate that the provider is supported
fn validate_provider(provider_id: &str) -> LlmResult<()> {
    match provider_id.to_lowercase().as_str() {
        "openai" | "anthropic" | "gemini" | "groq" | "mistral" | "ollama" | "bedrock" => Ok(()),
        _ => Err(LlmError::UnsupportedModel(format!(
            "Provider '{provider_id}' is not supported"
        ))),
    }
}

/// Validate authentication configuration
fn validate_auth(auth: &AuthConfig) -> LlmResult<()> {
    match auth {
        AuthConfig::Key { value } => {
            if value.is_empty() {
                Err(LlmError::Auth("API key cannot be empty".to_string()))
            } else {
                Ok(())
            }
        }
        AuthConfig::Keyfile { path } => {
            if std::path::Path::new(path).exists() {
                Ok(())
            } else {
                Err(LlmError::Auth(format!("Keyfile not found: {path}")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn test_profile() -> ModelProfile {
        ModelProfile::new(
            "Test".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            "https://api.openai.com/v1".to_string(),
            AuthConfig::Key {
                value: "test-key".to_string(),
            },
        )
    }

    #[test]
    fn test_new_client() {
        let profile = test_profile();
        let client = LLMClient::new(profile.clone());
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.profile(), &profile);
    }

    #[test]
    fn test_unsupported_provider() {
        let mut profile = test_profile();
        profile.provider_id = "unsupported".to_string();
        let result = LLMClient::new(profile);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LlmError::UnsupportedModel(_)));
    }

    #[test]
    fn test_empty_api_key() {
        let mut profile = test_profile();
        profile.auth = AuthConfig::Key {
            value: String::new(),
        };
        let result = LLMClient::new(profile);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LlmError::Auth(_)));
    }

    #[test]
    fn test_get_api_key() {
        let profile = test_profile();
        let client = LLMClient::new(profile).unwrap();
        let key = client.get_api_key().unwrap();
        assert_eq!(key, "test-key");
    }

    #[test]
    fn test_model_spec() {
        let profile = test_profile();
        let client = LLMClient::new(profile).unwrap();
        assert_eq!(client.model_spec(), "openai:gpt-4");
    }

    #[test]
    fn test_conversation_to_request() {
        let mut conversation = Conversation::new(Uuid::new_v4());
        conversation.add_message(Message::user("Hello".to_string()));
        conversation.add_message(Message::assistant("Hi there".to_string()));

        let request = conversation_to_request(&conversation);
        // Request should contain parts for both messages
        assert_eq!(request.parts.len(), 2);
    }

    #[test]
    fn test_message_to_request_part_user() {
        let message = Message::user("Hello".to_string());
        let part = message_to_request_part(&message);
        // Verify it's a UserPrompt part
        assert!(matches!(part, serdes_ai::ModelRequestPart::UserPrompt(_)));
    }

    #[test]
    fn test_message_to_request_part_assistant() {
        let message = Message::assistant("Hi there".to_string());
        let part = message_to_request_part(&message);
        // Verify it's a ModelResponse part (not UserPrompt - that was the bug!)
        assert!(matches!(part, serdes_ai::ModelRequestPart::ModelResponse(_)));
    }

    #[test]
    fn test_message_to_request_part_system() {
        let message = Message::system("You are helpful".to_string());
        let part = message_to_request_part(&message);
        // Verify it's a SystemPrompt part
        assert!(matches!(part, serdes_ai::ModelRequestPart::SystemPrompt(_)));
    }

    #[test]
    fn test_validate_provider_openai() {
        assert!(validate_provider("openai").is_ok());
        assert!(validate_provider("OpenAI").is_ok());
    }

    #[test]
    fn test_validate_provider_anthropic() {
        assert!(validate_provider("anthropic").is_ok());
        assert!(validate_provider("Anthropic").is_ok());
    }

    #[test]
    fn test_validate_provider_invalid() {
        assert!(validate_provider("invalid").is_err());
    }

    #[test]
    fn test_validate_auth_key() {
        let auth = AuthConfig::Key {
            value: "test-key".to_string(),
        };
        assert!(validate_auth(&auth).is_ok());
    }

    #[test]
    fn test_validate_auth_empty_key() {
        let auth = AuthConfig::Key {
            value: String::new(),
        };
        assert!(validate_auth(&auth).is_err());
    }

    #[test]
    fn test_validate_auth_nonexistent_keyfile() {
        let auth = AuthConfig::Keyfile {
            path: "/nonexistent/path/to/key".to_string(),
        };
        assert!(validate_auth(&auth).is_err());
    }
}
