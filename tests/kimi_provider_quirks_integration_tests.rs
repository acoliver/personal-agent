use personal_agent::{AuthConfig, LlmClient, LlmMessage, ModelProfile, StreamEvent};
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

    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_test_kimi_ua", "sk-kimi-test")
        .expect("store test key");

    let profile = ModelProfile::new(
        "Kimi".to_string(),
        "kimi-for-coding".to_string(),
        "kimi-for-coding".to_string(),
        mock_server.uri(),
        AuthConfig::Keychain {
            label: "_test_kimi_ua".to_string(),
        },
    );

    let client = LlmClient::from_profile(&profile).expect("client");
    let response = client
        .request(&[LlmMessage::user("hello")])
        .await
        .expect("request should succeed");

    assert_eq!(response.content, "ok");

    let _ = personal_agent::services::secure_store::api_keys::delete("_test_kimi_ua");
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

    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_test_kimi_base", "sk-kimi-test")
        .expect("store test key");

    let profile = ModelProfile::new(
        "Kimi".to_string(),
        "kimi-for-coding".to_string(),
        "kimi-for-coding".to_string(),
        mock_server.uri(),
        AuthConfig::Keychain {
            label: "_test_kimi_base".to_string(),
        },
    );

    let client = LlmClient::from_profile(&profile).expect("client");
    let response = client
        .request(&[LlmMessage::user("hello")])
        .await
        .expect("request should succeed with explicit base url");

    assert_eq!(response.content, "override ok");

    let _ = personal_agent::services::secure_store::api_keys::delete("_test_kimi_base");
}

#[tokio::test]
async fn kimi_streaming_requests_include_required_user_agent_header() {
    let mock_server = MockServer::start().await;

    // Streaming responses use SSE (Server-Sent Events) format
    let chunk1 = serde_json::json!({"id":"chatcmpl-stream","object":"chat.completion.chunk","created":1,"model":"kimi-for-coding","choices":[{"index":0,"delta":{"role":"assistant","content":"stream"},"finish_reason":null}]});
    let chunk2 = serde_json::json!({"id":"chatcmpl-stream","object":"chat.completion.chunk","created":1,"model":"kimi-for-coding","choices":[{"index":0,"delta":{"content":" ok"},"finish_reason":null}]});
    let chunk3 = serde_json::json!({"id":"chatcmpl-stream","object":"chat.completion.chunk","created":1,"model":"kimi-for-coding","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]});
    let sse_body = format!(
        "data: {chunk1}

data: {chunk2}

data: {chunk3}

data: [DONE]

"
    );

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("user-agent", "RooCode/1.0"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_body),
        )
        .mount(&mock_server)
        .await;

    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_test_kimi_stream", "sk-kimi-test")
        .expect("store test key");

    let profile = ModelProfile::new(
        "Kimi".to_string(),
        "kimi-for-coding".to_string(),
        "kimi-for-coding".to_string(),
        mock_server.uri(),
        AuthConfig::Keychain {
            label: "_test_kimi_stream".to_string(),
        },
    );

    let client = LlmClient::from_profile(&profile).expect("client");

    let mut collected_text = String::new();
    let result = client
        .request_stream(&[LlmMessage::user("hello")], |event| {
            if let StreamEvent::TextDelta(text) = event {
                collected_text.push_str(&text);
            }
        })
        .await;

    assert!(
        result.is_ok(),
        "streaming request should succeed (User-Agent matched); got: {:?}",
        result.err()
    );
    assert_eq!(collected_text, "stream ok");

    let _ = personal_agent::services::secure_store::api_keys::delete("_test_kimi_stream");
}

#[tokio::test]
async fn kimi_without_user_agent_is_rejected_by_mock() {
    let mock_server = MockServer::start().await;

    // This mock REQUIRES the User-Agent header — requests without it get a 404 from wiremock
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("user-agent", "RooCode/1.0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "chatcmpl-ua-check",
            "object": "chat.completion",
            "created": 1,
            "model": "kimi-for-coding",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "agent verified"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
        })))
        .mount(&mock_server)
        .await;

    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_test_kimi_ua_check", "sk-kimi-test")
        .expect("store test key");

    // Use a provider WITHOUT custom headers — the request should fail because
    // wiremock won't match without the User-Agent header
    let profile = ModelProfile::new(
        "NoHeaders".to_string(),
        "openai".to_string(), // Not kimi-for-coding — no quirks headers
        "kimi-for-coding".to_string(),
        mock_server.uri(),
        AuthConfig::Keychain {
            label: "_test_kimi_ua_check".to_string(),
        },
    );

    let client = LlmClient::from_profile(&profile).expect("client");
    let result = client.request(&[LlmMessage::user("hello")]).await;

    // Should fail because wiremock returns 404 when User-Agent doesn't match
    assert!(
        result.is_err(),
        "request without User-Agent should be rejected by the strict mock"
    );

    let _ = personal_agent::services::secure_store::api_keys::delete("_test_kimi_ua_check");
}
