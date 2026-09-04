//! Hardware regression for the app-shaped chat path over the local model.
//!
//! The scripted tests hand-build `ModelRequest` values and the raw-prompt
//! hardware test drives the engine `Generator` directly; neither covers
//! app-shaped messages -> `LocalLlamaModel::request_stream` -> real engine.
//! This file pins that exact path: it builds the request the way the app
//! builds it (system + user `ModelRequest`s, tools in the parameters, profile
//! sampling) and asserts the stream carries visible text content.
//!
//! Run with:
//! `PA_LOCAL_GGUF=tmp/models/granite-4.2-3b-Q8_0.gguf cargo test --test local_model_repro_tests -- --ignored --nocapture`
//!
// @requirement:REQ-LM-004

use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use personal_agent::llm::local::engine::{EngineHandle, EngineLoadSettings, EngineStatus};
use personal_agent::llm::local::generator::{GenRequest, GenerateError, Generation, Generator};
use personal_agent::llm::local::llama_model::LocalLlamaModel;
use personal_agent::llm::local::render::{render, ChatTurn, ToolSpec};
use personal_agent::models::profile::DEFAULT_SYSTEM_PROMPT;
use serdes_ai::core::messages::{
    ModelRequest, ModelRequestPart, ModelResponsePartDelta, ModelResponseStreamEvent,
    SystemPromptPart, UserContent, UserPromptPart,
};
use serdes_ai::models::{Model, ModelRequestParameters};
use serdes_ai_tools::ToolDefinition;

const USER_MESSAGE: &str = "howdy";

/// GGUF resolution mirrors the app: env override first, then the default
/// app-support path where the settings UI installs the model.
fn gguf_path() -> PathBuf {
    std::env::var_os("PA_LOCAL_GGUF").map_or_else(
        || {
            let home = std::env::var_os("HOME").expect("HOME must be set");
            PathBuf::from(home)
                .join("Library/Application Support/PersonalAgent/models/granite-4.2-3b-Q8_0.gguf")
        },
        PathBuf::from,
    )
}

/// Real-engine generator with explicit settings, standing in for the private
/// `EngineGenerator` (which reads persisted settings instead of `PA_LOCAL_GGUF`).
struct HardwareGenerator {
    engine: EngineHandle,
    settings: EngineLoadSettings,
}

#[async_trait::async_trait]
impl Generator for HardwareGenerator {
    async fn generate(&self, request: GenRequest) -> Result<Generation, GenerateError> {
        self.engine.start_generation(request, self.settings.clone())
    }

    fn status(&self) -> EngineStatus {
        self.engine.status()
    }

    async fn unload(&self) {
        self.engine.request_unload();
    }
}

/// The `[SystemPrompt, UserPrompt]` message list the serdes agent path sends,
/// with the app's default system prompt.
fn app_shaped_messages() -> Vec<ModelRequest> {
    let mut system = ModelRequest::default();
    system
        .parts
        .push(ModelRequestPart::SystemPrompt(SystemPromptPart::new(
            DEFAULT_SYSTEM_PROMPT.to_string(),
        )));
    let mut user = ModelRequest::default();
    user.parts
        .push(ModelRequestPart::UserPrompt(UserPromptPart::new(
            UserContent::text(USER_MESSAGE),
        )));
    vec![system, user]
}

/// The tool list shape the agent path sends (one representative MCP-style
/// tool; the scaffold it renders is the same for any tool set).
fn app_shaped_params() -> ModelRequestParameters {
    let tool = ToolDefinition::new("web_search", "Search the web for information.")
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "The search query."}
            },
            "required": ["query"]
        }));
    ModelRequestParameters::new()
        .with_tools_arc(Arc::new(vec![tool]))
        .with_allow_text(true)
}

/// The profile knobs the app sends for the failing conversations.
fn app_shaped_settings() -> serdes_ai::core::ModelSettings {
    serdes_ai::core::ModelSettings {
        temperature: Some(1.0),
        top_p: Some(1.0),
        max_tokens: Some(4096),
        ..serdes_ai::core::ModelSettings::default()
    }
}

/// Reconstructs the prompt `render_prompt` builds, for display only: the
/// assertion lives on the stream events, not on this string.
fn display_prompt() -> String {
    let turns = vec![
        ChatTurn::System {
            content: DEFAULT_SYSTEM_PROMPT.to_string(),
        },
        ChatTurn::User {
            content: USER_MESSAGE.to_string(),
        },
    ];
    let tools = vec![ToolSpec {
        name: "web_search".to_string(),
        description: "Search the web for information.".to_string(),
        parameters_json: serde_json::json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "The search query."}
            },
            "required": ["query"]
        })
        .to_string(),
    }];
    render(&turns, &tools, true)
}

/// App-shaped request through `request_stream` against the real GGUF must
/// produce non-empty text content.
#[tokio::test]
#[ignore = "loads the Granite GGUF and runs Metal inference"]
async fn app_shaped_request_over_request_stream_produces_text() {
    let path = gguf_path();
    assert!(path.exists(), "GGUF not found at {}", path.display());
    eprintln!("GGUF: {}", path.display());

    let generator = HardwareGenerator {
        engine: EngineHandle::spawn(),
        settings: EngineLoadSettings {
            model_path: path,
            n_ctx: 2048,
            gpu_layers: 999,
            idle_unload: false,
            idle_timeout: std::time::Duration::from_secs(60),
        },
    };
    let model = LocalLlamaModel::new(Arc::new(generator), "granite-4.2-3b");

    let messages = app_shaped_messages();
    let params = app_shaped_params();
    let settings = app_shaped_settings();

    eprintln!(
        "== rendered prompt ==\n{}\n== end prompt ==",
        display_prompt()
    );

    let mut stream = model
        .request_stream(&messages, &settings, &params)
        .await
        .expect("stream starts");

    let mut text = String::new();
    let mut input_tokens = None;
    let mut output_tokens = None;
    while let Some(event) = stream.next().await {
        let event = event.expect("stream event");
        eprintln!("event: {event:?}");
        match event {
            ModelResponseStreamEvent::PartDelta(delta) => {
                if let ModelResponsePartDelta::Text(text_delta) = delta.delta {
                    text.push_str(&text_delta.content_delta);
                }
            }
            ModelResponseStreamEvent::StreamComplete(complete) => {
                input_tokens = complete.input_tokens;
                output_tokens = complete.output_tokens;
            }
            _ => {}
        }
    }

    eprintln!(
        "== stream summary ==\ntext: {text:?}\ninput_tokens: {input_tokens:?}\noutput_tokens: {output_tokens:?}"
    );

    assert!(
        !text.trim().is_empty(),
        "app-shaped request produced no text content (input_tokens={input_tokens:?}, \
         output_tokens={output_tokens:?})"
    );
}
