//! E2E test using real Synthetic API with GLM-4.6
//!
//! This test hits the actual API - run with:
//!   cargo test --test e2e_chat_synthetic -- --ignored --nocapture
//!
//! Requires:
//! - ~/.llxprt/profiles/synthetic.json (profile config)
//! - ~/.synthetic_key (API key)

use personal_agent::{AuthConfig, LlmClient, ModelProfile};
use std::path::PathBuf;

/// Load synthetic profile from ~/.llxprt/profiles/synthetic.json
fn load_synthetic_profile() -> ModelProfile {
    let home = dirs::home_dir().expect("No home directory");
    let profile_path = home.join(".llxprt/profiles/synthetic.json");

    let content = std::fs::read_to_string(&profile_path)
        .expect("Failed to read ~/.llxprt/profiles/synthetic.json");

    let json: serde_json::Value =
        serde_json::from_str(&content).expect("Failed to parse synthetic.json");

    let provider = json["provider"].as_str().unwrap_or("openai").to_string();
    let model = json["model"]
        .as_str()
        .expect("No model in profile")
        .to_string();
    let base_url = json["ephemeralSettings"]["base-url"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let keyfile_path = json["ephemeralSettings"]["auth-keyfile"]
        .as_str()
        .unwrap_or("~/.synthetic_key")
        .to_string();
    
    // Expand ~ to home directory
    let keyfile_path = if keyfile_path.starts_with("~/") {
        home.join(&keyfile_path[2..]).to_string_lossy().to_string()
    } else {
        keyfile_path
    };

    ModelProfile::new(
        "Synthetic GLM".to_string(),
        provider,
        model,
        base_url,
        AuthConfig::Keyfile { path: keyfile_path },
    )
}

#[tokio::test]
#[ignore] // Run manually: cargo test --test e2e_chat_synthetic -- --ignored --nocapture
async fn test_real_chat_with_synthetic_api() {
    println!("=== E2E Test: Real Chat with Synthetic API ===\n");

    // Load profile from user's config
    let profile = load_synthetic_profile();
    println!("Profile loaded: {} / {}", profile.provider_id, profile.model_id);
    println!("Base URL: {}", profile.base_url);

    // Verify key file exists
    if let AuthConfig::Keyfile { ref path } = profile.auth {
        let path = std::path::Path::new(path);
        assert!(path.exists(), "Key file not found: {:?}", path);
        println!("Key file: {:?} [OK]", path);
    }

    // Create LlmClient from profile
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
