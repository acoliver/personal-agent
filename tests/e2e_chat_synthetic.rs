//! E2E test using PA_E2E_* environment-backed profile configuration.
//!
//! This test hits the actual API - run with:
//!   cargo test --test `e2e_chat_synthetic` -- --ignored --nocapture
//!
//! Requires:
//! - PA_E2E_PROVIDER_ID (optional; default: ollama)
//! - PA_E2E_MODEL_ID (optional; default: minimax-m2.7:cloud)
//! - PA_E2E_BASE_URL (optional; default: https://ollama.com/v1)
//! - PA_E2E_KEY_LABEL (optional; default: pa-e2e-ollama-cloud)
//! - PA_E2E_API_KEY (recommended for non-interactive runs)

use personal_agent::{AuthConfig, LlmClient, ModelProfile};

mod support;

fn load_e2e_profile() -> ModelProfile {
    support::e2e_config::load_e2e_profile()
}

#[tokio::test]
#[ignore = "Requires PA_E2E_* configuration"]
async fn test_real_chat_with_synthetic_api() {
    println!("=== E2E Test: Real Chat with PA_E2E Profile ===\n");

    let profile = load_e2e_profile();
    println!(
        "Profile loaded: {} / {}",
        profile.provider_id, profile.model_id
    );
    println!("Base URL: {}", profile.base_url);

    let AuthConfig::Keychain { ref label } = profile.auth;
    assert!(!label.is_empty(), "Key label must not be empty");
    println!("Key label: {label} [OK]");

    let api_key_override_present = std::env::var("PA_E2E_API_KEY")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());

    if !api_key_override_present {
        let key_exists = personal_agent::services::secure_store::api_keys::exists(label)
            .expect("Keychain lookup should not fail");
        assert!(
            key_exists,
            "Expected configured PA_E2E key label to exist in secure store when PA_E2E_API_KEY is unset"
        );
    }

    let client =
        LlmClient::from_profile(&profile).expect("Failed to create LlmClient from profile");

    println!("\nSending test message to LLM...");

    let messages = vec![personal_agent::LlmMessage::user(
        "Say 'Hello from E2E test' and nothing else.",
    )];

    let mut response_text = String::new();
    let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let events_clone = events.clone();

    let result = client
        .request_stream_with_tools(&messages, &[], move |event| {
            events_clone.lock().unwrap().push(event.clone());
            if let personal_agent::StreamEvent::TextDelta(text) = event {
                print!("{text}");
                std::io::Write::flush(&mut std::io::stdout()).ok();
            }
        })
        .await;

    println!("\n");

    // Verify we got a real response
    match result {
        Ok(()) => {
            {
                let events = events.lock().unwrap();
                for event in events.iter() {
                    if let personal_agent::StreamEvent::TextDelta(text) = event {
                        response_text.push_str(text);
                    }
                }
            }

            assert!(!response_text.is_empty(), "Should get response from LLM");
            println!("[OK] Got response: {}", response_text.trim());
            println!("[OK] E2E test PASSED - Real LLM interaction works!");
        }
        Err(e) => {
            panic!("E2E test FAILED: LLM request failed: {e}");
        }
    }
}
