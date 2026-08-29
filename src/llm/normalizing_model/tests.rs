use super::*;
use futures::StreamExt as _;
use serdes_ai::core::messages::parts::{ThinkingPart, ToolCallArgs, ToolCallPart};
use serdes_ai::core::messages::{
    ModelRequestPart, ModelResponse, ModelResponsePart, ModelResponsePartDelta,
    ModelResponseStreamEvent,
};
use std::io::{Read as _, Write as _};
use std::net::TcpListener;

#[test]
fn convert_request_includes_reasoning_content_for_assistant_history() {
    let mut response = ModelResponse::new();
    response.add_part(ModelResponsePart::Thinking(ThinkingPart::new("step one")));
    response.add_part(ModelResponsePart::Text(
        serdes_ai::core::messages::parts::TextPart::new("final"),
    ));
    response.add_part(ModelResponsePart::ToolCall(
        ToolCallPart::new(
            "read_file",
            ToolCallArgs::json(serde_json::json!({ "path": "a" })),
        )
        .with_tool_call_id("call_1"),
    ));

    let mut request = ModelRequest::new();
    request.add_part(ModelRequestPart::ModelResponse(Box::new(response)));

    let converted = convert_request(&request);
    assert_eq!(converted.len(), 1);

    let assistant = &converted[0];
    assert_eq!(assistant.role, "assistant");
    assert_eq!(assistant.reasoning_content.as_deref(), Some("step one"));
    assert!(assistant
        .tool_calls
        .as_ref()
        .is_some_and(|calls| calls.len() == 1));
}

#[test]
fn build_chat_request_payload_serializes_reasoning_content_field() {
    let mut response = ModelResponse::new();
    response.add_part(ModelResponsePart::Thinking(ThinkingPart::new("chain")));
    response.add_part(ModelResponsePart::Text(
        serdes_ai::core::messages::parts::TextPart::new("answer"),
    ));

    let mut history_turn = ModelRequest::new();
    history_turn.add_part(ModelRequestPart::ModelResponse(Box::new(response)));

    let payload = build_chat_request_payload(
        "kimi-k2-0711-preview",
        &[history_turn],
        &ModelSettings::default(),
        &ModelRequestParameters::default(),
        true,
        Some(512),
        None,
        None,
    )
    .expect("payload should serialize");

    let messages = payload
        .get("messages")
        .and_then(serde_json::Value::as_array)
        .expect("messages array should be present");
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0]
            .get("reasoning_content")
            .and_then(serde_json::Value::as_str),
        Some("chain")
    );
}

#[test]
fn build_chat_request_payload_uses_configured_max_tokens_field_name() {
    let settings = ModelSettings {
        max_tokens: Some(2048),
        ..ModelSettings::default()
    };

    let payload = build_chat_request_payload(
        "gpt-4.1",
        &[],
        &settings,
        &ModelRequestParameters::default(),
        false,
        None,
        Some("max_completion_tokens"),
        None,
    )
    .expect("payload should serialize");

    assert_eq!(
        payload
            .get("max_completion_tokens")
            .and_then(serde_json::Value::as_u64),
        Some(2048)
    );
    assert!(payload.get("max_tokens").is_none());
}

#[test]
fn build_chat_request_payload_omits_token_limit_when_max_tokens_is_absent() {
    let payload = build_chat_request_payload(
        "gpt-4.1",
        &[],
        &ModelSettings::default(),
        &ModelRequestParameters::default(),
        false,
        None,
        Some("max_completion_tokens"),
        None,
    )
    .expect("payload should serialize");

    assert!(payload.get("max_tokens").is_none());
    assert!(payload.get("max_completion_tokens").is_none());
}

#[test]
fn resolve_token_field_uses_max_tokens_for_empty_string() {
    assert_eq!(resolve_token_field(Some("")), "max_tokens".to_string());
}

#[test]
fn resolve_token_field_uses_max_tokens_for_whitespace() {
    assert_eq!(resolve_token_field(Some("   ")), "max_tokens".to_string());
    assert_eq!(resolve_token_field(Some("	")), "max_tokens".to_string());
}

#[test]
fn resolve_token_field_keeps_explicit_standard_names() {
    assert_eq!(
        resolve_token_field(Some("max_tokens")),
        "max_tokens".to_string()
    );
    assert_eq!(
        resolve_token_field(Some("max_completion_tokens")),
        "max_completion_tokens".to_string()
    );
}

#[test]
fn resolve_token_field_uses_max_tokens_for_reserved_keys() {
    assert_eq!(resolve_token_field(Some("model")), "max_tokens".to_string());
    assert_eq!(
        resolve_token_field(Some("messages")),
        "max_tokens".to_string()
    );
    assert_eq!(
        resolve_token_field(Some("stream")),
        "max_tokens".to_string()
    );
    assert_eq!(resolve_token_field(Some("tools")), "max_tokens".to_string());
    assert_eq!(
        resolve_token_field(Some("temperature")),
        "max_tokens".to_string()
    );
}

#[test]
fn resolve_token_field_returns_custom_valid_override() {
    assert_eq!(
        resolve_token_field(Some("custom_tokens")),
        "custom_tokens".to_string()
    );
}

#[test]
fn resolve_token_field_trims_whitespace() {
    assert_eq!(
        resolve_token_field(Some("  custom_field  ")),
        "custom_field".to_string()
    );
}

// ------------------------------------------------------------------
// Streaming timeout semantics (issue #213)
// ------------------------------------------------------------------

/// Inner model stub; the wrapper's `request_stream` never delegates to it.
struct StubInnerModel {
    name: String,
    system: String,
    profile: ModelProfile,
}

#[async_trait]
impl Model for StubInnerModel {
    fn name(&self) -> &str {
        &self.name
    }

    fn system(&self) -> &str {
        &self.system
    }

    fn profile(&self) -> &ModelProfile {
        &self.profile
    }

    async fn request(
        &self,
        _messages: &[ModelRequest],
        _settings: &ModelSettings,
        _params: &ModelRequestParameters,
    ) -> Result<ModelResponse, ModelError> {
        Err(ModelError::NotSupported(
            "stub has no non-streaming path".to_string(),
        ))
    }

    async fn request_stream(
        &self,
        _messages: &[ModelRequest],
        _settings: &ModelSettings,
        _params: &ModelRequestParameters,
    ) -> Result<StreamedResponse, ModelError> {
        Err(ModelError::NotSupported(
            "stub has no streaming path".to_string(),
        ))
    }
}

fn build_wrapper(base_url: String, idle_read_timeout: Duration) -> NormalizingSseModel {
    NormalizingSseModel::new(NormalizingSseModelConfig {
        inner: Arc::new(StubInnerModel {
            name: "stub-inner".to_string(),
            system: String::new(),
            profile: ModelProfile::default(),
        }),
        default_headers: reqwest::header::HeaderMap::new(),
        idle_read_timeout,
        api_key: "test-key".to_string(),
        base_url,
        model_name: "stub-model".to_string(),
        enable_thinking: false,
        thinking_budget: None,
        max_tokens_field_name: None,
        extra_request_fields: None,
    })
    .expect("wrapper should build")
}

fn sse_data(payload: &str) -> String {
    format!("data: {payload}\n\n")
}

fn content_chunk(delta: &str) -> String {
    sse_data(
        &serde_json::json!({
            "id": "t", "object": "chat.completion.chunk", "created": 1,
            "model": "stub-model",
            "choices": [{"index": 0, "delta": {"content": delta}, "finish_reason": null}]
        })
        .to_string(),
    )
}

fn finish_chunk() -> String {
    sse_data(
        &serde_json::json!({
            "id": "t", "object": "chat.completion.chunk", "created": 1,
            "model": "stub-model",
            "choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}]
        })
        .to_string(),
    )
}

/// Spawn a raw TCP HTTP server that replies with a scripted SSE body:
/// each script entry sleeps for `delay`, then writes `bytes`.
fn spawn_sse_server(script: Vec<(Duration, String)>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
    let addr = listener.local_addr().expect("local addr");
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept request");
        drain_http_request(&mut stream);
        if stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            )
            .is_err()
        {
            return;
        }
        for (delay, chunk) in script {
            std::thread::sleep(delay);
            if stream.write_all(chunk.as_bytes()).is_err() {
                break;
            }
            let _ = stream.flush();
        }
        let _ = stream.shutdown(std::net::Shutdown::Both);
    });
    format!("http://{addr}")
}

/// Read and discard the incoming request (headers plus body) so the client
/// can finish sending before we respond.
fn drain_http_request(stream: &mut std::net::TcpStream) {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    let mut buf = [0u8; 4096];
    let mut seen = Vec::new();
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                seen.extend_from_slice(&buf[..n]);
                if request_fully_read(&seen) {
                    break;
                }
            }
        }
    }
}

fn request_fully_read(seen: &[u8]) -> bool {
    let Some(header_end) = seen.windows(4).position(|w| w == b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&seen[..header_end]).to_ascii_lowercase();
    let content_length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);
    seen.len() >= header_end + 4 + content_length
}

async fn collect_stream(
    wrapper: &NormalizingSseModel,
    settings: &ModelSettings,
) -> Vec<Result<ModelResponseStreamEvent, ModelError>> {
    let mut request = ModelRequest::new();
    request.add_user_prompt("build a report");
    let stream = wrapper
        .request_stream(&[request], settings, &ModelRequestParameters::default())
        .await
        .expect("request_stream should start");
    stream.collect().await
}

fn error_strings(events: &[Result<ModelResponseStreamEvent, ModelError>]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| event.as_ref().err().map(std::string::ToString::to_string))
        .collect()
}

fn text_deltas(events: &[Result<ModelResponseStreamEvent, ModelError>]) -> String {
    events
        .iter()
        .filter_map(|event| match event.as_ref().ok()? {
            ModelResponseStreamEvent::PartStart(start) => match &start.part {
                ModelResponsePart::Text(text) => Some(text.content.clone()),
                _ => None,
            },
            ModelResponseStreamEvent::PartDelta(delta) => match &delta.delta {
                ModelResponsePartDelta::Text(text) => Some(text.content_delta.clone()),
                _ => None,
            },
            ModelResponseStreamEvent::PartEnd(_) | ModelResponseStreamEvent::StreamComplete(_) => {
                None
            }
        })
        .collect()
}

#[tokio::test]
async fn stream_survives_total_deadline_while_chunks_keep_arriving() {
    // Total stream duration ≈ 1.2s with chunks every 150ms.
    let mut script = vec![(
        Duration::from_millis(10),
        sse_data(
            &serde_json::json!({
                "id": "t", "object": "chat.completion.chunk", "created": 1,
                "model": "stub-model",
                "choices": [{"index": 0, "delta": {"role": "assistant"}, "finish_reason": null}]
            })
            .to_string(),
        ),
    )];
    for letter in ["a", "b", "c", "d", "e", "f", "g", "h"] {
        script.push((Duration::from_millis(150), content_chunk(letter)));
    }
    script.push((Duration::from_millis(150), finish_chunk()));
    script.push((Duration::from_millis(10), sse_data("[DONE]")));

    let base_url = spawn_sse_server(script);
    let wrapper = build_wrapper(base_url, Duration::from_secs(5));

    // A total deadline shorter than the stream must not abort an actively
    // streaming response: only a silent gap may kill a stream.
    let settings = ModelSettings {
        timeout: Some(Duration::from_millis(400)),
        ..ModelSettings::default()
    };
    let events = collect_stream(&wrapper, &settings).await;

    let errors = error_strings(&events);
    assert!(
        errors.is_empty(),
        "actively streaming response was interrupted: {errors:?}"
    );
    assert_eq!(text_deltas(&events), "abcdefgh");
}

#[tokio::test]
async fn stream_fails_when_silent_beyond_idle_read_timeout() {
    let script = vec![
        (Duration::from_millis(10), content_chunk("a")),
        // Server goes silent far longer than the idle window.
        (Duration::from_secs(1), sse_data("[DONE]")),
    ];
    let base_url = spawn_sse_server(script);
    let wrapper = build_wrapper(base_url, Duration::from_millis(200));

    let events = collect_stream(&wrapper, &ModelSettings::default()).await;

    let errors = error_strings(&events);
    assert!(
        !errors.is_empty(),
        "silent stream should have failed with an error"
    );
}

#[test]
fn default_idle_read_timeout_is_five_minutes() {
    // The replacement for the old 2-minute total deadline must stay a
    // generous idle window, not regress into a whole-body cap.
    assert_eq!(DEFAULT_STREAM_IDLE_READ_TIMEOUT, Duration::from_mins(5));
}

#[test]
fn build_chat_request_payload_merges_extra_request_fields() {
    let extra_fields = serde_json::json!({
        "reasoning": {"effort": "medium"},
        "custom_param": "value"
    });

    let payload = build_chat_request_payload(
        "gpt-4.1",
        &[],
        &ModelSettings::default(),
        &ModelRequestParameters::default(),
        false,
        None,
        None,
        Some(&extra_fields),
    )
    .expect("payload should serialize");

    assert_eq!(
        payload
            .get("reasoning")
            .and_then(serde_json::Value::as_object)
            .and_then(|obj| obj.get("effort"))
            .and_then(serde_json::Value::as_str),
        Some("medium")
    );
    assert_eq!(
        payload
            .get("custom_param")
            .and_then(serde_json::Value::as_str),
        Some("value")
    );
}

#[test]
fn build_chat_request_payload_skips_reserved_keys_in_extra_fields() {
    let extra_fields = serde_json::json!({
        "model": "should-be-ignored",
        "messages": "should-be-ignored",
        "stream": false,
        "valid_key": "kept"
    });

    let payload = build_chat_request_payload(
        "gpt-4.1",
        &[],
        &ModelSettings::default(),
        &ModelRequestParameters::default(),
        false,
        None,
        None,
        Some(&extra_fields),
    )
    .expect("payload should serialize");

    // model and messages should not be overwritten by extra_fields
    assert_eq!(
        payload.get("model").and_then(serde_json::Value::as_str),
        Some("gpt-4.1")
    );
    assert!(payload.get("messages").is_some()); // original messages array
    assert_eq!(
        payload.get("stream").and_then(serde_json::Value::as_bool),
        Some(true)
    ); // default streaming
    assert_eq!(
        payload.get("valid_key").and_then(serde_json::Value::as_str),
        Some("kept")
    );
}

#[test]
fn build_chat_request_payload_uses_default_token_field_when_no_override() {
    let settings = ModelSettings {
        max_tokens: Some(1024),
        ..ModelSettings::default()
    };

    let payload = build_chat_request_payload(
        "gpt-4.1",
        &[],
        &settings,
        &ModelRequestParameters::default(),
        false,
        None,
        None,
        None,
    )
    .expect("payload should serialize");

    assert_eq!(
        payload
            .get("max_tokens")
            .and_then(serde_json::Value::as_u64),
        Some(1024)
    );
    assert!(payload.get("max_completion_tokens").is_none());

    let payload_thinking = build_chat_request_payload(
        "gpt-4.1",
        &[],
        &settings,
        &ModelRequestParameters::default(),
        true,
        None,
        None,
        None,
    )
    .expect("payload should serialize");

    assert_eq!(
        payload_thinking
            .get("max_tokens")
            .and_then(serde_json::Value::as_u64),
        Some(1024)
    );
    assert!(payload_thinking.get("max_completion_tokens").is_none());
}
