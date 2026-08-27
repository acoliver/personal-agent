//! Live turns against a real Responses endpoint, using a seeded grant.
//!
//! Gated behind `#[ignore]` and the `PA_E2E_CODEX_*` environment, matching the
//! other e2e tests here. Nothing in this file asks a human for anything: the
//! token is seeded, so these can run unattended in CI.
//!
//! ```bash
//! export PA_E2E_CODEX_ACCOUNT=chatgpt-acct-1
//! export PA_E2E_CODEX_TOKEN_JSON="$(cat ~/.keys/pa-codex.json)"
//! cargo test --test e2e_codex_stream -- --ignored --nocapture
//! ```

use std::sync::{Arc, Mutex};

use personal_agent::llm::{LlmClient, Message, StreamEvent};
use personal_agent::models::{AuthConfig, ModelProfile};
use personal_agent::services::secure_store;
use uuid::Uuid;

const ACCOUNT_ENV: &str = "PA_E2E_CODEX_ACCOUNT";
const TOKEN_ENV: &str = "PA_E2E_CODEX_TOKEN_JSON";
const MODEL_ENV: &str = "PA_E2E_CODEX_MODEL";
const ENDPOINT_ENV: &str = "PA_E2E_CODEX_ENDPOINT";

const DEFAULT_MODEL: &str = "gpt-5.6-luna";
const DEFAULT_ENDPOINT: &str = "wss://chatgpt.com/backend-api/codex/responses";

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Seed the grant the profile will use, and return its account slug.
///
/// `PA_E2E_CODEX_TOKEN_JSON` carries the whole record so CI can inject one
/// without this test knowing how it was obtained.
fn seed_account() -> String {
    let account = env_or(ACCOUNT_ENV, "");
    assert!(
        !account.is_empty(),
        "set {ACCOUNT_ENV} to the account slug to test with"
    );

    if let Ok(blob) = std::env::var(TOKEN_ENV) {
        if !blob.trim().is_empty() {
            secure_store::oauth_tokens::store(&account, blob.trim())
                .expect("seed the grant into the keychain");
        }
    }

    let stored = personal_agent::services::oauth::store::load(&account)
        .expect("read the seeded grant")
        .unwrap_or_else(|| {
            panic!("no grant stored for {account}; set {TOKEN_ENV} or sign in first")
        });
    assert!(
        !stored.needs_reauth,
        "the seeded grant for {account} needs a fresh sign-in"
    );
    account
}

fn profile(account: &str) -> ModelProfile {
    ModelProfile::new(
        "Codex E2E".to_string(),
        "openai-codex".to_string(),
        env_or(MODEL_ENV, DEFAULT_MODEL),
        env_or(ENDPOINT_ENV, DEFAULT_ENDPOINT),
        AuthConfig::OAuth {
            account: account.to_string(),
        },
    )
}

/// Run one streamed turn and collect what the UI would see.
async fn run_turn(client: &LlmClient, history: &[Message]) -> Vec<StreamEvent> {
    let collected = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&collected);
    client
        .request_stream(history, move |event| {
            sink.lock().expect("event sink").push(event);
        })
        .await
        .expect("stream");
    let events = collected.lock().expect("event sink").clone();
    events
}

fn text_of(events: &[StreamEvent]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::TextDelta(delta) => Some(delta.clone()),
            _ => None,
        })
        .collect()
}

fn completions(events: &[StreamEvent]) -> Vec<(Option<u32>, Option<u32>)> {
    events
        .iter()
        .filter_map(|event| match event {
            StreamEvent::Complete {
                input_tokens,
                output_tokens,
            } => Some((*input_tokens, *output_tokens)),
            _ => None,
        })
        .collect()
}

#[tokio::test]
#[ignore = "Requires PA_E2E_CODEX_* configuration and network access"]
async fn a_live_turn_streams_text_and_reports_usage() {
    let account = seed_account();
    let client = LlmClient::from_profile(&profile(&account))
        .expect("client")
        .for_conversation(Uuid::new_v4());

    let events = run_turn(
        &client,
        &[Message::user("Reply with the single word: ready")],
    )
    .await;

    let text = text_of(&events);
    println!("-- response: {text}");
    assert!(!text.trim().is_empty(), "no text arrived: {events:?}");

    let done = completions(&events);
    assert_eq!(done.len(), 1, "exactly one terminal event: {events:?}");
    let (input, output) = done[0];
    println!("-- usage: input={input:?} output={output:?}");
    assert!(
        input.is_some_and(|t| t > 0),
        "the provider reported no input tokens"
    );
    assert!(
        output.is_some_and(|t| t > 0),
        "the provider reported no output tokens"
    );
}

#[tokio::test]
#[ignore = "Requires PA_E2E_CODEX_* configuration and network access"]
async fn a_second_turn_continues_the_same_conversation() {
    let account = seed_account();
    let conversation = Uuid::new_v4();
    let client = LlmClient::from_profile(&profile(&account))
        .expect("client")
        .for_conversation(conversation);

    let first = run_turn(
        &client,
        &[Message::user("My favourite colour is teal. Say ok.")],
    )
    .await;
    let first_text = text_of(&first);
    assert!(!first_text.trim().is_empty(), "first turn returned nothing");

    let history = vec![
        Message::user("My favourite colour is teal. Say ok."),
        Message::assistant(first_text),
        Message::user("What is my favourite colour? Answer with one word."),
    ];
    let second = run_turn(&client, &history).await;
    let second_text = text_of(&second);

    println!("-- second turn: {second_text}");
    assert!(
        second_text.to_lowercase().contains("teal"),
        "the chained turn lost the conversation: {second_text}"
    );
    assert_eq!(completions(&second).len(), 1);
}

#[tokio::test]
#[ignore = "Requires PA_E2E_CODEX_* configuration and network access"]
async fn a_live_tool_call_round_trips() {
    use personal_agent::llm::tools::{Tool, ToolResult};

    let account = seed_account();
    let client = LlmClient::from_profile(&profile(&account))
        .expect("client")
        .for_conversation(Uuid::new_v4());

    let tools = vec![Tool {
        name: "get_weather".to_string(),
        description: "Get the current weather for a city.".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": { "city": {"type": "string", "description": "City name"} },
            "required": ["city"],
            "additionalProperties": false
        }),
    }];

    let collected = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&collected);
    client
        .request_stream_with_tools(
            &[Message::user(
                "What is the weather in Tokyo? Call the get_weather tool.",
            )],
            &tools,
            move |event| sink.lock().expect("sink").push(event),
        )
        .await
        .expect("stream");
    let first: Vec<StreamEvent> = collected.lock().expect("sink").clone();

    let call = first
        .iter()
        .find_map(|event| match event {
            StreamEvent::ToolUse(use_) => Some(use_.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no tool call arrived; text was {:?}", text_of(&first)));
    println!("-- tool call: {} {}", call.name, call.input);
    let empty_args = call.input.is_null()
        || call
            .input
            .as_object()
            .is_some_and(serde_json::Map::is_empty);
    assert!(
        !empty_args,
        "the tool call arrived with empty arguments: argument deltas did not assemble"
    );

    let mut assistant = Message::assistant(text_of(&first));
    assistant.tool_uses = vec![call.clone()];
    let mut result_turn = Message::user("");
    result_turn.tool_results = vec![ToolResult::success(
        &call.id,
        r#"{"temperature_c": 18, "conditions": "clear"}"#,
    )];

    let history = vec![
        Message::user("What is the weather in Tokyo? Call the get_weather tool."),
        assistant,
        result_turn,
    ];
    let second = run_turn(&client, &history).await;
    let text = text_of(&second);

    println!("-- final answer: {text}");
    assert!(!text.trim().is_empty(), "the chained turn returned no text");
}
