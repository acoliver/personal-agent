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
                run_inference_loop(&model_path_clone, context_window, &request_rx);
            })
            .map_err(|e| LocalModelError::ThreadSpawnFailed(e.to_string()))?;

        Ok(Self {
            request_tx: Some(request_tx),
            thread_handle: Some(thread_handle),
            capabilities,
            model_path,
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

    /// Generate a response (non-streaming).
    ///
    /// # Errors
    ///
    /// Returns an error if inference fails.
    pub fn chat(&self, messages: &[Message], tools: Option<&[Tool]>) -> LocalModelResult<Message> {
        let request_tx = self
            .request_tx
            .as_ref()
            .ok_or(LocalModelError::ModelNotLoaded)?;

        let (event_tx, event_rx) = bounded::<InferenceEvent>(256);

        let request = InferenceRequest {
            messages: messages.to_vec(),
            tools: tools.map(<[Tool]>::to_vec),
            event_tx,
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
                        output_tokens: u32::try_from(total_tokens).ok(),
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

/// Run the inference loop in a dedicated thread.
///
/// This function loads the model and processes incoming requests.
fn run_inference_loop(
    model_path: &std::path::Path,
    context_window: usize,
    request_rx: &Receiver<InferenceRequest>,
) {
    // For now, use a simple stub implementation
    // In production, this would use llama-gguf's ChatEngine

    tracing::info!(
        "Loading model from {} with {} token context",
        model_path.display(),
        context_window
    );

    // TODO: Replace with actual llama-gguf implementation
    // let file = llama_gguf::GgufFile::open(model_path)?;
    // let config = llama_gguf::engine::ChatEngineConfig {
    //     context_size: context_window,
    //     ..Default::default()
    // };
    // let engine = llama_gguf::engine::ChatEngine::from_file(model_path, config)?;

    tracing::info!("Model loaded, waiting for requests");

    // Process requests
    while let Ok(request) = request_rx.recv() {
        process_request(&request);
    }

    tracing::info!("Inference thread exiting");
}

/// Process a single inference request.
///
/// TODO: Replace with actual llama-gguf inference.
fn process_request(request: &InferenceRequest) {
    let _ = request.event_tx.send(InferenceEvent::Token(
        "Local inference not yet implemented. Model loaded successfully.".to_string(),
    ));
    let _ = request
        .event_tx
        .send(InferenceEvent::Complete { total_tokens: 10 });
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
