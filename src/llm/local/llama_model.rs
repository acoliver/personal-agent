//! `LocalLlamaModel`: a `serdes_ai::Model` backed by the in-process engine.
//!
//! Rendered prompts go straight to the actor; decoded output is re-parsed
//! into serdes stream events, with Granite's `<tool_call>` blocks surfacing as
//! complete `ToolUse` parts the moment their closing marker arrives. This
//! model is deliberately never wrapped in `NormalizingSseModel`: nothing here
//! is SSE, and the wrapper would try to repair an HTTP stream that does not
//! exist.
//!
// @plan:PLAN-20260903-LOCALMODEL.P02
// @requirement:REQ-LM-004

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use chrono::Utc;
use futures::Stream;
use serdes_ai::core::messages::{
    FinishReason, ModelRequest, ModelRequestPart, ModelResponse, ModelResponsePart,
    ModelResponsePartDelta, ModelResponseStreamEvent, RetryContent, StreamCompleteEvent, TextPart,
    ToolCallArgs, ToolCallPart, ToolReturnPart, UserContent, UserContentPart,
};
use serdes_ai::core::RequestUsage;
use serdes_ai::models::{Model, ModelRequestParameters, StreamedResponse};
use serdes_ai_models::error::ModelError;
use serdes_ai_models::profile::ModelProfile as SerdesModelProfile;

use super::generator::{AbortGuard, GenEvent, GenRequest, GenSampling, Generator};
use super::render::{render, ChatTurn, ToolSpec};
use super::toolcall::{parse_call_block, RawToolCall, TOOL_CALL_CLOSE, TOOL_CALL_OPEN};

/// Sampling temperature used when a profile does not pick one; the `PoC`'s
/// proven default.
const DEFAULT_TEMPERATURE: f64 = 0.1;
/// Token ceiling when a profile does not set `max_tokens`.
///
/// Large enough that an agent turn can stream a full answer plus tool calls;
/// also the floor of the output reserve the shared compression budget leaves
/// for the answer.
pub const DEFAULT_MAX_TOKENS: usize = 8192;
/// Bytes held back from text emission so a `<tool_call>` marker split across
/// decoded pieces can never leak into visible text.
const MARKER_HOLDBACK: usize = TOOL_CALL_OPEN.len() - 1;

/// The serdes `Model` view of the local engine.
pub struct LocalLlamaModel {
    generator: Arc<dyn Generator>,
    model_name: String,
    profile: SerdesModelProfile,
}

impl LocalLlamaModel {
    /// Builds a model over any [`Generator`], which is what makes the stream
    /// mapping testable without a GGUF.
    #[must_use]
    pub fn new(generator: Arc<dyn Generator>, model_name: impl Into<String>) -> Self {
        Self {
            generator,
            model_name: model_name.into(),
            profile: model_profile(),
        }
    }
}

/// Capability surface of the engine: tools and streaming yes, everything that
/// leaves the machine no.
fn model_profile() -> SerdesModelProfile {
    SerdesModelProfile {
        supports_tools: true,
        supports_parallel_tools: false,
        supports_native_structured_output: false,
        supports_strict_tools: false,
        supports_system_messages: true,
        supports_images: false,
        supports_streaming: true,
        ..SerdesModelProfile::default()
    }
}

/// Renders a serdes request into the Granite prompt.
fn render_prompt(messages: &[ModelRequest], params: &ModelRequestParameters) -> String {
    let tools: Vec<ToolSpec> = params
        .tools
        .iter()
        .map(|tool| ToolSpec {
            name: tool.name.clone(),
            description: tool.description.clone(),
            parameters_json: serde_json::to_string(&tool.parameters_json_schema)
                .unwrap_or_else(|_| "{}".to_string()),
        })
        .collect();

    let mut turns: Vec<ChatTurn> = Vec::new();
    for request in messages {
        for part in &request.parts {
            match part {
                ModelRequestPart::SystemPrompt(system) => {
                    turns.push(ChatTurn::System {
                        content: system.content.clone(),
                    });
                }
                ModelRequestPart::UserPrompt(prompt) => {
                    turns.push(ChatTurn::User {
                        content: user_text(&prompt.content),
                    });
                }
                ModelRequestPart::ToolReturn(ret) => {
                    push_tool_result(&mut turns, ret);
                }
                ModelRequestPart::RetryPrompt(retry) => {
                    turns.push(ChatTurn::User {
                        content: retry_text(&retry.content),
                    });
                }
                ModelRequestPart::ModelResponse(response) => {
                    push_assistant(&mut turns, response);
                }
                // Builtin returns (web search et al.) have no local tool; the
                // engine cannot have produced one to answer.
                ModelRequestPart::BuiltinToolReturn(_) => {}
            }
        }
    }

    render(&turns, &tools, true)
}

/// Concatenates the text parts of a user message.
fn user_text(content: &UserContent) -> String {
    match content {
        UserContent::Text(text) => text.clone(),
        UserContent::Parts(parts) => parts
            .iter()
            .filter_map(|part| match part {
                UserContentPart::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(""),
    }
}

/// Groups consecutive tool returns into one `<tool_response>` user turn, the
/// shape the template normalises history into.
fn push_tool_result(turns: &mut Vec<ChatTurn>, ret: &ToolReturnPart) {
    let content = ret.content.to_string_content();
    match turns.last_mut() {
        Some(ChatTurn::ToolResponses { results }) => results.push(content),
        _ => turns.push(ChatTurn::ToolResponses {
            results: vec![content],
        }),
    }
}

/// Stringifies a retry prompt so a failed tool call round still reaches the
/// model as instructions.
fn retry_text(content: &RetryContent) -> String {
    match content {
        RetryContent::Text(text) => text.clone(),
        RetryContent::Structured { message, errors } => errors.as_ref().map_or_else(
            || message.clone(),
            |errors| format!("{message}\nerrors: {}", errors.join(", ")),
        ),
    }
}

/// Rebuilds one assistant history turn from the stored response parts.
fn push_assistant(turns: &mut Vec<ChatTurn>, response: &ModelResponse) {
    let text = response
        .parts
        .iter()
        .filter_map(|part| match part {
            ModelResponsePart::Text(text) => Some(text.content.clone()),
            _ => None,
        })
        .collect::<String>();
    let calls = response
        .parts
        .iter()
        .filter_map(|part| match part {
            ModelResponsePart::ToolCall(call) => Some(RawToolCall {
                name: call.tool_name.clone(),
                arguments: args_to_pairs(&call.args),
            }),
            _ => None,
        })
        .collect::<Vec<_>>();
    if calls.is_empty() {
        turns.push(ChatTurn::Assistant { content: text });
    } else {
        turns.push(ChatTurn::AssistantToolCalls {
            rationale: (!text.is_empty()).then_some(text),
            calls,
        });
    }
}

/// Converts serdes tool-call arguments back to the raw string pairs the
/// renderer writes. Values stay strings; schema-driven coercion happens above
/// this layer when the tool actually runs.
fn args_to_pairs(args: &ToolCallArgs) -> Vec<(String, String)> {
    let value = match args {
        ToolCallArgs::Json(value) => value.clone(),
        ToolCallArgs::String(raw) => serde_json::from_str(raw).unwrap_or(serde_json::Value::Null),
    };
    match value {
        serde_json::Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| {
                let rendered = match value {
                    serde_json::Value::String(text) => text,
                    other => other.to_string(),
                };
                (key, rendered)
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Incremental re-parsing of the decoded text into serdes stream events.
///
/// Text outside tool-call blocks streams as per-token deltas; a block emits
/// one complete `ToolUse` part pair when `</tool_call>` closes. The parser is
/// block-based, so arguments stream to the consumer only as whole calls.
struct GenerationStream {
    events: Pin<Box<dyn Stream<Item = GenEvent> + Send>>,
    buffer: String,
    /// Scan cursor into `buffer` for the next marker search.
    scan: usize,
    /// Inside a `<tool_call>` block whose closer has not arrived.
    in_block: bool,
    out: VecDeque<ModelResponseStreamEvent>,
    text_index: Option<usize>,
    next_index: usize,
    tool_calls_emitted: usize,
    max_tokens: usize,
    finished: bool,
    failure: Option<String>,
    /// Held while the generation runs; dropping it before the stream ends
    /// would mark the generation cancelled in the engine and silently yield
    /// zero tokens. Released on normal completion so a post-stream drop
    /// cannot write a stale cancel entry.
    abort: Option<AbortGuard>,
}

impl GenerationStream {
    fn new(
        events: Pin<Box<dyn Stream<Item = GenEvent> + Send>>,
        max_tokens: usize,
        abort: AbortGuard,
    ) -> Self {
        Self {
            events,
            buffer: String::new(),
            scan: 0,
            in_block: false,
            out: VecDeque::new(),
            text_index: None,
            next_index: 0,
            tool_calls_emitted: 0,
            max_tokens,
            finished: false,
            failure: None,
            abort: Some(abort),
        }
    }

    /// Feeds one decoded piece through the block scanner.
    fn consume(&mut self, piece: &str) {
        self.buffer.push_str(piece);
        loop {
            if self.in_block {
                let Some(rel) = self.buffer[self.scan..].find(TOOL_CALL_CLOSE) else {
                    break;
                };
                let end = self.scan + rel;
                let body = self.buffer[self.scan..end].to_string();
                self.scan = end + TOOL_CALL_CLOSE.len();
                self.in_block = false;
                self.emit_tool_call(&body);
            } else if let Some(rel) = self.buffer[self.scan..].find(TOOL_CALL_OPEN) {
                let end = self.scan + rel;
                self.emit_text(self.scan..end);
                self.scan = end + TOOL_CALL_OPEN.len();
                self.in_block = true;
            } else {
                self.emit_safe_tail();
                break;
            }
        }
    }

    /// Emits decoded text as deltas, opening the text part on first use.
    fn emit_text(&mut self, range: std::ops::Range<usize>) {
        if range.start >= range.end {
            return;
        }
        let text_index = *self.text_index.get_or_insert_with(|| {
            let index = self.next_index;
            self.next_index += 1;
            self.out.push_back(ModelResponseStreamEvent::part_start(
                index,
                ModelResponsePart::Text(TextPart::new("")),
            ));
            index
        });
        self.out.push_back(ModelResponseStreamEvent::text_delta(
            text_index,
            &self.buffer[range],
        ));
    }

    /// Emits the text that cannot contain a marker opener yet.
    fn emit_safe_tail(&mut self) {
        let end = self.buffer.len();
        let mut safe_end = end.saturating_sub(MARKER_HOLDBACK);
        while safe_end > self.scan && !self.buffer.is_char_boundary(safe_end) {
            safe_end -= 1;
        }
        if safe_end > self.scan {
            self.emit_text(self.scan..safe_end);
            self.scan = safe_end;
        }
    }

    /// Parses one closed block and emits it as a complete tool-use part.
    fn emit_tool_call(&mut self, body: &str) {
        self.close_text_part();
        match parse_call_block(body) {
            Ok(call) => {
                let index = self.next_index;
                self.next_index += 1;
                self.tool_calls_emitted += 1;
                self.out.push_back(ModelResponseStreamEvent::part_start(
                    index,
                    ModelResponsePart::ToolCall(ToolCallPart::new(
                        call.name.clone(),
                        call_args(&call),
                    )),
                ));
                self.out
                    .push_back(ModelResponseStreamEvent::part_end(index));
            }
            Err(error) => {
                self.failure = Some(error.to_string());
            }
        }
    }

    fn close_text_part(&mut self) {
        if let Some(index) = self.text_index.take() {
            self.out
                .push_back(ModelResponseStreamEvent::part_end(index));
        }
    }

    /// Terminal flush: everything still buffered becomes text, then the
    /// completion event carries the token counts.
    fn finish(&mut self, prompt_tokens: usize, generated_tokens: usize) {
        // The actor finished the turn; dropping the guard here keeps a later
        // stream drop from inserting a dead generation into the cancel set.
        self.abort = None;
        if !self.in_block {
            let end = self.buffer.len();
            if end > self.scan {
                self.emit_text(self.scan..end);
            }
        }
        // An unterminated block keeps its text: the parse error is the model's
        // malformed output, but what it did write is still showable.
        self.close_text_part();
        let finish_reason = if self.tool_calls_emitted > 0 {
            FinishReason::ToolCall
        } else if generated_tokens >= self.max_tokens {
            FinishReason::Length
        } else {
            FinishReason::Stop
        };
        self.out.push_back(ModelResponseStreamEvent::StreamComplete(
            StreamCompleteEvent {
                finish_reason,
                input_tokens: u64::try_from(prompt_tokens).ok(),
                output_tokens: u64::try_from(generated_tokens).ok(),
                cache_creation_tokens: None,
                cache_read_tokens: None,
            },
        ));
        self.finished = true;
    }
}

/// Builds the JSON argument object for a parsed call.
fn call_args(call: &RawToolCall) -> ToolCallArgs {
    let mut map = serde_json::Map::new();
    for (key, value) in &call.arguments {
        map.insert(key.clone(), serde_json::Value::String(value.clone()));
    }
    ToolCallArgs::Json(serde_json::Value::Object(map))
}

impl Stream for GenerationStream {
    type Item = Result<ModelResponseStreamEvent, ModelError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(event) = self.out.pop_front() {
                return Poll::Ready(Some(Ok(event)));
            }
            if let Some(message) = self.failure.take() {
                self.finished = true;
                return Poll::Ready(Some(Err(ModelError::invalid_response(format!(
                    "malformed tool call from local model: {message}"
                )))));
            }
            if self.finished {
                return Poll::Ready(None);
            }
            match Pin::new(&mut self.events).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(GenEvent::Delta(piece))) => self.consume(&piece),
                Poll::Ready(Some(GenEvent::Complete {
                    prompt_tokens,
                    generated_tokens,
                })) => self.finish(prompt_tokens, generated_tokens),
                Poll::Ready(Some(GenEvent::Failed(message))) => {
                    self.finished = true;
                    return Poll::Ready(Some(Err(ModelError::invalid_response(message))));
                }
                // The actor always sends Complete or Failed before dropping
                // the channel; an early close means the thread died.
                Poll::Ready(None) => {
                    self.finished = true;
                    return Poll::Ready(Some(Err(ModelError::incomplete_stream(
                        "local generation ended without completion",
                    ))));
                }
            }
        }
    }
}

/// The engine ignores `settings.timeout` because there is no network hop: the
/// only slow phase is the one-time model load, and cancelling that would mean
/// dropping and reloading gigabytes on the next message.
#[async_trait]
impl Model for LocalLlamaModel {
    fn name(&self) -> &str {
        &self.model_name
    }

    fn system(&self) -> &str {
        super::LOCAL_PROVIDER_ID
    }

    async fn request(
        &self,
        messages: &[ModelRequest],
        settings: &serdes_ai::core::ModelSettings,
        params: &ModelRequestParameters,
    ) -> Result<ModelResponse, ModelError> {
        let mut stream = self.request_stream(messages, settings, params).await?;
        let mut parts = Vec::new();
        let mut finish_reason = None;
        let mut usage = None;
        while let Some(event) = futures::StreamExt::next(&mut stream).await {
            match event? {
                ModelResponseStreamEvent::PartStart(start) => parts.push(start.part),
                ModelResponseStreamEvent::PartDelta(delta) => {
                    if let ModelResponsePartDelta::Text(text) = delta.delta {
                        if let Some(ModelResponsePart::Text(existing)) = parts.last_mut() {
                            existing.content.push_str(&text.content_delta);
                        }
                    }
                }
                ModelResponseStreamEvent::PartEnd(_) => {}
                ModelResponseStreamEvent::StreamComplete(complete) => {
                    finish_reason = Some(complete.finish_reason);
                    usage = Some(RequestUsage {
                        request_tokens: complete.input_tokens,
                        response_tokens: complete.output_tokens,
                        total_tokens: match (complete.input_tokens, complete.output_tokens) {
                            (Some(input), Some(output)) => Some(input + output),
                            _ => None,
                        },
                        cache_creation_tokens: complete.cache_creation_tokens,
                        cache_read_tokens: complete.cache_read_tokens,
                        details: None,
                    });
                }
            }
        }
        Ok(ModelResponse {
            parts,
            model_name: Some(self.model_name.clone()),
            timestamp: Utc::now(),
            finish_reason,
            usage,
            vendor_id: None,
            vendor_details: None,
            kind: "response".to_string(),
        })
    }

    async fn request_stream(
        &self,
        messages: &[ModelRequest],
        settings: &serdes_ai::core::ModelSettings,
        params: &ModelRequestParameters,
    ) -> Result<StreamedResponse, ModelError> {
        let prompt = render_prompt(messages, params);
        let max_tokens = settings
            .max_tokens
            .and_then(|tokens| usize::try_from(tokens).ok())
            .unwrap_or(DEFAULT_MAX_TOKENS);
        let request = GenRequest {
            prompt,
            sampling: GenSampling {
                temperature: settings.temperature.unwrap_or(DEFAULT_TEMPERATURE),
                top_p: settings.top_p,
                seed: settings.seed,
            },
            max_tokens,
            stop: settings.stop.clone().unwrap_or_default(),
        };
        let generation = self
            .generator
            .generate(request.clone())
            .await
            .map_err(|error| ModelError::configuration(error.0))?;
        // The abort guard travels with the stream: leaving it in `generation`
        // cancels the generation the moment this function returns, and the
        // actor then stops before sampling its first token.
        // @requirement:REQ-LM-004
        let (_, events, abort) = generation.into_parts();
        Ok(Box::pin(GenerationStream::new(events, max_tokens, abort)))
    }

    fn profile(&self) -> &SerdesModelProfile {
        &self.profile
    }
}
