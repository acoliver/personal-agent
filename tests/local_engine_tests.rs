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
use personal_agent::llm::local::llama_model::{LocalLlamaModel, DEFAULT_MAX_TOKENS};
use personal_agent::llm::local::toolcall::TOOL_CALL_CLOSE;
use personal_agent::llm::local::{effective_context_window, effective_context_window_for};
use personal_agent::models::profile::{AuthConfig, ModelProfile};
use personal_agent::services::local_model_settings::LocalModelSettings;
use serdes_ai::core::messages::{
    BuiltinToolReturnContent, BuiltinToolReturnPart, RetryContent, RetryPromptPart,
    ToolReturnContent, ToolReturnPart,
};
use serdes_ai::core::messages::{
    FinishReason, ModelRequest, ModelRequestPart, ModelResponse, ModelResponsePart,
    ModelResponsePartDelta, ModelResponseStreamEvent, SystemPromptPart, TextPart, ToolCallArgs,
    ToolCallPart, UserContent, UserContentPart, UserPromptPart,
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

/// llama.cpp's backend is process-global, so the engine-spawning tests in
/// this binary must not race each other for it; the load-failure test needs
/// to WIN init to exercise the missing-file branch deterministically.
static ENGINE_SPAWN_LOCK: Mutex<()> = Mutex::new(());

/// A generator that shares its cancel set with the test and hands out a
/// never-ending stream, so abort-guard ownership is observable without a
/// GGUF and without waiting on generation events.
struct ObservableGenerator {
    cancelled: Arc<Mutex<HashSet<u64>>>,
}

#[async_trait::async_trait]
impl Generator for ObservableGenerator {
    async fn generate(&self, _request: GenRequest) -> Result<Generation, GenerateError> {
        Ok(Generation::new(
            42,
            Box::pin(futures::stream::pending::<GenEvent>()),
            Arc::clone(&self.cancelled),
        ))
    }

    fn status(&self) -> EngineStatus {
        EngineStatus::NotLoaded
    }

    async fn unload(&self) {}
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

/// The stream returned by `request_stream` must own the abort guard: a live
/// stream is not cancelled in the engine, and dropping it early is. The
/// regression: `request_stream` dropped the guard as soon as it returned, so
/// every real generation was cancelled before its first token and completed
/// cleanly with zero content.
// @requirement:REQ-LM-004
#[tokio::test]
async fn request_stream_holds_the_abort_guard_for_the_streams_lifetime() {
    let cancelled = Arc::new(Mutex::new(HashSet::new()));
    let model = LocalLlamaModel::new(
        Arc::new(ObservableGenerator {
            cancelled: Arc::clone(&cancelled),
        }),
        "granite-4.2-3b",
    );

    let stream = model
        .request_stream(
            &simple_request(),
            &serdes_ai::core::ModelSettings::default(),
            &ModelRequestParameters::default(),
        )
        .await
        .expect("stream starts");

    assert!(
        !cancelled.lock().expect("set").contains(&42),
        "a live stream must not be cancelled in the engine"
    );

    drop(stream);

    assert!(
        cancelled.lock().expect("set").contains(&42),
        "dropping the stream early must cancel the generation"
    );
}

/// A load failure (missing GGUF here) must surface as a Failed event and an
/// Error status, not a hang. Which engine thread wins the process-global
/// backend decides the message, so only the shape is asserted.
#[tokio::test]
// The guard is the point: llama.cpp's backend is process-global, so engine
// spawns must serialize across libtest threads; each test owns its own
// current-thread runtime, so holding it across awaits cannot self-deadlock.
#[allow(clippy::await_holding_lock)]
async fn engine_load_failure_fails_the_generation_and_sets_error_status() {
    let _serial = ENGINE_SPAWN_LOCK.lock().expect("engine spawn lock");
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
            GenEvent::Failed(message) => {
                saw_failure = true;
                // Whichever engine in this binary spawned first holds the
                // process-global backend; the loser drains with a
                // "backend init failed" message instead of reaching the
                // missing-file branch, so only one of the two is asserted.
                if !message.contains("backend init failed") {
                    assert!(
                        message
                            .contains("Local model file not found: /nonexistent/path/model.gguf"),
                        "missing-file failure must name the path, got: {message}"
                    );
                    assert!(
                        message.contains("Settings → Local Model"),
                        "missing-file failure must carry the fix, got: {message}"
                    );
                }
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

/// `shutdown` joins the actor deterministically: when it returns, the
/// receiver is gone, so new jobs fail fast instead of queueing forever. This
/// is the contract the exit-time hook relies on to quiesce llama.cpp before
/// its C++ static destructors run (process-exit SIGABRT regression).
// @requirement:REQ-LM-004
#[tokio::test]
// Same serialization as the load-failure test: see the note there.
#[allow(clippy::await_holding_lock)]
async fn engine_shutdown_joins_the_actor_and_fails_new_jobs() {
    let _serial = ENGINE_SPAWN_LOCK.lock().expect("engine spawn lock");
    let engine = EngineHandle::spawn();
    engine.shutdown();

    let result = engine.start_generation(
        GenRequest {
            prompt: "hi".to_string(),
            sampling: GenSampling {
                temperature: 0.1,
                top_p: None,
                seed: None,
            },
            max_tokens: 1,
            stop: Vec::new(),
        },
        EngineLoadSettings {
            model_path: PathBuf::from("/nonexistent/model.gguf"),
            n_ctx: 512,
            gpu_layers: 0,
            idle_unload: false,
            idle_timeout: Duration::from_secs(60),
        },
    );
    assert!(
        result.is_err(),
        "jobs after shutdown must fail fast, got a generation"
    );
}

/// Real-model end-to-end: load, generate a few tokens, then idle-unload.
/// Requires the Granite GGUF; see the file header for the invocation.
#[tokio::test]
// Same serialization as the load-failure test: see the note there.
#[allow(clippy::await_holding_lock)]
#[ignore = "loads the 3.6 GB GGUF and runs Metal inference"]
async fn real_model_generates_and_idle_unloads() {
    let _serial = ENGINE_SPAWN_LOCK.lock().expect("engine spawn lock");
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

// --- Effective context window (one budget, one pipeline) ---

/// A local-provider profile with adjustable sampler `max_tokens`.
fn local_profile(max_tokens: Option<u32>) -> ModelProfile {
    let mut profile = ModelProfile::new(
        "Granite (local)".to_string(),
        "local".to_string(),
        "granite-4.2-3b".to_string(),
        String::new(),
        AuthConfig::None,
    );
    profile.parameters.max_tokens = max_tokens;
    profile
}

/// Remote profiles keep exactly the profile's configured window; engine
/// settings must not touch them.
// @requirement:REQ-LM-001
#[test]
fn remote_profiles_keep_their_configured_window() {
    let mut profile = local_profile(None);
    profile.provider_id = "anthropic".to_string();
    profile.context_window_size = 200_000;
    let engine = LocalModelSettings {
        n_ctx: 8192,
        ..LocalModelSettings::default()
    };
    assert_eq!(effective_context_window_for(&profile, &engine), 200_000);
    assert_eq!(effective_context_window(&profile), 200_000);
}

/// Local: the engine window minus the output reserve, where the reserve is
/// the larger of the profile's `max_tokens` and the engine default; never an
/// underflow.
// @requirement:REQ-LM-001
#[test]
fn local_profiles_budget_against_the_engine_window_minus_the_output_reserve() {
    let engine_32k = LocalModelSettings {
        n_ctx: 32_768,
        ..LocalModelSettings::default()
    };

    // No profile max_tokens: reserve is the engine's output default.
    assert_eq!(
        effective_context_window_for(&local_profile(None), &engine_32k),
        32_768 - DEFAULT_MAX_TOKENS
    );

    // A larger profile max_tokens raises the reserve.
    assert_eq!(
        effective_context_window_for(&local_profile(Some(16_384)), &engine_32k),
        32_768 - 16_384
    );

    // A smaller profile max_tokens never lowers the reserve below default.
    assert_eq!(
        effective_context_window_for(&local_profile(Some(1024)), &engine_32k),
        32_768 - DEFAULT_MAX_TOKENS
    );

    // Reserve larger than the window saturates at zero instead of panicking.
    let tiny = LocalModelSettings {
        n_ctx: 4096,
        ..LocalModelSettings::default()
    };
    assert_eq!(
        effective_context_window_for(&local_profile(Some(16_384)), &tiny),
        0
    );
}

/// The chat `compress()` budget: a long local-profile conversation must hit
/// the shared pipeline under the engine-derived window, where the profile's
/// 128k default would have waved it through (the original overflow).
// @requirement:REQ-LM-001
#[test]
fn local_budget_routes_long_conversations_into_the_shared_pipeline() {
    use personal_agent::compression::pipeline::CompressionPipeline;
    use personal_agent::config::CompressionConfig;
    use personal_agent::llm::Message as LlmMessage;
    use personal_agent::models::CompressionPhase;

    let profile = local_profile(Some(4096));
    let engine = LocalModelSettings {
        n_ctx: 32_768,
        ..LocalModelSettings::default()
    };
    let budget = effective_context_window_for(&profile, &engine);

    // About 24k estimated tokens (cl100k): past the local engine budget's
    // truncation threshold, but far below even the observation-mask
    // threshold of the profile's 128k default, so only the local budget
    // compresses.
    let filler = "tool output line ".repeat(1_000);
    let messages: Vec<LlmMessage> = (0..6).map(|_| LlmMessage::user(filler.clone())).collect();

    let compressed = CompressionPipeline::new().compress(
        messages.clone(),
        budget,
        &CompressionConfig::default(),
    );
    assert_ne!(
        compressed.phase,
        CompressionPhase::None,
        "the engine-derived window must trigger compression"
    );

    let passthrough = CompressionPipeline::new().compress(
        messages,
        profile.context_window_size,
        &CompressionConfig::default(),
    );
    assert_eq!(
        passthrough.phase,
        CompressionPhase::None,
        "the profile's 128k default must not compress (this was the overflow)"
    );
}

/// The context-overflow regression: a prompt larger than `n_ctx` used to
/// SIGABRT the whole process inside `llama_decode`'s prefill assert
/// (`GGML_ASSERT(n_tokens_all <= cparams.n_batch)`). It must instead fail
/// the generation with an actionable message and leave the process alive.
///
/// Requires the Granite GGUF; see the file header for the invocation.
// @requirement:REQ-LM-003
#[tokio::test]
// Same serialization as the load-failure test: see the note there.
#[allow(clippy::await_holding_lock)]
#[ignore = "loads the 3.6 GB GGUF and runs Metal inference"]
async fn oversized_prompt_fails_the_generation_without_killing_the_process() {
    let _serial = ENGINE_SPAWN_LOCK.lock().expect("engine spawn lock");
    let Some(path) = std::env::var_os("PA_LOCAL_GGUF").map(PathBuf::from) else {
        panic!("PA_LOCAL_GGUF must point at the Granite GGUF");
    };
    let engine = EngineHandle::spawn();
    let settings = EngineLoadSettings {
        model_path: path,
        n_ctx: 2048,
        gpu_layers: 999,
        idle_unload: false,
        idle_timeout: Duration::from_secs(60),
    };
    // ~40k tokens against a 2048-token context: far past the old prefill
    // assert, deliberately bypassing every compression layer.
    let oversized_prompt = "context overflow filler ".repeat(10_000);
    let generation = engine
        .start_generation(
            GenRequest {
                prompt: oversized_prompt,
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

    let mut events = generation.events;
    let mut failure = None;
    while let Some(event) = events.next().await {
        match event {
            GenEvent::Failed(message) => {
                failure = Some(message);
                break;
            }
            GenEvent::Delta(_) => {}
            GenEvent::Complete { .. } => panic!("an oversized prompt must not generate"),
        }
    }
    let message = failure.expect("the generation must fail with the actionable message");
    assert!(
        message.contains("conversation exceeds the local context window"),
        "failure must name the overflow, got: {message}"
    );
    assert!(
        message.contains("tokens > 2048"),
        "failure must carry N/M token counts, got: {message}"
    );
    assert!(
        message.contains("raise Context size in Settings"),
        "failure must carry the fix, got: {message}"
    );

    // The process (and engine) survived: the actor still reports a coherent
    // status after declining the generation.
    assert!(
        matches!(
            engine.status(),
            EngineStatus::Loaded { .. } | EngineStatus::NotLoaded
        ),
        "engine must stay alive after an oversized prompt: {:?}",
        engine.status()
    );
}

// ── engine actor job plumbing (Load/Unload/Shutdown/Drop), no GGUF needed ──

/// Settings naming a GGUF that does not exist, so an actor that wins the
/// backend init still never loads weights.
fn missing_model_settings(path: &str) -> EngineLoadSettings {
    EngineLoadSettings {
        model_path: PathBuf::from(path),
        n_ctx: 512,
        gpu_layers: 0,
        idle_unload: false,
        idle_timeout: Duration::from_secs(60),
    }
}

/// Both load-failure shapes settle in `Error`: a won init reports the missing
/// file, a lost init reports the drained backend. Bounded so a wedged actor
/// fails the test instead of hanging it.
fn wait_for_error_status(engine: &EngineHandle) -> String {
    for _ in 0..300 {
        if let EngineStatus::Error { message } = engine.status() {
            return message;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("status never reached Error: {:?}", engine.status());
}

/// A `Load` job against a missing GGUF must fail with an actionable message
/// and push the engine into `Error` — never hang, never panic.
#[tokio::test]
// The guard is the point: llama.cpp's backend is process-global, so engine
// spawns must serialize across libtest threads; each test owns its own
// current-thread runtime, so holding it across awaits cannot self-deadlock.
#[allow(clippy::await_holding_lock)]
async fn engine_load_job_reports_missing_file_and_error_status() {
    let _serial = ENGINE_SPAWN_LOCK.lock().expect("engine spawn lock");
    let engine = EngineHandle::spawn();

    let message = engine
        .load(missing_model_settings("/nonexistent/load-job/model.gguf"))
        .await
        .expect_err("a missing GGUF must fail the load job");
    if !message.contains("backend init failed") {
        assert!(
            message.contains("Local model file not found: /nonexistent/load-job/model.gguf"),
            "missing-file failure must name the path, got: {message}"
        );
        assert!(
            message.contains("Settings → Local Model"),
            "missing-file failure must carry the fix, got: {message}"
        );
    }

    let status_message = wait_for_error_status(&engine);
    assert!(
        status_message.contains("not found") || status_message.contains("backend init failed"),
        "error status must explain the failure, got: {status_message}"
    );
    engine.shutdown();
}

/// An `Unload` with nothing resident must be absorbed silently: the actor
/// stays alive and still answers the next job in order.
#[tokio::test]
// Same serialization as the load-failure test: see the note there.
#[allow(clippy::await_holding_lock)]
async fn engine_unload_before_any_load_keeps_the_actor_serving_jobs() {
    let _serial = ENGINE_SPAWN_LOCK.lock().expect("engine spawn lock");
    let engine = EngineHandle::spawn();
    engine.request_unload();

    let message = engine
        .load(missing_model_settings("/nonexistent/after-unload.gguf"))
        .await
        .expect_err("the follow-up load must still fail on the missing file");
    assert!(
        message.contains("Local model file not found") || message.contains("backend init failed"),
        "the actor must survive the unload and answer the next job, got: {message}"
    );
    engine.shutdown();
}

/// Whoever holds the process-global llama backend, any *second* actor
/// deterministically loses init: it must report the failure once, answer
/// every queued job with it (Load reply, Generate event), absorb Unload,
/// and exit promptly on Shutdown.
#[tokio::test]
// Same serialization as the load-failure test: see the note there.
#[allow(clippy::await_holding_lock)]
async fn a_second_engine_drains_all_jobs_after_losing_backend_init() {
    let _serial = ENGINE_SPAWN_LOCK.lock().expect("engine spawn lock");
    // Answering one job proves some actor owns the backend (this one or an
    // earlier test's); the flag never resets, so the next spawn must lose.
    let first = EngineHandle::spawn();
    let _ = first
        .load(missing_model_settings("/nonexistent/init-holder.gguf"))
        .await;

    let drained = EngineHandle::spawn();
    let message = wait_for_error_status(&drained);
    assert!(
        message.contains("backend init failed"),
        "a losing actor must report the init failure, got: {message}"
    );

    // Load: the drain answers with the same failure message.
    let load_error = drained
        .load(missing_model_settings("/nonexistent/drained.gguf"))
        .await
        .expect_err("a drained load must fail");
    assert_eq!(load_error, message);

    // Generate: the drain answers through the event channel, then closes it.
    let generation = drained
        .start_generation(
            GenRequest {
                prompt: "hi".to_string(),
                sampling: GenSampling {
                    temperature: 0.1,
                    top_p: None,
                    seed: None,
                },
                max_tokens: 1,
                stop: Vec::new(),
            },
            missing_model_settings("/nonexistent/drained-too.gguf"),
        )
        .expect("the job is accepted even while drained");
    let mut events = generation.events;
    match events.next().await {
        Some(GenEvent::Failed(failure)) => assert_eq!(failure, message),
        other => panic!("expected the drained failure event, got {other:?}"),
    }
    assert!(events.next().await.is_none(), "the channel must close");

    // Unload: absorbed silently by the drain.
    drained.request_unload();

    // Shutdown: ends the drain and joins; later jobs fail fast.
    drained.shutdown();
    let late = drained
        .load(missing_model_settings("/nonexistent/late.gguf"))
        .await
        .expect_err("jobs after shutdown must fail fast");
    assert_eq!(late, "local model engine thread is gone");
}

/// `shutdown` joins the actor, so a `Load` sent afterwards fails fast with
/// the thread-gone message instead of queueing forever.
#[tokio::test]
// Same serialization as the load-failure test: see the note there.
#[allow(clippy::await_holding_lock)]
async fn engine_load_after_shutdown_reports_the_thread_gone() {
    let _serial = ENGINE_SPAWN_LOCK.lock().expect("engine spawn lock");
    let engine = EngineHandle::spawn();
    engine.shutdown();

    let message = engine
        .load(missing_model_settings("/nonexistent/late.gguf"))
        .await
        .expect_err("a load after shutdown must fail");
    assert_eq!(message, "local model engine thread is gone");
}

/// The exit-time contract: dropping the last handle quiesces the engine
/// (raises the flag, tells the actor to exit, joins) instead of leaving
/// llama.cpp state to C++ static teardown.
#[tokio::test]
// Same serialization as the load-failure test: see the note there.
#[allow(clippy::await_holding_lock)]
async fn dropping_the_last_handle_joins_the_actor() {
    let _serial = ENGINE_SPAWN_LOCK.lock().expect("engine spawn lock");
    let engine = EngineHandle::spawn();
    let drop_completed = tokio::time::timeout(Duration::from_secs(10), async move {
        drop(engine);
    })
    .await;
    assert!(
        drop_completed.is_ok(),
        "dropping the last handle must quiesce the actor without hanging"
    );
}

// ── serdes → GenRequest mapping (capturing generator) ──────────────────────

/// A generator that records the mapped `GenRequest` and replays a fixed event
/// list, making the serdes settings mapping observable without an engine.
#[derive(Clone)]
struct CapturingGenerator {
    events: Vec<GenEvent>,
    captured: Arc<Mutex<Option<GenRequest>>>,
    fail_start: bool,
}

#[async_trait::async_trait]
impl Generator for CapturingGenerator {
    async fn generate(&self, request: GenRequest) -> Result<Generation, GenerateError> {
        if self.fail_start {
            return Err(GenerateError(
                "local model engine thread is gone".to_string(),
            ));
        }
        *self.captured.lock().expect("captured request") = Some(request);
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

fn capturing_model(events: Vec<GenEvent>) -> (LocalLlamaModel, Arc<Mutex<Option<GenRequest>>>) {
    let captured = Arc::new(Mutex::new(None));
    (
        LocalLlamaModel::new(
            Arc::new(CapturingGenerator {
                events,
                captured: Arc::clone(&captured),
                fail_start: false,
            }),
            "granite-4.2-3b",
        ),
        captured,
    )
}

fn captured_request(captured: &Arc<Mutex<Option<GenRequest>>>) -> GenRequest {
    captured
        .lock()
        .expect("captured request")
        .take()
        .expect("the generator must receive the mapped request")
}

/// Every sampler knob, the token ceiling, and the stop strings must travel
/// from serdes settings onto the engine request; the prompt must be the
/// rendered Granite conversation with the tool scaffold included.
#[tokio::test]
async fn request_stream_maps_serdes_settings_and_tools_onto_the_gen_request() {
    let (model, captured) = capturing_model(vec![GenEvent::Complete {
        prompt_tokens: 1,
        generated_tokens: 0,
    }]);
    let settings = serdes_ai::core::ModelSettings {
        max_tokens: Some(128),
        temperature: Some(0.7),
        top_p: Some(0.9),
        seed: Some(7),
        stop: Some(vec!["DONE".to_string()]),
        ..serdes_ai::core::ModelSettings::default()
    };
    let params =
        ModelRequestParameters::default().with_tools(vec![serdes_ai_tools::ToolDefinition::new(
            "get_weather",
            "Look up weather",
        )
        .with_parameters(serde_json::json!({
            "type": "object",
            "properties": {"city": {"type": "string"}}
        }))]);

    let mut stream = model
        .request_stream(&simple_request(), &settings, &params)
        .await
        .expect("stream starts");
    while let Some(event) = stream.next().await {
        event.expect("stream event");
    }

    let request = captured_request(&captured);
    assert_eq!(request.max_tokens, 128);
    assert!((request.sampling.temperature - 0.7).abs() < 1e-12);
    assert_eq!(request.sampling.top_p, Some(0.9));
    assert_eq!(request.sampling.seed, Some(7));
    assert_eq!(request.stop, vec!["DONE".to_string()]);
    assert!(
        request.prompt.starts_with("<|im_start|>system\nbe brief"),
        "the prompt must render the conversation, got: {}",
        request.prompt
    );
    assert!(
        request
            .prompt
            .ends_with("<|im_start|>assistant\n<think></think>"),
        "the prompt must end on the open assistant header"
    );
    assert!(
        request.prompt.contains("\"name\":\"get_weather\""),
        "tool schemas must travel inside the system scaffold"
    );
}

/// Empty serdes settings fall back to the engine defaults: the 8192 token
/// ceiling and the `PoC`'s `0.1` temperature.
#[tokio::test]
async fn request_stream_falls_back_to_engine_defaults_without_settings() {
    let (model, captured) = capturing_model(vec![GenEvent::Complete {
        prompt_tokens: 1,
        generated_tokens: 0,
    }]);

    collect_events_with(
        &model,
        &simple_request(),
        &serdes_ai::core::ModelSettings::default(),
    )
    .await;

    let request = captured_request(&captured);
    assert_eq!(request.max_tokens, DEFAULT_MAX_TOKENS);
    assert!((request.sampling.temperature - 0.1).abs() < 1e-12);
    assert_eq!(request.sampling.top_p, None);
    assert_eq!(request.sampling.seed, None);
    assert!(request.stop.is_empty());
}

/// A generator that cannot start (engine gone) surfaces as a configuration
/// error, not as a broken stream.
#[tokio::test]
async fn a_failed_generation_start_surfaces_as_a_configuration_error() {
    let captured = Arc::new(Mutex::new(None));
    let model = LocalLlamaModel::new(
        Arc::new(CapturingGenerator {
            events: Vec::new(),
            captured,
            fail_start: true,
        }),
        "granite-4.2-3b",
    );
    let Err(error) = model
        .request_stream(
            &simple_request(),
            &serdes_ai::core::ModelSettings::default(),
            &ModelRequestParameters::default(),
        )
        .await
    else {
        panic!("an unreachable generator must fail the stream start");
    };
    assert!(error.to_string().contains("engine thread is gone"));
}

// ── stream terminal states and finish reasons ─────────────────────────────

/// A generation that fills its token budget must report `Length`, not `Stop`.
#[tokio::test]
async fn generation_that_fills_max_tokens_finishes_with_length() {
    let model = scripted_model(vec![
        GenEvent::Delta("a".to_string()),
        GenEvent::Delta("b".to_string()),
        GenEvent::Delta("c".to_string()),
        GenEvent::Complete {
            prompt_tokens: 4,
            generated_tokens: 3,
        },
    ]);
    let settings = serdes_ai::core::ModelSettings {
        max_tokens: Some(3),
        ..serdes_ai::core::ModelSettings::default()
    };

    let events = collect_events_with(&model, &simple_request(), &settings).await;
    let Some(ModelResponseStreamEvent::StreamComplete(complete)) = events.last() else {
        panic!("expected a completion event, got {events:?}");
    };
    assert_eq!(complete.finish_reason, FinishReason::Length);
    assert_eq!(complete.output_tokens, Some(3));
    // The tail held back by the marker window flushes with the part close.
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
    assert_eq!(text, "abc");
}

/// A structurally broken tool-call block must end the stream with a typed
/// error naming the malformed output.
#[tokio::test]
async fn malformed_tool_call_block_surfaces_as_a_stream_error() {
    let model = scripted_model(vec![GenEvent::Delta(
        "<tool_call>\nnot a function\n</tool_call>".to_string(),
    )]);

    let settings = serdes_ai::core::ModelSettings::default();
    let mut stream = model
        .request_stream(
            &simple_request(),
            &settings,
            &ModelRequestParameters::default(),
        )
        .await
        .expect("stream starts");
    let error = stream
        .next()
        .await
        .expect("an event")
        .expect_err("a parse failure");
    assert!(
        error
            .to_string()
            .contains("malformed tool call from local model"),
        "got: {error}"
    );
}

/// The actor always sends a terminal event; an early channel close means the
/// thread died and must surface as an incomplete-stream error, not silence.
#[tokio::test]
async fn stream_closed_without_completion_is_an_error() {
    let model = scripted_model(Vec::new());

    let settings = serdes_ai::core::ModelSettings::default();
    let mut stream = model
        .request_stream(
            &simple_request(),
            &settings,
            &ModelRequestParameters::default(),
        )
        .await
        .expect("stream starts");
    let error = stream
        .next()
        .await
        .expect("an event")
        .expect_err("an early close");
    assert!(error.to_string().contains("ended without completion"));
}

/// An unterminated `<tool_call>` block drops its own text (it is malformed
/// output) but the prose before it stays visible and the stream completes
/// cleanly.
#[tokio::test]
async fn unterminated_tool_call_block_keeps_the_prior_prose_only() {
    let model = scripted_model(vec![
        GenEvent::Delta("before ".to_string()),
        GenEvent::Delta("<tool_call>\n<function=ping>\nnever closed".to_string()),
        GenEvent::Complete {
            prompt_tokens: 3,
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
    assert_eq!(text, "before ");
    let Some(ModelResponseStreamEvent::StreamComplete(complete)) = events.last() else {
        panic!("expected a completion event, got {events:?}");
    };
    assert_eq!(complete.finish_reason, FinishReason::Stop);
    assert!(!text.contains("<tool_call>"), "markers never leak: {text}");
}

// ── prompt rendering: every serdes request part shape ─────────────────────

/// Every `ModelRequestPart` variant must land in the rendered prompt in the
/// template's shapes: history responses become assistant turns (text prose
/// plus `<tool_call>` blocks), consecutive tool returns share one user turn,
/// retries become user instructions, builtin returns are dropped, and
/// multi-part user content contributes its text parts only.
#[tokio::test]
async fn rendered_prompt_covers_every_request_part_shape() {
    let (model, captured) = capturing_model(vec![GenEvent::Complete {
        prompt_tokens: 1,
        generated_tokens: 0,
    }]);

    let history = ModelResponse::with_parts(vec![
        ModelResponsePart::Text(TextPart::new("I will check the weather.")),
        ModelResponsePart::ToolCall(ToolCallPart::new(
            "get_weather",
            ToolCallArgs::Json(serde_json::json!({"city": "Paris"})),
        )),
    ]);
    // String-form args parse back into parameters; unparseable args degrade
    // to a parameterless call instead of being dropped.
    let string_args = ModelResponse::with_parts(vec![ModelResponsePart::ToolCall(
        ToolCallPart::new("ping", ToolCallArgs::String("{\"k\": \"v\"}".to_string())),
    )]);
    let broken_args = ModelResponse::with_parts(vec![ModelResponsePart::ToolCall(
        ToolCallPart::new("note", ToolCallArgs::String("not json".to_string())),
    )]);

    let request = vec![ModelRequest {
        parts: vec![
            ModelRequestPart::SystemPrompt(SystemPromptPart::new("be brief")),
            ModelRequestPart::UserPrompt(UserPromptPart::new(UserContent::text("hello there"))),
            ModelRequestPart::ModelResponse(Box::new(history)),
            ModelRequestPart::ToolReturn(ToolReturnPart::new(
                "get_weather",
                ToolReturnContent::text("sunny"),
            )),
            ModelRequestPart::ToolReturn(ToolReturnPart::new(
                "get_weather",
                ToolReturnContent::text("rainy"),
            )),
            ModelRequestPart::ModelResponse(Box::new(string_args)),
            ModelRequestPart::ModelResponse(Box::new(broken_args)),
            ModelRequestPart::RetryPrompt(RetryPromptPart::new(RetryContent::text("try again"))),
            ModelRequestPart::RetryPrompt(RetryPromptPart::new(RetryContent::structured(
                "bad output",
                Some(vec!["missing city".to_string()]),
            ))),
            ModelRequestPart::BuiltinToolReturn(BuiltinToolReturnPart::new(
                "web_search",
                BuiltinToolReturnContent::Other {
                    kind: "test".to_string(),
                    data: serde_json::json!({}),
                },
                "call-1",
            )),
            ModelRequestPart::UserPrompt(UserPromptPart::new(UserContent::Parts(vec![
                UserContentPart::text("see this"),
                UserContentPart::image_url("https://example.invalid/cat.png"),
            ]))),
        ],
        ..ModelRequest::default()
    }];

    collect_events_with(&model, &request, &serdes_ai::core::ModelSettings::default()).await;
    let prompt = captured_request(&captured).prompt;

    assert!(prompt.contains("<|im_start|>system\nbe brief<|im_end|>\n"));
    assert!(prompt.contains("<|im_start|>user\nhello there<|im_end|>\n"));
    assert!(
        prompt.contains(
            "<|im_start|>assistant\n<think></think>I will check the weather.\n<tool_call>\n\
             <function=get_weather>\n<parameter=city>\nParis\n</parameter>\n</function>\n\
             </tool_call>\n<|im_end|>\n"
        ),
        "history must render as an assistant tool-call turn, got: {prompt}"
    );
    assert!(
        prompt.contains(
            "<|im_start|>user\n<tool_response>\nsunny\n</tool_response>\n\
             <tool_response>\nrainy\n</tool_response>\n<|im_end|>\n"
        ),
        "consecutive tool returns must share one user turn, got: {prompt}"
    );
    assert!(
        prompt.contains("<function=ping>\n<parameter=k>\nv\n</parameter>\n</function>"),
        "string-form args must parse back into parameters, got: {prompt}"
    );
    assert!(
        prompt.contains("<function=note>\n</function>"),
        "unparseable args must degrade to a parameterless call, got: {prompt}"
    );
    assert!(prompt.contains("<|im_start|>user\ntry again<|im_end|>\n"));
    assert!(
        prompt.contains("bad output\nerrors: missing city"),
        "structured retries must carry their errors, got: {prompt}"
    );
    assert!(prompt.contains("<|im_start|>user\nsee this<|im_end|>\n"));
    assert!(!prompt.contains("cat.png"), "non-text parts are dropped");
    assert!(
        !prompt.contains("web_search"),
        "builtin returns are dropped"
    );
}

/// Variant of `collect_events` that lets a test pin the request settings.
async fn collect_events_with(
    model: &LocalLlamaModel,
    messages: &[ModelRequest],
    settings: &serdes_ai::core::ModelSettings,
) -> Vec<ModelResponseStreamEvent> {
    let mut stream = model
        .request_stream(messages, settings, &ModelRequestParameters::default())
        .await
        .expect("stream starts");
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event.expect("stream event"));
    }
    events
}
