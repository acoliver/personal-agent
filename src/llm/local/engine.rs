//! Local LLM inference engine.
//!
//! Wraps the llama-gguf `ChatEngine` with a thread-based execution model
//! to prevent blocking the async runtime and GPUI.

use std::path::PathBuf;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{bounded, Receiver, Sender};

use crate::llm::local::capabilities::ModelCapabilities;
use crate::llm::local::error::{LocalModelError, LocalModelResult};
use crate::llm::{Message, StreamEvent, Tool};

/// Request sent to the inference thread.
pub struct InferenceRequest {
    /// Messages to generate response for.
    pub messages: Vec<Message>,
    /// Available tools (if any).
    pub tools: Option<Vec<Tool>>,
    /// Channel to send response events.
    pub event_tx: Sender<InferenceEvent>,
    /// System prompt override.
    pub system_prompt: Option<String>,
}

/// Events sent back from the inference thread.
#[derive(Debug, Clone)]
pub enum InferenceEvent {
    /// Text token generated.
    Token(String),
    /// Thinking content generated.
    Thinking(String),
    /// Tool call parsed from output.
    ToolCall {
        name: String,
        arguments: serde_json::Value,
    },
    /// Generation complete.
    Complete { total_tokens: usize },
    /// Error occurred.
    Error(String),
}

/// Local inference engine wrapper.
///
/// Uses a dedicated thread for inference to avoid blocking the async runtime.
pub struct LocalEngine {
    /// Sender for inference requests.
    request_tx: Option<Sender<InferenceRequest>>,
    /// Handle to the inference thread.
    thread_handle: Option<JoinHandle<()>>,
    /// Model capabilities.
    capabilities: ModelCapabilities,
    /// Path to the model file.
    model_path: PathBuf,
    /// Context window size.
    context_window: usize,
}

impl LocalEngine {
    /// Load a model and start the inference thread.
    ///
    /// # Arguments
    ///
    /// * `model_path` - Path to the GGUF model file.
    /// * `context_window` - Context window size in tokens.
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be loaded or the thread
    /// cannot be spawned.
    pub fn load(model_path: PathBuf, context_window: usize) -> LocalModelResult<Self> {
        let capabilities = ModelCapabilities::for_model("qwen3.5-4b");
        let context_window = capabilities.clamp_context(context_window);

        let (request_tx, request_rx) = bounded::<InferenceRequest>(4);

        let model_path_clone = model_path.clone();
        let thread_handle = thread::Builder::new()
            .name("local-llm-inference".to_string())
            .spawn(move || {
                if let Err(e) = run_inference_loop(&model_path_clone, context_window, &request_rx) {
                    tracing::error!("Inference thread error: {e}");
                }
            })
            .map_err(|e| LocalModelError::ThreadSpawnFailed(e.to_string()))?;

        Ok(Self {
            request_tx: Some(request_tx),
            thread_handle: Some(thread_handle),
            capabilities,
            model_path,
            context_window,
        })
    }

    /// Check if the engine is still running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.thread_handle
            .as_ref()
            .is_some_and(|h| !h.is_finished())
    }

    /// Get model capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &ModelCapabilities {
        &self.capabilities
    }

    /// Get the model path.
    #[must_use]
    pub fn model_path(&self) -> &std::path::Path {
        &self.model_path
    }

    /// Get context window size.
    #[must_use]
    pub const fn context_window(&self) -> usize {
        self.context_window
    }

    /// Generate a response (non-streaming).
    ///
    /// # Errors
    ///
    /// Returns an error if inference fails.
    pub fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Tool]>,
        system_prompt: Option<&str>,
    ) -> LocalModelResult<Message> {
        let request_tx = self
            .request_tx
            .as_ref()
            .ok_or(LocalModelError::ModelNotLoaded)?;

        let (event_tx, event_rx) = bounded::<InferenceEvent>(256);

        let request = InferenceRequest {
            messages: messages.to_vec(),
            tools: tools.map(<[Tool]>::to_vec),
            event_tx,
            system_prompt: system_prompt.map(str::to_string),
        };

        request_tx
            .send(request)
            .map_err(|e| LocalModelError::InferenceError(format!("Failed to send request: {e}")))?;

        // Collect all events
        let mut content = String::new();
        let mut thinking: Option<String> = None;
        let mut tool_uses = Vec::new();
        let mut error = None;

        while let Ok(event) = event_rx.recv() {
            match event {
                InferenceEvent::Token(tok) => content.push_str(&tok),
                InferenceEvent::Thinking(think) => {
                    thinking = Some(thinking.unwrap_or_default() + &think);
                }
                InferenceEvent::ToolCall { name, arguments } => {
                    use crate::llm::tools::ToolUse;
                    tool_uses.push(ToolUse::new(
                        uuid::Uuid::new_v4().to_string(),
                        &name,
                        arguments,
                    ));
                }
                InferenceEvent::Complete { .. } => break,
                InferenceEvent::Error(e) => {
                    error = Some(e);
                    break;
                }
            }
        }

        if let Some(e) = error {
            return Err(LocalModelError::InferenceError(e));
        }

        Ok(Message::assistant(content)
            .with_thinking(thinking.unwrap_or_default())
            .with_tool_uses(tool_uses))
    }

    /// Generate a response with streaming.
    ///
    /// Calls the provided callback for each generated token.
    ///
    /// # Errors
    ///
    /// Returns an error if inference fails.
    pub fn chat_stream<F>(
        &self,
        messages: &[Message],
        tools: Option<&[Tool]>,
        system_prompt: Option<&str>,
        mut on_event: F,
    ) -> LocalModelResult<()>
    where
        F: FnMut(StreamEvent) + Send,
    {
        let request_tx = self
            .request_tx
            .as_ref()
            .ok_or(LocalModelError::ModelNotLoaded)?;

        let (event_tx, event_rx) = bounded::<InferenceEvent>(256);

        let request = InferenceRequest {
            messages: messages.to_vec(),
            tools: tools.map(<[Tool]>::to_vec),
            event_tx,
            system_prompt: system_prompt.map(str::to_string),
        };

        request_tx
            .send(request)
            .map_err(|e| LocalModelError::InferenceError(format!("Failed to send request: {e}")))?;

        // Stream events to callback
        while let Ok(event) = event_rx.recv() {
            match event {
                InferenceEvent::Token(tok) => {
                    on_event(StreamEvent::TextDelta(tok));
                }
                InferenceEvent::Thinking(think) => {
                    on_event(StreamEvent::ThinkingDelta(think));
                }
                InferenceEvent::ToolCall { name, arguments } => {
                    use crate::llm::tools::ToolUse;
                    let tool_use = ToolUse::new(uuid::Uuid::new_v4().to_string(), &name, arguments);
                    on_event(StreamEvent::ToolUse(tool_use));
                }
                InferenceEvent::Complete { total_tokens } => {
                    on_event(StreamEvent::Complete {
                        input_tokens: None,
                        output_tokens: Some(u32::try_from(total_tokens).unwrap_or(u32::MAX)),
                    });
                    break;
                }
                InferenceEvent::Error(e) => {
                    on_event(StreamEvent::Error(e.clone()));
                    return Err(LocalModelError::InferenceError(e));
                }
            }
        }

        Ok(())
    }

    /// Unload the model and stop the inference thread.
    pub fn unload(&mut self) {
        // Drop the sender to signal the thread to stop
        self.request_tx = None;

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }

        tracing::info!("Local model unloaded");
    }
}

impl Drop for LocalEngine {
    fn drop(&mut self) {
        self.unload();
    }
}

/// Inference thread state - holds the loaded engine.
struct InferenceThreadState {
    /// `ChatEngine` handles automatic chat template formatting from GGUF metadata.
    chat_engine: llama_gguf::engine::ChatEngine,
    #[allow(dead_code)]
    context_window: usize,
}

impl InferenceThreadState {
    /// Load the model and create inference state.
    fn load(model_path: &std::path::Path, context_window: usize) -> LocalModelResult<Self> {
        tracing::info!(
            "Loading model from {} with {} token context",
            model_path.display(),
            context_window
        );

        // Configure engine with model path and context window
        let config = llama_gguf::engine::EngineConfig {
            model_path: model_path.to_string_lossy().to_string(),
            max_context_len: Some(context_window),
            temperature: 0.7,
            top_p: 0.9,
            top_k: 40,
            repeat_penalty: 1.1,
            max_tokens: 4096,
            seed: None,
            use_gpu: false,
            tokenizer_path: None,
            kv_cache_type: llama_gguf::model::KVCacheType::default(),
        };

        // Load the base engine from GGUF file
        let engine = llama_gguf::engine::Engine::load(config)
            .map_err(|e| LocalModelError::LoadFailed(e.to_string()))?;

        // Create ChatEngine with default system prompt (it will use GGUF's chat template)
        let chat_engine = llama_gguf::engine::ChatEngine::new(
            engine,
            Some("You are a helpful assistant. Respond clearly and concisely.".to_string()),
        );

        tracing::info!("Model loaded successfully with ChatEngine (auto chat template)");

        Ok(Self {
            chat_engine,
            context_window,
        })
    }

    /// Process a chat request.
    fn process_request(&mut self, request: &InferenceRequest) {
        // Format messages into a single prompt string for ChatEngine
        // ChatEngine handles the chat template automatically from GGUF metadata
        let prompt = Self::format_prompt(request);

        // Use chat_streaming which handles the chat template properly
        match self.chat_engine.chat_streaming(&prompt) {
            Ok(stream) => {
                let mut total_tokens = 0;
                let mut accumulated = String::new();

                for token_result in stream {
                    match token_result {
                        Ok(token) => {
                            accumulated.push_str(&token);
                            total_tokens += 1;

                            // Send content tokens
                            if !token.is_empty() {
                                let _ = request.event_tx.send(InferenceEvent::Token(token));
                            }

                            // Check for tool calls in accumulated output
                            if let Some(tool_call) = Self::parse_tool_call(&accumulated) {
                                let _ = request.event_tx.send(tool_call);
                            }
                        }
                        Err(e) => {
                            let _ = request.event_tx.send(InferenceEvent::Error(e.to_string()));
                            return;
                        }
                    }
                }

                let _ = request
                    .event_tx
                    .send(InferenceEvent::Complete { total_tokens });
            }
            Err(e) => {
                let _ = request.event_tx.send(InferenceEvent::Error(e.to_string()));
            }
        }
    }

    /// Format messages into prompt for `ChatEngine`.
    fn format_prompt(request: &InferenceRequest) -> String {
        // For ChatEngine, we just send the user's message
        // The engine handles the chat template from GGUF metadata
        // Find the last user message
        let mut prompt = String::new();

        for msg in &request.messages {
            if msg.role == crate::llm::client::Role::User {
                prompt.clone_from(&msg.content);
                break;
            }
        }

        // If tools are provided, append tool info to the prompt
        if let Some(ref tools) = request.tools {
            let tools_section = Self::format_tools(tools);
            prompt = format!("{prompt}\n\n{tools_section}");
        }

        prompt
    }

    /// Format tools for inclusion in prompt.
    fn format_tools(tools: &[Tool]) -> String {
        use std::fmt::Write;
        let mut output = String::new();
        output.push_str("Available tools (respond with JSON to use them):\n");

        for tool in tools {
            let _ = writeln!(output, "- {}: {}", tool.name, tool.description);
        }

        output.push_str(
            "\nTo call a tool, respond with: {\"name\": \"tool_name\", \"arguments\": {...}}\n",
        );
        output
    }

    /// Parse a tool call from accumulated output.
    fn parse_tool_call(output: &str) -> Option<InferenceEvent> {
        // Find JSON objects that look like tool calls
        if let Some(start) = output.find("{\"name\":") {
            if let Some(end) = Self::find_matching_brace(&output[start..]) {
                let content = &output[start..=start + end];

                if let Ok(json) = serde_json::from_str::<serde_json::Value>(content) {
                    if let (Some(name), Some(args)) = (
                        json.get("name").and_then(|n| n.as_str()),
                        json.get("arguments"),
                    ) {
                        return Some(InferenceEvent::ToolCall {
                            name: name.to_string(),
                            arguments: args.clone(),
                        });
                    }
                }
            }
        }
        None
    }

    /// Find the index of the matching closing brace.
    fn find_matching_brace(s: &str) -> Option<usize> {
        let mut depth = 0;
        for (i, c) in s.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        None
    }
}

/// Run the inference loop in a dedicated thread.
///
/// This function loads the model and processes incoming requests.
fn run_inference_loop(
    model_path: &std::path::Path,
    context_window: usize,
    request_rx: &Receiver<InferenceRequest>,
) -> LocalModelResult<()> {
    // Load model in this thread
    let mut state = InferenceThreadState::load(model_path, context_window)?;

    tracing::info!("Inference thread ready, waiting for requests");

    // Process requests until channel is closed
    while let Ok(request) = request_rx.recv() {
        state.process_request(&request);
    }

    tracing::info!("Inference thread exiting");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities() {
        let caps = ModelCapabilities::for_default();
        assert!(!caps.supports_tools);
        assert_eq!(caps.recommended_context, 32_768);
    }
}
