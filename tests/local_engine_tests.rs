//! Scripted-generator tests for the local model stream mapping, plus the
//! engine actor's load-failure and idle-unload behaviour.
//!
//! The GGUF-bearing end-to-end test is `#[ignore]`d; run it with
//! `PA_LOCAL_GGUF=tmp/models/granite-4.2-3b-Q8_0.gguf cargo test local_engine -- --ignored`.
//!
//! @plan:PLAN-20260903-LOCALMODEL.P02
//! @requirement:REQ-LM-003 REQ-LM-004

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::StreamExt;
use personal_agent::llm::local::engine::{EngineHandle, EngineLoadSettings, EngineStatus};
use personal_agent::llm::local::generator::{
    GenEvent, GenRequest, GenSampling, GenerateError, Generation, Generator,
};
use personal_agent::llm::local::llama_model::LocalLlamaModel;
use personal_agent::llm::local::toolcall::TOOL_CALL_CLOSE;
use serdes_ai::core::messages::{
    FinishReason, ModelRequest, ModelRequestPart, ModelResponsePart, ModelResponsePartDelta,
    ModelResponseStreamEvent, SystemPromptPart, UserContent, UserPromptPart,
};
use serdes_ai::models::Model;
use serdes_ai::models::ModelRequestParameters;

/// A generator that replays a fixed event list, standing in for the actor.
#[derive(Clone)]
struct ScriptedGenerator {
    events: Vec<GenEvent>,
}

#[async_trait::async_trait]
impl Generator for ScriptedGenerator {
    async fn generate(&self, _request: GenRequest) -> Result<Generation, GenerateError> {
        let cancelled = Arc::new(Mutex::new(HashSet::new()));
        Ok(Generation::new(
            0,
            Box::pin(futures::stream::iter(self.events.clone())),
            cancelled,
        ))
    }

    fn status(&self) -> EngineStatus {
        EngineStatus::NotLoaded
    }

    async fn unload(&self) {}
}

fn scripted_model(events: Vec<GenEvent>) -> LocalLlamaModel {
    LocalLlamaModel::new(Arc::new(ScriptedGenerator { events }), "granite-4.2-3b")
}

fn simple_request() -> Vec<ModelRequest> {
    let mut request = ModelRequest::default();
    request
        .parts
        .push(ModelRequestPart::SystemPrompt(SystemPromptPart::new(
            "be brief",
        )));
    request
        .parts
        .push(ModelRequestPart::UserPrompt(UserPromptPart::new(
            UserContent::text("hello"),
        )));
    vec![request]
}

/// Collects a whole stream into events, failing the test on stream errors.
async fn collect_events(
    model: &LocalLlamaModel,
    messages: &[ModelRequest],
) -> Vec<ModelResponseStreamEvent> {
    let settings = serdes_ai::core::ModelSettings::default();
    let params = ModelRequestParameters::default();
    let mut stream = model
        .request_stream(messages, &settings, &params)
        .await
        .expect("stream starts");
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event.expect("stream event"));
    }
    events
}

fn tool_call_text() -> String {
    "<tool_call>\n<function=get_weather>\n<parameter=city>\nParis\n</parameter>\n</function>\n</tool_call>"
        .to_string()
}

#[tokio::test]
async fn text_only_generation_streams_deltas_then_completion() {
    let model = scripted_model(vec![
        GenEvent::Delta("Hello".to_string()),
        GenEvent::Delta(" world".to_string()),
        GenEvent::Complete {
            prompt_tokens: 12,
            generated_tokens: 3,
        },
    ]);

    let events = collect_events(&model, &simple_request()).await;

    // MARKER_HOLDBACK buffers the tail until no `<tool_call>` opener can be
    // forming, so short pieces regroup ("Hello" is under the holdback window);
    // the streamed text and completion counts are what the contract fixes.
    assert_eq!(
        events,
        vec![
            ModelResponseStreamEvent::part_start(
                0,
                ModelResponsePart::Text(serdes_ai::core::messages::TextPart::new(""))
            ),
            ModelResponseStreamEvent::text_delta(0, "H"),
            ModelResponseStreamEvent::text_delta(0, "ello world"),
            ModelResponseStreamEvent::part_end(0),
            ModelResponseStreamEvent::StreamComplete(
                serdes_ai::core::messages::StreamCompleteEvent {
                    finish_reason: FinishReason::Stop,
                    input_tokens: Some(12),
                    output_tokens: Some(3),
                    cache_creation_tokens: None,
                    cache_read_tokens: None,
                },
            ),
        ]
    );
}

#[tokio::test]
async fn a_complete_tool_call_block_becomes_one_tool_use_part() {
    // Split the block across pieces so the buffering is exercised; the pieces
    // cut arbitrarily, the way real tokenization would.
    let whole = tool_call_text();
    let cut = whole.len() / 3;
    let model = scripted_model(vec![
        GenEvent::Delta(whole[..cut].to_string()),
        GenEvent::Delta(whole[cut..2 * cut].to_string()),
        GenEvent::Delta(whole[2 * cut..].to_string()),
        GenEvent::Complete {
            prompt_tokens: 20,
            generated_tokens: 30,
        },
    ]);

    let events = collect_events(&model, &simple_request()).await;

    let mut saw_tool_start = false;
    let mut part_ends = 0usize;
    let mut saw_complete = false;
    for event in &events {
        match event {
            ModelResponseStreamEvent::PartStart(start) => match &start.part {
                ModelResponsePart::ToolCall(call) => {
                    saw_tool_start = true;
                    assert_eq!(call.tool_name, "get_weather");
                    assert_eq!(
                        call.args,
                        serdes_ai::core::messages::ToolCallArgs::Json(serde_json::json!({
                            "city": "Paris"
                        }))
                    );
                }
                other => panic!("unexpected part start: {other:?}"),
            },
            ModelResponseStreamEvent::PartEnd(_) => part_ends += 1,
            ModelResponseStreamEvent::StreamComplete(complete) => {
                saw_complete = true;
                assert_eq!(complete.finish_reason, FinishReason::ToolCall);
                assert_eq!(complete.input_tokens, Some(20));
                assert_eq!(complete.output_tokens, Some(30));
            }
            ModelResponseStreamEvent::PartDelta(delta) => match &delta.delta {
                ModelResponsePartDelta::Text(text) => {
                    // Raw markers must never leak into visible text.
                    assert!(!text.content_delta.contains("<tool_call>"));
                    assert!(!text.content_delta.contains(TOOL_CALL_CLOSE));
                }
                other => panic!("unexpected delta: {other:?}"),
            },
        }
    }
    assert!(saw_tool_start, "tool use part never started: {events:?}");
    assert_eq!(part_ends, 1, "only the tool part closes");
    assert!(saw_complete);
}

#[tokio::test]
async fn multiple_tool_call_blocks_each_get_their_own_part() {
    let first = tool_call_text();
    let second = "<tool_call>\n<function=ping>\n</function>\n</tool_call>".to_string();
    let model = scripted_model(vec![
        GenEvent::Delta(first),
        GenEvent::Delta(second),
        GenEvent::Complete {
            prompt_tokens: 5,
            generated_tokens: 40,
        },
    ]);

    let events = collect_events(&model, &simple_request()).await;
    let tool_names: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            ModelResponseStreamEvent::PartStart(start) => match &start.part {
                ModelResponsePart::ToolCall(call) => Some(call.tool_name.as_str()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(tool_names, vec!["get_weather", "ping"]);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ModelResponseStreamEvent::PartEnd(_)))
            .count(),
        2
    );
}

#[tokio::test]
async fn prose_around_a_tool_call_stays_visible_text() {
    let model = scripted_model(vec![
        GenEvent::Delta("Let me check. ".to_string()),
        GenEvent::Delta(tool_call_text()),
        GenEvent::Delta("Done.".to_string()),
        GenEvent::Complete {
            prompt_tokens: 5,
            generated_tokens: 9,
        },
    ]);

    let events = collect_events(&model, &simple_request()).await;
    let text: String = events
        .iter()
        .filter_map(|event| match event {
            ModelResponseStreamEvent::PartDelta(delta) => match &delta.delta {
                ModelResponsePartDelta::Text(text) => Some(text.content_delta.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(text, "Let me check. Done.");
}

#[tokio::test]
async fn a_failed_generation_surfaces_as_a_stream_error() {
    let model = scripted_model(vec![GenEvent::Failed("decode exploded".to_string())]);
    let settings = serdes_ai::core::ModelSettings::default();
    let params = ModelRequestParameters::default();
    let mut stream = model
        .request_stream(&simple_request(), &settings, &params)
        .await
        .expect("stream starts");
    let error = stream
        .next()
        .await
        .expect("an event")
        .expect_err("a failure");
    assert!(error.to_string().contains("decode exploded"));
}

#[tokio::test]
async fn request_collects_stream_events_into_one_response() {
    let model = scripted_model(vec![
        GenEvent::Delta("Hi".to_string()),
        GenEvent::Complete {
            prompt_tokens: 2,
            generated_tokens: 1,
        },
    ]);
    let settings = serdes_ai::core::ModelSettings::default();
    let params = ModelRequestParameters::default();

    let response = model
        .request(&simple_request(), &settings, &params)
        .await
        .expect("response");

    assert_eq!(response.parts.len(), 1);
    assert!(matches!(
        &response.parts[0],
        ModelResponsePart::Text(text) if text.content == "Hi"
    ));
    assert_eq!(response.finish_reason, Some(FinishReason::Stop));
    let usage = response.usage.expect("usage");
    assert_eq!(usage.request_tokens, Some(2));
    assert_eq!(usage.response_tokens, Some(1));
    assert_eq!(usage.total_tokens, Some(3));
}

/// Dropping a generation marks it cancelled so the actor stops at the next
/// token boundary.
#[tokio::test]
async fn dropping_a_generation_inserts_it_into_the_cancel_set() {
    let cancelled: Arc<Mutex<HashSet<u64>>> = Arc::new(Mutex::new(HashSet::new()));
    let generation = Generation::new(
        7,
        Box::pin(futures::stream::empty::<GenEvent>()),
        Arc::clone(&cancelled),
    );
    assert!(!cancelled.lock().expect("set").contains(&7));
    drop(generation);
    assert!(cancelled.lock().expect("set").contains(&7));
}

/// A load failure (missing GGUF here) must surface as a Failed event and an
/// Error status, not a hang. Which engine thread wins the process-global
/// backend decides the message, so only the shape is asserted.
#[tokio::test]
async fn engine_load_failure_fails_the_generation_and_sets_error_status() {
    let engine = EngineHandle::spawn();
    assert_eq!(engine.status(), EngineStatus::NotLoaded);

    let settings = EngineLoadSettings {
        model_path: PathBuf::from("/nonexistent/path/model.gguf"),
        n_ctx: 512,
        gpu_layers: 0,
        idle_unload: false,
        idle_timeout: Duration::from_secs(60),
    };
    let generation = engine
        .start_generation(
            GenRequest {
                prompt: "<|im_start|>assistant\n<think></think>".to_string(),
                sampling: GenSampling {
                    temperature: 0.1,
                    top_p: None,
                    seed: Some(1234),
                },
                max_tokens: 16,
                stop: Vec::new(),
            },
            settings,
        )
        .expect("job accepted");

    // Drive the tokio receiver stream to its terminal event.
    let mut events = generation.events;
    let mut saw_failure = false;
    while let Some(event) = events.next().await {
        match event {
            GenEvent::Failed(_) => {
                saw_failure = true;
            }
            other => panic!("unexpected event before failure: {other:?}"),
        }
    }
    assert!(saw_failure, "load failure never surfaced");
    for _ in 0..100 {
        if matches!(engine.status(), EngineStatus::Error { .. }) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("status never reached Error: {:?}", engine.status());
}

/// Real-model end-to-end: load, generate a few tokens, then idle-unload.
/// Requires the Granite GGUF; see the file header for the invocation.
#[tokio::test]
#[ignore = "loads the 3.6 GB GGUF and runs Metal inference"]
async fn real_model_generates_and_idle_unloads() {
    let Some(path) = std::env::var_os("PA_LOCAL_GGUF").map(PathBuf::from) else {
        panic!("PA_LOCAL_GGUF must point at the Granite GGUF");
    };
    let engine = EngineHandle::spawn();
    let settings = EngineLoadSettings {
        model_path: path,
        n_ctx: 2048,
        gpu_layers: 999,
        idle_unload: true,
        idle_timeout: Duration::from_secs(2),
    };
    let generation = engine
        .start_generation(
            GenRequest {
                prompt: "<|im_start|>system\nAnswer in one short sentence.<|im_end|>\n<|im_start|>user\nWhat is 2+2?<|im_end|>\n<|im_start|>assistant\n<think></think>".to_string(),
                sampling: GenSampling {
                    temperature: 0.1,
                    top_p: None,
                    seed: Some(1234),
                },
                max_tokens: 64,
                stop: Vec::new(),
            },
            settings.clone(),
        )
        .expect("job accepted");

    let mut events = generation.events;
    let mut complete = None;
    while let Some(event) = events.next().await {
        match event {
            GenEvent::Delta(_) => {}
            GenEvent::Complete {
                prompt_tokens,
                generated_tokens,
            } => {
                complete = Some((prompt_tokens, generated_tokens));
            }
            GenEvent::Failed(message) => panic!("generation failed: {message}"),
        }
    }
    let (prompt_tokens, generated_tokens) = complete.expect("completion event");
    assert!(prompt_tokens > 0);
    assert!(generated_tokens > 0);
    match engine.status() {
        EngineStatus::Loaded { .. } => {}
        other => panic!("expected Loaded after generation, got {other:?}"),
    }

    // Idle timeout is 2s; the actor should drop the model and return to
    // NotLoaded.
    for _ in 0..300 {
        if engine.status() == EngineStatus::NotLoaded {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("engine never idle-unloaded: {:?}", engine.status());
}
