//! Streaming functionality for LLM responses

use super::client::LLMClient;
use super::error::{LlmError, LlmResult};
use super::events::ChatStreamEvent;
use crate::models::{AuthConfig, Conversation, MessageRole};
use futures::stream::{Stream, StreamExt};
use serdes_ai::agent::{AgentBuilder, AgentStreamEvent, ModelConfig, RunOptions};
use serdes_ai::core::messages::{ModelRequest, ModelRequestPart, ModelResponse, UserPromptPart};
use std::pin::Pin;

/// Convert our conversation history to `SerdesAI`'s `ModelRequest` format
fn conversation_to_model_requests(conversation: &Conversation) -> Vec<ModelRequest> {
    conversation
        .messages
        .iter()
        .filter(|m| m.role != MessageRole::System) // System is handled separately
        .map(|msg| {
            match msg.role {
                MessageRole::User => {
                    let part = ModelRequestPart::UserPrompt(UserPromptPart::new(msg.content.clone()));
                    ModelRequest::with_parts(vec![part])
                }
                MessageRole::Assistant => {
                    let response = ModelResponse::text(&msg.content);
                    let part = ModelRequestPart::ModelResponse(Box::new(response));
                    ModelRequest::with_parts(vec![part])
                }
                MessageRole::System => unreachable!(), // Filtered above
            }
        })
        .collect()
}

/// Send a message and get a stream of response events
///
/// This function:
/// 1. Creates an agent using the model spec string (e.g., "openai:gpt-4o")
/// 2. Converts the conversation history to `SerdesAI` format
/// 3. Streams the response back as `ChatStreamEvent`s
///
/// # Arguments
///
/// * `client` - The LLM client to use
/// * `conversation` - The conversation history for context
/// * `user_message` - The new user message to send
///
/// # Returns
///
/// A stream of `ChatStreamEvent` events
///
/// # Errors
///
/// Returns an error if the agent cannot be created or the stream fails.
pub async fn send_message_stream(
    client: &LLMClient,
    conversation: &Conversation,
    user_message: String,
) -> LlmResult<Pin<Box<dyn Stream<Item = ChatStreamEvent> + Send>>> {
    let profile = client.profile();
    
    // Get API key
    let api_key = match &profile.auth {
        AuthConfig::Key { value } => value.clone(),
        AuthConfig::Keyfile { path } => {
            std::fs::read_to_string(path)
                .map_err(|e| LlmError::Auth(format!("Failed to read keyfile: {e}")))?
                .trim()
                .to_string()
        }
    };
    
    // Build model spec string (e.g., "openai:gpt-4o")
    let model_spec = client.model_spec();
    
    // Create ModelConfig with our API key
    let config = ModelConfig::new(&model_spec).with_api_key(&api_key);
    
    // Build system prompt from conversation if there's a system message
    let system_prompt = conversation
        .messages
        .iter()
        .find(|m| m.role == MessageRole::System)
        .map(|m| m.content.clone());
    
    // Create the agent using our new from_config method
    let mut builder: AgentBuilder<(), String> = AgentBuilder::from_config(config)
        .map_err(|e| LlmError::SerdesAi(format!("Failed to create model: {e}")))?;
    
    if let Some(prompt) = system_prompt {
        builder = builder.system_prompt(prompt);
    }
    
    // Apply model parameters
    let params = &profile.parameters;
    builder = builder
        .temperature(params.temperature)
        .max_tokens(u64::from(params.max_tokens));
    
    let agent = builder.build();
    
    // Convert conversation history to ModelRequest format
    let message_history = conversation_to_model_requests(conversation);
    
    // Create run options with message history
    let options = RunOptions::new().message_history(message_history);
    
    // Start streaming with conversation history
    let stream = agent
        .run_stream_with_options(user_message, (), options)
        .await
        .map_err(|e| LlmError::SerdesAi(e.to_string()))?;
    
    // Convert SerdesAI stream events to our ChatStreamEvent
    let mapped_stream = stream.map(|event| match event {
        Ok(AgentStreamEvent::TextDelta { text }) => {
            ChatStreamEvent::text(text)
        }
        Ok(AgentStreamEvent::ThinkingDelta { text }) => {
            ChatStreamEvent::thinking(text)
        }
        Ok(AgentStreamEvent::RunComplete { .. }) => {
            ChatStreamEvent::complete(None, None)
        }
        Ok(AgentStreamEvent::Error { message }) => {
            ChatStreamEvent::error(message, true)
        }
        Err(e) => {
            ChatStreamEvent::error(e.to_string(), false)
        }
        _ => {
            // Other events (tool calls, etc.) - emit empty text delta for now
            ChatStreamEvent::text(String::new())
        }
    });
    
    Ok(Box::pin(mapped_stream))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Message, ModelProfile};
    use uuid::Uuid;

    fn test_profile() -> ModelProfile {
        ModelProfile::new(
            "Test".to_string(),
            "openai".to_string(),
            "gpt-4".to_string(),
            "https://api.openai.com/v1".to_string(),
            AuthConfig::Key {
                value: "test-key-123".to_string(),
            },
        )
    }

    #[test]
    fn test_model_spec_format() {
        let profile = test_profile();
        let client = LLMClient::new(profile).unwrap();
        assert_eq!(client.model_spec(), "openai:gpt-4");
    }

    #[test]
    fn test_conversation_to_model_requests_empty() {
        let conv = Conversation::new(Uuid::new_v4());
        let requests = conversation_to_model_requests(&conv);
        assert!(requests.is_empty());
    }

    #[test]
    fn test_conversation_to_model_requests_filters_system() {
        let mut conv = Conversation::new(Uuid::new_v4());
        conv.add_message(Message::system("You are helpful.".to_string()));
        conv.add_message(Message::user("Hello".to_string()));
        conv.add_message(Message::assistant("Hi there!".to_string()));
        
        let requests = conversation_to_model_requests(&conv);
        // Should have 2 requests (user + assistant), not 3 (system filtered)
        assert_eq!(requests.len(), 2);
    }
}
