//! Local LLM provider implementation.
//!
//! Integrates the local inference engine with the `LlmClient` interface,
//! allowing seamless switching between cloud and local providers.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::llm::local::capabilities::ModelCapabilities;
use crate::llm::local::engine::LocalEngine;
use crate::llm::local::error::{LocalModelError, LocalModelResult};
use crate::llm::local::hardware::HardwareCapabilities;
use crate::llm::local::model_manager::LocalModelManager;
use crate::llm::{LlmError, Message, StreamEvent, Tool};

/// Local model provider that runs in-process.
///
/// Wraps the `LocalEngine` with lazy loading - the model is only
/// loaded into memory when first used.
pub struct LocalProvider {
    /// Lazily-loaded inference engine.
    engine: Arc<Mutex<Option<LocalEngine>>>,
    /// Path to the model file.
    model_path: PathBuf,
    /// Context window size.
    context_window: usize,
    /// Model capabilities.
    capabilities: ModelCapabilities,
}

impl LocalProvider {
    /// Create a new local provider.
    ///
    /// The model is NOT loaded until the first request is made.
    /// Use `ensure_loaded()` to preload the model.
    ///
    /// # Arguments
    ///
    /// * `model_path` - Path to the GGUF model file.
    /// * `context_window` - Context window size in tokens.
    ///
    /// # Errors
    ///
    /// Returns an error if the model file does not exist.
    pub fn new(model_path: PathBuf, context_window: usize) -> LocalModelResult<Self> {
        if !model_path.exists() {
            return Err(LocalModelError::ModelNotFound(model_path));
        }

        let capabilities = ModelCapabilities::for_model("qwen3.5-4b");

        Ok(Self {
            engine: Arc::new(Mutex::new(None)),
            model_path,
            context_window,
            capabilities,
        })
    }

    /// Create a provider using the default model.
    ///
    /// # Errors
    ///
    /// Returns an error if the model is not downloaded or
    /// cannot be accessed.
    pub fn with_default_model() -> LocalModelResult<Self> {
        let manager = LocalModelManager::new()?;

        if !manager.is_model_downloaded() {
            return Err(LocalModelError::ModelNotFound(manager.model_path()));
        }

        Self::new(manager.model_path(), 32_768) // Default 32K context
    }

    /// Check if the model is loaded into memory.
    #[must_use]
    pub async fn is_loaded(&self) -> bool {
        self.engine.lock().await.is_some()
    }

    /// Get model capabilities.
    #[must_use]
    pub const fn capabilities(&self) -> &ModelCapabilities {
        &self.capabilities
    }

    /// Ensure the model is loaded.
    ///
    /// This is called automatically on first use, but can be
    /// called proactively to preload the model.
    ///
    /// # Errors
    ///
    /// Returns an error if the model cannot be loaded.
    pub async fn ensure_loaded(&self) -> LocalModelResult<()> {
        // Check if already loaded without holding lock
        {
            let engine_guard = self.engine.lock().await;
            if engine_guard.is_some() {
                return Ok(());
            }
        }

        // Check hardware before loading
        let hw = HardwareCapabilities::detect();
        if hw.is_memory_critical() {
            return Err(LocalModelError::InsufficientMemory {
                needed_gb: 6.0,
                available_gb: hw.available_ram_gb,
            });
        }

        // Load without holding lock
        tracing::info!("Loading local model into memory...");
        let engine = LocalEngine::load(self.model_path.clone(), self.context_window)?;

        // Store the loaded engine
        let mut engine_guard = self.engine.lock().await;
        *engine_guard = Some(engine);
        drop(engine_guard);
        tracing::info!("Local model loaded");

        Ok(())
    }

    /// Unload the model from memory.
    ///
    /// This frees up RAM but the model will need to be reloaded
    /// on the next request.
    pub async fn unload(&self) {
        let mut engine_guard = self.engine.lock().await;
        if let Some(mut engine) = engine_guard.take() {
            engine.unload();
            tracing::info!("Local model unloaded from memory");
        }
    }

    /// Make a non-streaming chat request.
    ///
    /// # Errors
    ///
    /// Returns an error if inference fails.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn request(&self, messages: &[Message], tools: &[Tool]) -> Result<Message, LlmError> {
        self.ensure_loaded()
            .await
            .map_err(|e| LlmError::LocalModel(e.to_string()))?;

        // Lock engine and perform inference
        // The lock is held for the duration of the chat call
        let engine_guard = self.engine.lock().await;
        let engine = engine_guard
            .as_ref()
            .ok_or_else(|| LlmError::LocalModel("Model not loaded".to_string()))?;

        // If tools provided but model doesn't support them, skip tools
        let effective_tools = if self.capabilities.supports_tools && !tools.is_empty() {
            Some(tools)
        } else {
            None
        };

        engine
            .chat(messages, effective_tools, None)
            .map_err(|e| LlmError::LocalModel(e.to_string()))
    }

    /// Make a streaming chat request.
    ///
    /// # Errors
    ///
    /// Returns an error if inference fails.
    #[allow(clippy::significant_drop_tightening)]
    pub async fn request_stream<F>(
        &self,
        messages: &[Message],
        tools: &[Tool],
        on_event: F,
    ) -> Result<(), LlmError>
    where
        F: FnMut(StreamEvent) + Send,
    {
        self.ensure_loaded()
            .await
            .map_err(|e| LlmError::LocalModel(e.to_string()))?;

        // Lock engine and perform inference
        // The lock is held for the duration of the chat_stream call
        let engine_guard = self.engine.lock().await;
        let engine = engine_guard
            .as_ref()
            .ok_or_else(|| LlmError::LocalModel("Model not loaded".to_string()))?;

        // If tools provided but model doesn't support them, skip tools
        let effective_tools = if self.capabilities.supports_tools && !tools.is_empty() {
            Some(tools)
        } else {
            None
        };

        engine
            .chat_stream(messages, effective_tools, None, on_event)
            .map_err(|e| LlmError::LocalModel(e.to_string()))
    }

    /// Get the model path.
    #[must_use]
    pub fn model_path(&self) -> &std::path::Path {
        &self.model_path
    }
}

/// Check if local models are available on this system.
#[must_use]
pub fn is_local_available() -> bool {
    HardwareCapabilities::detect().can_run_local_model()
}

/// Check if the default model is downloaded.
#[must_use]
pub fn is_model_downloaded() -> bool {
    LocalModelManager::new()
        .map(|m| m.is_model_downloaded())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_local_available() {
        // Should work on any dev machine
        let available = is_local_available();
        // Just check it doesn't panic
        let _ = available;
    }
}
