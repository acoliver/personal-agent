//! Live E2E tests against the real Kimi coding API.
//!
//! These tests are `#[ignore]` so they don't run in normal CI.
//! To run them locally:
//!
//! ```sh
//! PA_E2E_API_KEY=<your-kimi-key> cargo test --test kimi_e2e_live_tests -- --ignored
//! ```
//!
//! They diagnose whether the User-Agent header and other configuration
//! actually reaches the Kimi API and produces a valid response.

use personal_agent::{AuthConfig, LlmClient, LlmMessage, ModelProfile, StreamEvent};

const KIMI_BASE_URL: &str = "https://api.kimi.com/coding/v1";
const KIMI_MODEL: &str = "kimi-k2-0711-preview";

fn require_api_key() -> String {
    std::env::var("PA_E2E_API_KEY")
        .or_else(|_| std::env::var("KIMI_API_KEY"))
        .expect(
            "Set PA_E2E_API_KEY or KIMI_API_KEY to run live Kimi tests\n\
             Example: PA_E2E_API_KEY=sk-... cargo test --test kimi_e2e_live_tests -- --ignored",
        )
}

fn kimi_profile(api_key_label: &str) -> ModelProfile {
    ModelProfile::new(
        "Kimi-E2E".to_string(),
        "kimi-for-coding".to_string(),
        KIMI_MODEL.to_string(),
        String::new(), // use quirks base_url
        AuthConfig::Keychain {
            label: api_key_label.to_string(),
        },
    )
}

/// Raw reqwest test — bypasses all serdes-ai / `LlmClient` layers.
/// This tells us if the Kimi API itself is reachable and accepts
/// our User-Agent header when sent via reqwest's `default_headers`.
#[tokio::test]
#[ignore = "requires PA_E2E_API_KEY or KIMI_API_KEY env var"]
async fn raw_reqwest_kimi_with_user_agent_header() {
    let api_key = require_api_key();

    // Build an HTTP client identical to what build_openai_model_with_quirks does
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static("RooCode/1.0"),
    );

    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .expect("client");

    let body = serde_json::json!({
        "model": KIMI_MODEL,
        "messages": [
            {"role": "user", "content": "Say exactly: pong"}
        ],
        "max_tokens": 32,
        "temperature": 0.0
    });

    let response = client
        .post(format!("{KIMI_BASE_URL}/chat/completions"))
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .expect("HTTP request should send");

    let status = response.status();
    let response_body = response.text().await.unwrap_or_default();

    println!("=== Raw reqwest Kimi response ===");
    println!("Status: {status}");
    println!("Body: {response_body}");

    assert!(
        status.is_success(),
        "Kimi API returned {status}: {response_body}"
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&response_body).expect("response should be valid JSON");
    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");
    println!("Content: {content}");
    assert!(!content.is_empty(), "response content should not be empty");
}

/// Same raw reqwest test but WITHOUT the User-Agent header.
/// If this fails with a "coding agents only" error, it proves the header is required.
/// If this succeeds, the header may not be the issue.
#[tokio::test]
#[ignore = "requires PA_E2E_API_KEY or KIMI_API_KEY env var"]
async fn raw_reqwest_kimi_without_user_agent_header() {
    let api_key = require_api_key();

    // Plain client with no custom User-Agent
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": KIMI_MODEL,
        "messages": [
            {"role": "user", "content": "Say exactly: pong"}
        ],
        "max_tokens": 32,
        "temperature": 0.0
    });

    let response = client
        .post(format!("{KIMI_BASE_URL}/chat/completions"))
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .expect("HTTP request should send");

    let status = response.status();
    let response_body = response.text().await.unwrap_or_default();

    println!("=== Raw reqwest Kimi (NO User-Agent) response ===");
    println!("Status: {status}");
    println!("Body: {response_body}");

    // We EXPECT this to fail if the User-Agent is truly required.
    // If it succeeds, the problem is elsewhere.
    if status.is_success() {
        println!("WARNING: Kimi accepted the request WITHOUT User-Agent header.");
        println!("This means User-Agent is NOT the gate — look elsewhere.");
    } else {
        println!("CONFIRMED: Kimi rejected the request without User-Agent header.");
        println!("The User-Agent header IS required.");
    }
}

/// Full `LlmClient` E2E test — exercises the actual code path the UI uses.
#[tokio::test]
#[ignore = "requires PA_E2E_API_KEY or KIMI_API_KEY env var"]
async fn llm_client_kimi_non_streaming_e2e() {
    let api_key = require_api_key();

    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_e2e_kimi", &api_key)
        .expect("store key");

    let profile = kimi_profile("_e2e_kimi");
    let client = LlmClient::from_profile(&profile).expect("client");

    println!("=== LlmClient non-streaming Kimi E2E ===");
    println!("model_spec: {}", client.model_spec());

    let result = client
        .request(&[LlmMessage::user("Say exactly: pong")])
        .await;

    match &result {
        Ok(msg) => {
            println!("SUCCESS: {}", msg.content);
            assert!(!msg.content.is_empty());
        }
        Err(e) => {
            println!("ERROR: {e}");
            panic!("LlmClient Kimi request failed: {e}");
        }
    }

    let _ = personal_agent::services::secure_store::api_keys::delete("_e2e_kimi");
}

/// Full `LlmClient` streaming E2E test.
#[tokio::test]
#[ignore = "requires PA_E2E_API_KEY or KIMI_API_KEY env var"]
async fn llm_client_kimi_streaming_e2e() {
    let api_key = require_api_key();

    personal_agent::services::secure_store::use_mock_backend();
    personal_agent::services::secure_store::api_keys::store("_e2e_kimi_stream", &api_key)
        .expect("store key");

    let profile = kimi_profile("_e2e_kimi_stream");
    let client = LlmClient::from_profile(&profile).expect("client");

    println!("=== LlmClient streaming Kimi E2E ===");

    let mut collected_text = String::new();
    let mut saw_complete = false;
    let mut saw_error: Option<String> = None;

    let result = client
        .request_stream(
            &[LlmMessage::user("Say exactly: pong")],
            |event| match event {
                StreamEvent::TextDelta(text) => {
                    print!("{text}");
                    collected_text.push_str(&text);
                }
                StreamEvent::ThinkingDelta(text) => {
                    print!("[think: {text}]");
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

    if let Some(ref err) = saw_error {
        println!("Stream error event: {err}");
    }

    match &result {
        Ok(()) => {
            println!("Collected text: {collected_text}");
            assert!(saw_complete, "should have received Complete event");
            assert!(!collected_text.is_empty(), "should have received text");
        }
        Err(e) => {
            println!("Stream result error: {e}");
            panic!("LlmClient Kimi streaming request failed: {e}");
        }
    }

    let _ = personal_agent::services::secure_store::api_keys::delete("_e2e_kimi_stream");
}
