use personal_agent::{AuthConfig, LlmClient, LlmMessage, ModelProfile};
use std::sync::Arc;

/// Tests that request_stream_with_tools returns an error when the API call fails
/// (e.g., invalid API key). This verifies the error propagation behavior.
#[tokio::test]
async fn request_stream_returns_error_on_api_failure() {
    let profile = ModelProfile::new(
        "Test".to_string(),
        "openai".to_string(),
        "gpt-4o".to_string(),
        String::new(),
        AuthConfig::Key {
            value: "invalid-key-for-testing".to_string(),
        },
    );

    let client = LlmClient::from_profile(&profile).unwrap();

    let messages = vec![LlmMessage::user("Hello")];
    let events: Arc<std::sync::Mutex<Vec<personal_agent::StreamEvent>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    let result = client
        .request_stream_with_tools(&messages, &[], move |event| {
            events_clone.lock().unwrap().push(event);
        })
        .await;

    // The request should fail due to invalid API key
    assert!(result.is_err(), "Expected error from invalid API key");
}
