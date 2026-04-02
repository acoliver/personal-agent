//! Tests that exercise the EXACT code path the UI uses for Kimi:
//! `LlmClient::create_agent()` → `LlmClient::run_agent_stream()`.
//!
//! The wiremock test verifies the User-Agent header actually gets sent.
//! The live test hits the real Kimi API to confirm end-to-end.

use personal_agent::llm::AgentClientExt;
use personal_agent::{AuthConfig, LlmClient, LlmMessage, ModelProfile, StreamEvent};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn kimi_profile(base_url: String, key_label: &str) -> ModelProfile {
    ModelProfile::new(
        "Kimi-Agent-Test".to_string(),
        "kimi-for-coding".to_string(),
        "kimi-for-coding".to_string(),
        base_url,
        AuthConfig::Keychain {
            label: key_label.to_string(),
        },
    )
}

/// SSE streaming response that the `OpenAI` streaming parser expects.
fn sse_streaming_response(content: &str) -> String {
    let chunk = serde_json::json!({
        "id": "chatcmpl-agent-test",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": "kimi-for-coding",
        "choices": [{
            "index": 0,
            "delta": {
                "role": "assistant",
                "content": content
            },
            "finish_reason": null
        }]
    });
    let done_chunk = serde_json::json!({
        "id": "chatcmpl-agent-test",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": "kimi-for-coding",
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 1,
            "completion_tokens": 1,
            "total_tokens": 2
        }
    });
    format!("data: {chunk}\n\ndata: {done_chunk}\n\ndata: [DONE]\n\n")
}

/// Wiremock test: the AGENT path must send `User-Agent: RooCode/1.0`.
///
/// This exercises: `create_agent()` → `build_agent_model()` → `build_model()` →
/// `build_openai_model_with_quirks()` → `NormalizingSseModel::request_stream()`.
#[tokio::test]
async fn agent_path_sends_user_agent_header_to_kimi() {
    let mock_server = MockServer::start().await;

    // Mock that REQUIRES User-Agent header — rejects without it
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("user-agent", "RooCode/1.0"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_streaming_response("pong")),
        )
        .mount(&mock_server)
        .await;

    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_test_agent_ua", "sk-kimi-test")
        .expect("store test key");

    let profile = kimi_profile(mock_server.uri(), "_test_agent_ua");
    let client = LlmClient::from_profile(&profile).expect("client");

    // This is THE code path the UI uses
    let agent = client
        .create_agent(vec![], "You are a test assistant.")
        .await
        .expect("create_agent should succeed");

    let mut collected_text = String::new();
    let mut saw_complete = false;
    let mut saw_error: Option<String> = None;

    let result = client
        .run_agent_stream(&agent, &[LlmMessage::user("ping")], |event| match event {
            StreamEvent::TextDelta(text) => {
                collected_text.push_str(&text);
            }
            StreamEvent::Complete => {
                saw_complete = true;
            }
            StreamEvent::Error(e) => {
                saw_error = Some(e);
            }
            _ => {}
        })
        .await;

    // Also check received requests to verify the header
    let received = mock_server.received_requests().await.unwrap_or_default();
    println!("Received {} request(s) to mock server", received.len());
    for (i, req) in received.iter().enumerate() {
        let ua = req
            .headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("MISSING");
        println!("Request {i}: User-Agent = {ua}");
    }

    if let Some(ref err) = saw_error {
        eprintln!("Agent stream error event: {err}");
    }
    if let Err(ref e) = result {
        eprintln!("Agent stream result error: {e}");
    }

    // The key assertion: the request reached wiremock (meaning User-Agent matched)
    assert!(
        !received.is_empty(),
        "mock server should have received at least one request"
    );
    assert!(
        result.is_ok(),
        "agent stream should succeed (User-Agent header must have been sent). Error: {:?}",
        result.err()
    );
    assert_eq!(collected_text, "pong", "agent should have received 'pong'");
    assert!(saw_complete, "agent should have completed");

    let _ = personal_agent::services::secure_store::api_keys::delete("_test_agent_ua");
}

/// Negative test: wiremock requires `User-Agent` but profile uses "openai" provider
/// (which has no quirks headers). The mock should NOT be hit, and
/// the request should fail or go unmatched.
#[tokio::test]
async fn agent_path_without_kimi_provider_misses_user_agent() {
    let mock_server = MockServer::start().await;

    // Mock requires User-Agent: RooCode/1.0
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("user-agent", "RooCode/1.0"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse_streaming_response("should-not-get-this")),
        )
        .expect(0) // should NOT be matched
        .mount(&mock_server)
        .await;

    // Fallback mock: accept anything, return 403 like Kimi would
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "error": {
                "message": "Kimi For Coding is currently only available for Coding Agents",
                "type": "access_terminated_error"
            }
        })))
        .mount(&mock_server)
        .await;

    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_test_agent_no_ua", "sk-test")
        .expect("store test key");

    // Use "openai" provider_id — NOT "kimi-for-coding"
    let profile = ModelProfile::new(
        "NotKimi".to_string(),
        "openai".to_string(),
        "some-model".to_string(),
        mock_server.uri(),
        AuthConfig::Keychain {
            label: "_test_agent_no_ua".to_string(),
        },
    );

    let client = LlmClient::from_profile(&profile).expect("client");
    let agent = client
        .create_agent(vec![], "test")
        .await
        .expect("create_agent");

    let mut saw_error = false;
    let _ = client
        .run_agent_stream(&agent, &[LlmMessage::user("hi")], |event| {
            if let StreamEvent::Error(_) = event {
                saw_error = true;
            }
        })
        .await;

    // Without the kimi-for-coding provider_id, User-Agent wasn't sent,
    // so the request hit the 403 fallback mock
    let received = mock_server.received_requests().await.unwrap_or_default();
    for (i, req) in received.iter().enumerate() {
        let ua = req
            .headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("MISSING");
        println!("Request {i}: User-Agent = {ua}");
        assert_ne!(
            ua, "RooCode/1.0",
            "openai provider should NOT send RooCode/1.0 User-Agent"
        );
    }

    let _ = personal_agent::services::secure_store::api_keys::delete("_test_agent_no_ua");
}

/// Wiremock test: SSE normalization handles bare `data:` prefix (no space).
///
/// Kimi sends `data:{json}` instead of `data: {json}`.  The
/// `NormalizingSseModel` wrapper normalizes this before the parser sees it.
#[tokio::test]
async fn agent_path_normalizes_bare_data_prefix_in_sse() {
    let mock_server = MockServer::start().await;

    // Build SSE with bare `data:` (no space) — this is what Kimi actually sends
    let chunk = serde_json::json!({
        "id": "chatcmpl-bare",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": "kimi-for-coding",
        "choices": [{
            "index": 0,
            "delta": {"role": "assistant", "content": "normalized"},
            "finish_reason": null
        }]
    });
    let done_chunk = serde_json::json!({
        "id": "chatcmpl-bare",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": "kimi-for-coding",
        "choices": [{
            "index": 0,
            "delta": {},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
    });
    // NOTE: bare `data:` without space — this is the Kimi SSE bug
    let bare_sse = format!(
        "data:{chunk}

data:{done_chunk}

data: [DONE]

"
    );

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("user-agent", "RooCode/1.0"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(bare_sse),
        )
        .mount(&mock_server)
        .await;

    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_test_bare_sse", "sk-kimi-test")
        .expect("store test key");

    let profile = kimi_profile(mock_server.uri(), "_test_bare_sse");
    let client = LlmClient::from_profile(&profile).expect("client");

    let agent = client
        .create_agent(vec![], "test")
        .await
        .expect("create_agent");

    let mut collected_text = String::new();
    let mut saw_complete = false;

    let result = client
        .run_agent_stream(&agent, &[LlmMessage::user("hi")], |event| match event {
            StreamEvent::TextDelta(text) => collected_text.push_str(&text),
            StreamEvent::Complete => saw_complete = true,
            _ => {}
        })
        .await;

    assert!(
        result.is_ok(),
        "should succeed with bare data: SSE format: {:?}",
        result.err()
    );
    assert_eq!(
        collected_text, "normalized",
        "SSE normalization should have parsed the bare data: chunks"
    );
    assert!(saw_complete, "stream should complete");

    let _ = personal_agent::services::secure_store::api_keys::delete("_test_bare_sse");
}

/// Live E2E: agent path against real Kimi API.
/// This is what the UI actually does.
#[tokio::test]
#[ignore = "requires PA_E2E_API_KEY or KIMI_API_KEY env var"]
async fn agent_path_kimi_live_e2e() {
    let api_key = std::env::var("PA_E2E_API_KEY")
        .or_else(|_| std::env::var("KIMI_API_KEY"))
        .expect("Set PA_E2E_API_KEY or KIMI_API_KEY");

    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_e2e_kimi_agent", &api_key)
        .expect("store key");

    let profile = ModelProfile::new(
        "Kimi-Agent-E2E".to_string(),
        "kimi-for-coding".to_string(),
        "kimi-k2-0711-preview".to_string(),
        String::new(), // use quirks base_url
        AuthConfig::Keychain {
            label: "_e2e_kimi_agent".to_string(),
        },
    );

    let client = LlmClient::from_profile(&profile).expect("client");

    println!("=== Agent path Kimi live E2E ===");
    println!("Creating agent...");

    let agent = client
        .create_agent(vec![], "You are a test assistant. Be brief.")
        .await
        .expect("create_agent");

    println!("Running agent stream...");

    let mut collected_text = String::new();
    let mut collected_thinking = String::new();
    let mut saw_complete = false;
    let mut saw_error: Option<String> = None;

    let result = client
        .run_agent_stream(
            &agent,
            &[LlmMessage::user("Say exactly: pong")],
            |event| match event {
                StreamEvent::TextDelta(text) => {
                    print!("{text}");
                    collected_text.push_str(&text);
                }
                StreamEvent::ThinkingDelta(text) => {
                    collected_thinking.push_str(&text);
                }
                StreamEvent::Complete => {
                    println!("\n[COMPLETE]");
                    saw_complete = true;
                }
                StreamEvent::Error(e) => {
                    println!("\n[ERROR: {e}]");
                    saw_error = Some(e);
                }
                _ => {}
            },
        )
        .await;

    println!("Collected text: '{collected_text}'");
    println!("Collected thinking: '{collected_thinking}'");

    if let Some(ref err) = saw_error {
        eprintln!("Agent error: {err}");
        assert!(
            !err.contains("Coding Agents") && !err.contains("coding agents"),
            "CONFIRMED BUG: Agent path is NOT sending User-Agent header!\nError: {err}"
        );
    }

    match result {
        Ok(()) => {
            // Kimi K2 may put content in thinking rather than text
            let has_output = !collected_text.is_empty() || !collected_thinking.is_empty();
            assert!(
                has_output,
                "should have received some output (text or thinking)"
            );
        }
        Err(ref e) => {
            let err_str = e.to_string();
            assert!(
                !err_str.contains("Coding Agents") && !err_str.contains("coding agents"),
                "CONFIRMED BUG: Agent path is NOT sending User-Agent header!\nError: {err_str}"
            );
            panic!("Agent stream failed: {err_str}");
        }
    }

    let _ = personal_agent::services::secure_store::api_keys::delete("_e2e_kimi_agent");
}
