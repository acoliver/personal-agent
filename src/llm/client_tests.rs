//! Tests for [`LlmClient`].
//!
//! Split from `client.rs`, which sits at this repo's 1000-line file cap.

use super::*;
use serdes_ai::core::messages::parts::ToolCallArgs;
use serdes_ai::core::{ModelResponse, ModelResponsePart};

#[test]
fn parse_response_includes_tool_uses_and_thinking() {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store("_test_parse_resp", "fake-key-for-test")
        .expect("store test key");

    let profile = ModelProfile {
        provider_id: "anthropic".to_string(),
        model_id: "claude-3-opus".to_string(),
        auth: AuthConfig::Keychain {
            label: "_test_parse_resp".to_string(),
        },
        ..Default::default()
    };

    let _client = LlmClient::from_profile(&profile).unwrap();

    // Clean up test key
    let _ = crate::services::secure_store::api_keys::delete("_test_parse_resp");
    let response = ModelResponse {
        parts: vec![
            ModelResponsePart::Thinking(serdes_ai::core::messages::parts::ThinkingPart::new(
                "Let me think",
            )),
            ModelResponsePart::Text(serdes_ai::core::messages::parts::TextPart::new(
                "Final answer",
            )),
            ModelResponsePart::ToolCall(
                serdes_ai::core::messages::parts::ToolCallPart::new(
                    "get_weather",
                    ToolCallArgs::json(serde_json::json!({"city": "NYC"})),
                )
                .with_tool_call_id("toolu_123"),
            ),
        ],
        ..ModelResponse::new()
    };

    let message = LlmClient::parse_response(response, &[]);

    assert_eq!(message.role, Role::Assistant);
    assert_eq!(message.content, "Final answer");
    assert_eq!(message.thinking_content, Some("Let me think".to_string()));
    assert_eq!(message.tool_uses.len(), 1);
    assert_eq!(message.tool_uses[0].name, "get_weather");
    assert_eq!(message.tool_uses[0].id, "toolu_123");
}

#[test]
fn message_builder_tracks_tool_results() {
    let message =
        Message::user("input").with_tool_results(vec![crate::llm::tools::ToolResult::success(
            "toolu_1", "ok",
        )]);

    let requests = LlmClient::build_model_requests(&[message]);
    let prompt = requests[0].user_prompts().next().unwrap();
    assert_eq!(prompt.as_text(), Some("input"));

    assert!(requests[0].parts.iter().any(|part| matches!(
        part,
        serdes_ai::core::messages::ModelRequestPart::ToolReturn(_)
    )));
}

#[test]
fn the_responses_transport_sends_no_sampling_parameters() {
    // The codex backend refuses them outright: it answered
    // "Unsupported parameter: temperature", and once that was dropped,
    // "Unsupported parameter: max_output_tokens". Either one stops the
    // turn before a single token streams.
    let mut profile = ModelProfile::new(
        "Codex".to_string(),
        "openai-codex".to_string(),
        "gpt-5.6-luna".to_string(),
        "wss://chatgpt.com/backend-api/codex/responses".to_string(),
        AuthConfig::OAuth {
            account: "chatgpt-acct".to_string(),
        },
    );
    profile.parameters.temperature = 0.7;
    profile.parameters.top_p = 0.9;
    profile.parameters.max_tokens = Some(4096);

    let client = LlmClient::from_profile(&profile).expect("client");
    let settings = client.model_settings();

    assert!(
        client.uses_open_responses(),
        "should use the Responses path"
    );
    assert_eq!(settings.temperature, None);
    assert_eq!(settings.top_p, None);
    assert_eq!(settings.max_tokens, None);
}

#[test]
fn other_providers_still_get_their_sampling_parameters() {
    let mut profile = ModelProfile::new(
        "Anthropic".to_string(),
        "anthropic".to_string(),
        "claude".to_string(),
        "https://api.anthropic.com/v1".to_string(),
        AuthConfig::None,
    );
    profile.parameters.temperature = 0.7;
    profile.parameters.max_tokens = Some(4096);

    let client = LlmClient::from_profile(&profile).expect("client");

    assert!(!client.uses_open_responses());
    assert_ne!(client.sampling(), SamplingSettings::none());
    assert_eq!(client.sampling().temperature, Some(0.7));
    assert_eq!(client.sampling().max_tokens, Some(4096));

    let settings = client.model_settings();
    assert_eq!(settings.temperature, Some(0.7));
    assert_eq!(settings.max_tokens, Some(4096));
}

#[tokio::test]
async fn build_model_wraps_non_openai_with_normalizer() {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store("_test_build_model", "test-key")
        .expect("store test key");

    let profile = ModelProfile {
        provider_id: "anthropic".to_string(),
        model_id: "claude-3-opus".to_string(),
        auth: AuthConfig::Keychain {
            label: "_test_build_model".to_string(),
        },
        parameters: crate::models::profile::ModelParameters {
            max_tokens_field_name: Some("max_completion_tokens".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let client = LlmClient::from_profile(&profile).unwrap();
    // Verify that build_model succeeds for non-OpenAI providers
    let result = client.build_model("anthropic", None).await;
    assert!(result.is_ok(), "build_model should succeed for anthropic");

    let _ = crate::services::secure_store::api_keys::delete("_test_build_model");
}

#[tokio::test]
async fn build_model_openai_uses_quirks_path() {
    crate::services::secure_store::use_mock_backend();
    crate::services::secure_store::api_keys::store("_test_build_openai", "test-key")
        .expect("store test key");

    let profile = ModelProfile {
        provider_id: "openai".to_string(),
        model_id: "gpt-4.1".to_string(),
        auth: AuthConfig::Keychain {
            label: "_test_build_openai".to_string(),
        },
        parameters: crate::models::profile::ModelParameters {
            max_tokens_field_name: Some("max_completion_tokens".to_string()),
            extra_request_fields: Some(serde_json::json!({"reasoning": {"effort": "medium"}})),
            ..Default::default()
        },
        ..Default::default()
    };

    let client = LlmClient::from_profile(&profile).unwrap();
    // Verify that build_model succeeds for OpenAI providers (uses quirks path)
    let result = client.build_model("openai", None).await;
    assert!(result.is_ok(), "build_model should succeed for openai");

    let _ = crate::services::secure_store::api_keys::delete("_test_build_openai");
}
