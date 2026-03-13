use personal_agent::{AuthConfig, LlmClient, LlmMessage, ModelProfile};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn kimi_requests_include_required_user_agent_header() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("user-agent", "RooCode/1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-test",
            "object": "chat.completion",
            "created": 1,
            "model": "kimi-for-coding",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "ok",
                        "tool_calls": null,
                        "refusal": null,
                        "reasoning_content": null
                    },
                    "finish_reason": "stop",
                    "logprobs": null
                }
            ],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 1,
                "total_tokens": 2
            },
            "system_fingerprint": null
        })))
        .mount(&mock_server)
        .await;

    let profile = ModelProfile::new(
        "Kimi".to_string(),
        "kimi-for-coding".to_string(),
        "kimi-for-coding".to_string(),
        mock_server.uri(),
        AuthConfig::Key {
            value: "sk-kimi-test".to_string(),
        },
    );

    let client = LlmClient::from_profile(&profile).expect("client");
    let response = client
        .request(&[LlmMessage::user("hello")])
        .await
        .expect("request should succeed");

    assert_eq!(response.content, "ok");
}

#[tokio::test]
async fn kimi_requests_respect_explicit_profile_base_url_override() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("user-agent", "RooCode/1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-test-explicit-base-url",
            "object": "chat.completion",
            "created": 1,
            "model": "kimi-for-coding",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "override ok",
                        "tool_calls": null,
                        "refusal": null,
                        "reasoning_content": null
                    },
                    "finish_reason": "stop",
                    "logprobs": null
                }
            ],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 1,
                "total_tokens": 2
            },
            "system_fingerprint": null
        })))
        .mount(&mock_server)
        .await;

    let profile = ModelProfile::new(
        "Kimi".to_string(),
        "kimi-for-coding".to_string(),
        "kimi-for-coding".to_string(),
        mock_server.uri(),
        AuthConfig::Key {
            value: "sk-kimi-test".to_string(),
        },
    );

    let client = LlmClient::from_profile(&profile).expect("client");
    let response = client
        .request(&[LlmMessage::user("hello")])
        .await
        .expect("request should succeed with explicit base url");

    assert_eq!(response.content, "override ok");
}
