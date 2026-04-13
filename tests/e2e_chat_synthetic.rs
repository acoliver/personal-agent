//! E2E test using `PA_E2E_*` environment-backed profile configuration.
//!
//! This test hits the actual API - run with:
//!   cargo test --test `e2e_chat_synthetic` -- --ignored --nocapture
//!
//! Requires:
//! - `PA_E2E_PROVIDER_ID` (optional; default: `ollama`)
//! - `PA_E2E_MODEL_ID` (optional; default: `minimax-m2.7:cloud`)
//! - `PA_E2E_BASE_URL` (optional; default: <https://ollama.com/v1>)
//! - `PA_E2E_KEY_LABEL` (optional; default: `pa-e2e-ollama-cloud`)
//! - `PA_E2E_API_KEY` (recommended for non-interactive runs)

use personal_agent::{AuthConfig, LlmClient, ModelProfile};

mod support;

fn load_e2e_profile() -> ModelProfile {
    support::e2e_config::load_e2e_profile()
}

fn summarize_events(events: &[personal_agent::StreamEvent]) -> (String, Vec<String>) {
    let mut response_text = String::new();
    let summaries = events
        .iter()
        .map(|event| match event {
            personal_agent::StreamEvent::TextDelta(text) => {
                response_text.push_str(text);
                format!("TextDelta({} chars)", text.len())
            }
            personal_agent::StreamEvent::ThinkingDelta(text) => {
                format!("ThinkingDelta({} chars)", text.len())
            }
            personal_agent::StreamEvent::ToolUse(tool_use) => {
                format!("ToolUse({})", tool_use.name)
            }
            personal_agent::StreamEvent::ToolCallStarted { tool_name, .. } => {
                format!("ToolCallStarted({tool_name})")
            }
            personal_agent::StreamEvent::ToolCallCompleted {
                tool_name, success, ..
            } => {
                format!("ToolCallCompleted({tool_name}, success={success})")
            }
            personal_agent::StreamEvent::ToolTranscript {
                tool_calls,
                tool_results,
            } => {
                format!(
                    "ToolTranscript(calls={}, results={})",
                    tool_calls.len(),
                    tool_results.len()
                )
            }
            personal_agent::StreamEvent::Complete {
                input_tokens,
                output_tokens,
            } => {
                format!("Complete(input_tokens={input_tokens:?}, output_tokens={output_tokens:?})")
            }
            personal_agent::StreamEvent::Error(message) => {
                format!("Error({message})")
            }
        })
        .collect();

    (response_text, summaries)
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

    match &profile.auth {
        AuthConfig::Keychain { label } => {
            assert!(!label.is_empty(), "Key label must not be empty");
            println!("Key label: {label} [OK]");
        }
        AuthConfig::None => {
            println!("No API key required (local model)");
        }
    }

    let api_key_override_present = std::env::var("PA_E2E_API_KEY")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());

    if !api_key_override_present {
        if let AuthConfig::Keychain { label } = &profile.auth {
            let key_exists = personal_agent::services::secure_store::api_keys::exists(label)
                .expect("Keychain lookup should not fail");
            assert!(
                key_exists,
                "Expected configured PA_E2E key label to exist in secure store when PA_E2E_API_KEY is unset"
            );
        }
    }

    let client =
        LlmClient::from_profile(&profile).expect("Failed to create LlmClient from profile");

    println!("\nSending test message to LLM...");

    let messages = vec![personal_agent::LlmMessage::user(
        "Say 'Hello from E2E test' and nothing else.",
    )];

    let mut last_error = None;
    let mut last_event_summary = Vec::new();

    for attempt in 1..=2 {
        if attempt > 1 {
            println!("\nRetrying live E2E request after empty response on attempt {attempt}...");
        }

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

        match result {
            Ok(()) => {
                let recorded_events = {
                    let events = events.lock().unwrap();
                    events.clone()
                };
                let (response_text, event_summary) = summarize_events(&recorded_events);
                last_event_summary = event_summary;

                if !response_text.trim().is_empty() {
                    println!("[OK] Got response: {}", response_text.trim());
                    println!("[OK] E2E test PASSED - Real LLM interaction works!");
                    return;
                }

                last_error = Some(format!(
                    "LLM stream completed without text response on attempt {attempt}"
                ));
            }
            Err(e) => {
                last_error = Some(format!("LLM request failed on attempt {attempt}: {e}"));
            }
        }
    }

    panic!(
        "E2E test FAILED: {}. Observed events: {:?}",
        last_error.unwrap_or_else(|| "unknown live E2E failure".to_string()),
        last_event_summary
    );
}
