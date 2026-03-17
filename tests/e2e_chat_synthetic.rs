//! E2E test using the real runtime-selected keychain-backed profile.
//!
//! This test hits the actual API - run with:
//!   cargo test --test e2e_chat_synthetic -- --ignored --nocapture
//!
//! Requires:
//! - ~/Library/Application Support/PersonalAgent/profiles/default.json
//! - the corresponding runtime profile JSON
//! - the profile's keychain label to exist in the OS keychain

use personal_agent::{AuthConfig, LlmClient, ModelProfile};

/// Load the runtime-selected default profile from the app support directory.
fn load_runtime_default_profile() -> ModelProfile {
    let home = dirs::home_dir().expect("No home directory");
    let profiles_dir = home.join("Library/Application Support/PersonalAgent/profiles");
    let default_path = profiles_dir.join("default.json");

    let default_content = std::fs::read_to_string(&default_path)
        .expect("Failed to read runtime profiles/default.json");
    let default_profile_id: String =
        serde_json::from_str(&default_content).expect("Failed to parse runtime profiles/default.json");

    let profile_path = profiles_dir.join(format!("{default_profile_id}.json"));
    let content = std::fs::read_to_string(&profile_path)
        .unwrap_or_else(|_| panic!("Failed to read runtime profile: {}", profile_path.display()));

    serde_json::from_str(&content).expect("Failed to parse runtime default profile JSON")
}

#[tokio::test]
#[ignore] // Run manually: cargo test --test e2e_chat_synthetic -- --ignored --nocapture
async fn test_real_chat_with_synthetic_api() {
    println!("=== E2E Test: Real Chat with Runtime Default Profile ===\n");

    let profile = load_runtime_default_profile();
    println!(
        "Profile loaded: {} / {}",
        profile.provider_id, profile.model_id
    );
    println!("Base URL: {}", profile.base_url);

    let AuthConfig::Keychain { ref label } = profile.auth;
    assert!(!label.is_empty(), "Keychain label must not be empty");
    println!("Keychain label: {} [OK]", label);

    let key_exists = personal_agent::services::secure_store::api_keys::exists(label)
        .expect("Keychain lookup should not fail");
    assert!(key_exists, "Expected runtime profile keychain label to exist");

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
                print!("{}", text);
                std::io::Write::flush(&mut std::io::stdout()).ok();
            }
        })
        .await;

    println!("\n");

    // Verify we got a real response
    match result {
        Ok(_) => {
            let events = events.lock().unwrap();
            for event in events.iter() {
                if let personal_agent::StreamEvent::TextDelta(text) = event {
                    response_text.push_str(text);
                }
            }

            assert!(!response_text.is_empty(), "Should get response from LLM");
            println!("[OK] Got response: {}", response_text.trim());
            println!("[OK] E2E test PASSED - Real LLM interaction works!");
        }
        Err(e) => {
            panic!("E2E test FAILED: LLM request failed: {}", e);
        }
    }
}
