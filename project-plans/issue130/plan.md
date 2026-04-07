# Issue #130: Embedded Local LLM with Hardware Detection

## Summary

Add an **optional embedded local LLM** that runs in-process, giving users immediate, friction-free AI access without API keys or network calls. This is additive — cloud APIs remain fully supported.

## Goals

1. **Hardware detection** — Check if user's machine has 16GB+ RAM
2. **Friction-free setup** — Offer to download Qwen3.5-4B with one click
3. **In-process runtime** — Model runs inside PersonalAgent via llama-gguf (pure Rust)
4. **Seamless profile integration** — Local model appears as a provider alongside OpenAI, Anthropic, etc.
5. **Zero friction for cloud users** — Existing API workflows unchanged

## Model Specification

| Attribute | Value |
|-----------|-------|
| Model | Qwen3.5-4B |
| Quantization | Q4_K_M |
| Download size | 2.71 GB |
| Runtime RAM | ~5-6 GB (model + context + KV cache) |
| Context window (default) | 32,768 tokens |
| Context window (max) | 262,144 tokens (user-configurable in settings) |
| HuggingFace repo | `lmstudio-community/Qwen3.5-4B-GGUF` |
| Filename | `Qwen3.5-4B-Q4_K_M.gguf` |

## Hardware Requirements

**Minimum: 16GB total RAM**

This is a hard requirement. Users with less than 16GB will not see the local model offer.

```rust
pub fn can_run_local_model() -> bool {
    let sys = sysinfo::System::new_all();
    let total_ram_gb = sys.total_memory() as f64 / 1_073_741_824.0;
    total_ram_gb >= 16.0
}
```

## Storage Paths

Model files are stored in platform-appropriate cache directories:

| Platform | Path |
|----------|------|
| macOS | `~/Library/Caches/PersonalAgent/models/` |
| Linux | `~/.cache/personalagent/models/` |
| Windows | `%LOCALAPPDATA%\PersonalAgent\models\` |

```rust
fn local_model_dir() -> PathBuf {
    dirs::cache_dir()
        .expect("Could not determine cache directory")
        .join("PersonalAgent")
        .join("models")
}
```

## User Flows

### Flow A: First Run with Capable Hardware

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│    Run AI Locally?                                        │
│                                                             │
│   Your Mac has 16GB RAM — you can run a local AI model      │
│   directly on your machine for:                             │
│                                                             │
│   • Offline use — no internet required                      │
│   • Private conversations — nothing leaves your machine     │
│   • Zero API costs — no subscriptions or usage fees         │
│                                                             │
│   Download Qwen3.5-4B (2.71 GB)                             │
│                                                             │
│   ┌─────────────────────────────────────────────────────┐   │
│   │  [Download Local AI]    [Not Now]    [Learn More]   │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                             │
│   You can always add cloud APIs (OpenAI, Anthropic) later.  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**After user clicks "Download Local AI":**

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│    Downloading Local AI...                                │
│                                                             │
│   Qwen3.5-4B (2.71 GB)                                      │
│                                                             │
│   ████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   │
│   1.2 GB / 2.71 GB  —  44%  —  3 min remaining              │
│                                                             │
│   [Cancel]                                                  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**After download completes:**

```
┌─────────────────────────────────────────────────────────────┐
│                                                             │
│   [OK] Local AI Ready!                                        │
│                                                             │
│   Qwen3.5-4B has been downloaded and is ready to use.       │
│                                                             │
│   A "Local AI" profile has been created for you.            │
│   You can start chatting immediately!                       │
│                                                             │
│   ┌─────────────────────────────────────────────────────┐   │
│   │              [Start Chatting]                        │   │
│   └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Flow B: User Declines or Has Insufficient Hardware

- Modal dismissed → User proceeds to normal chat interface
- Settings → Local Model shows hardware status and manual download option

### Flow C: User Adds Cloud API First

- If user configures a cloud API profile first, the local model offer is skipped
- Local model can still be downloaded later via Settings

## Settings UI: Local Model Section

```
┌─────────────────────────────────────────────────────────────────────────┐
│  Settings                                                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  ┌─ Local Model ─────────────────────────────────────────────────────┐ │
│  │                                                                    │ │
│  │  Hardware Status                                                  │ │
│  │  ┌──────────────────────────────────────────────────────────────┐ │ │
│  │  │  [OK] Your Mac has 16GB RAM — local models supported           │ │ │
│  │  └──────────────────────────────────────────────────────────────┘ │ │
│  │                                                                    │ │
│  │  Downloaded Model                                                 │ │
│  │  ┌──────────────────────────────────────────────────────────────┐ │ │
│  │  │  [OK] Qwen3.5-4B (Q4_K_M) — 2.71 GB — Downloaded               │ │ │
│  │  │                                                              │ │ │
│  │  │  Context Window: [32768     ▼] tokens (default: 32K, max: 256K)│ │
│  │  │                                                              │ │ │
│  │  │  [Delete Model]  [Verify Integrity]                          │ │ │
│  │  └──────────────────────────────────────────────────────────────┘ │ │
│  │                                                                    │ │
│  │  Custom Model (Advanced)                                          │ │
│  │  ┌──────────────────────────────────────────────────────────────┐ │ │
│  │  │  Use custom GGUF model:                                      │ │ │
│  │  │                                                              │ │ │
│  │  │  Path: [/path/to/custom-model.gguf        ] [Browse...]      │ │ │
│  │  │                                                              │ │ │
│  │  │  [Use Custom Model]                                          │ │ │
│  │  └──────────────────────────────────────────────────────────────┘ │ │
│  │                                                                    │ │
│  └────────────────────────────────────────────────────────────────────┘ │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

**If model not downloaded:**

```
┌─ Local Model ─────────────────────────────────────────────────────┐
│                                                                    │
│  Hardware Status                                                  │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │  [OK] Your Mac has 16GB RAM — local models supported           │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                                                                    │
│  No local model downloaded                                        │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │                                                              │ │
│  │  Download Qwen3.5-4B (2.71 GB) for local, offline AI:       │ │
│  │                                                              │ │
│  │  [Download Local AI]                                         │ │
│  │                                                              │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                                                                    │
│  Custom Model (Advanced)                                          │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │  Use custom GGUF model:                                      │ │
│  │  Path: [                      ] [Browse...]                  │ │
│  │  [Use Custom Model]                                          │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

**If hardware insufficient:**

```
┌─ Local Model ─────────────────────────────────────────────────────┐
│                                                                    │
│  Hardware Status                                                  │
│  ┌──────────────────────────────────────────────────────────────┐ │
│  │   This machine has 8GB RAM — local models require 16GB     │ │
│  │                                                              │ │
│  │  You can still use cloud APIs (OpenAI, Anthropic, etc.).    │ │
│  └──────────────────────────────────────────────────────────────┘ │
│                                                                    │
│  [Download anyway (not recommended)]                              │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

## Model Selector Integration

The local model appears alongside cloud providers:

```
┌─ Select Model ───────────────────────────────────────────────────┐
│                                                                  │
│  ═══ Local ═══                                                  │
│  ⬤ Local AI (Qwen3.5-4B) [OK] Downloaded                          │
│                                                                  │
│  ═══ Cloud Providers ═══                                        │
│  ⬜ Claude 3.5 Sonnet (Anthropic)                                │
│  ⬜ GPT-4o (OpenAI)                                              │
│  ⬜ GPT-4o Mini (OpenAI)                                         │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

## Technical Architecture

### New Module: `src/llm/local/`

```
src/llm/local/
├── mod.rs              # Module exports
├── hardware.rs         # RAM detection, capability checks
├── model_manager.rs    # Download, cache, verify, delete
├── engine.rs           # llama-gguf ChatEngine wrapper
└── provider.rs         # LocalLlmProvider impl
```

### Hardware Detection (`hardware.rs`)

```rust
use sysinfo::System;

/// Hardware capabilities for local model inference
#[derive(Debug, Clone)]
pub struct HardwareCapabilities {
    pub total_ram_gb: f64,
    pub cpu_cores: usize,
    pub os_name: String,
}

impl HardwareCapabilities {
    /// Detect current hardware capabilities
    pub fn detect() -> Self {
        let sys = System::new_all();
        Self {
            total_ram_gb: sys.total_memory() as f64 / 1_073_741_824.0,
            cpu_cores: sys.cpus().len(),
            os_name: System::name().unwrap_or_else(|| "Unknown".to_string()),
        }
    }

    /// Check if this machine can run a local model
    pub const fn can_run_local_model(&self) -> bool {
        self.total_ram_gb >= 16.0
    }
}
```

### Model Manager (`model_manager.rs`)

```rust
use std::path::PathBuf;
use llama_gguf::huggingface::HfClient;

/// Model download and cache management
pub struct LocalModelManager {
    cache_dir: PathBuf,
    hf_client: HfClient,
}

/// Default model configuration
pub const DEFAULT_MODEL_REPO: &str = "lmstudio-community/Qwen3.5-4B-GGUF";
pub const DEFAULT_MODEL_FILE: &str = "Qwen3.5-4B-Q4_K_M.gguf";

impl LocalModelManager {
    /// Create a new model manager
    pub fn new() -> Result<Self, LocalModelError> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| LocalModelError::CacheDirNotFound)?
            .join("PersonalAgent")
            .join("models");
        
        std::fs::create_dir_all(&cache_dir)?;
        
        let hf_client = HfClient::new().with_cache_dir(cache_dir.clone());
        
        Ok(Self { cache_dir, hf_client })
    }

    /// Check if the default model is downloaded
    pub fn is_model_downloaded(&self) -> bool {
        self.model_path().exists()
    }

    /// Get the path to the model file
    pub fn model_path(&self) -> PathBuf {
        self.cache_dir.join(DEFAULT_MODEL_FILE)
    }

    /// Download the default model with progress callback
    pub async fn download_default_model(
        &self,
        progress: impl Fn(u64, u64) + Send + 'static,
    ) -> Result<PathBuf, LocalModelError> {
        let path = tokio::task::spawn_blocking(move || {
            // HfClient::download_file handles resume, progress, and caching
            self.hf_client.download_file(
                DEFAULT_MODEL_REPO,
                DEFAULT_MODEL_FILE,
                true, // show progress
            )
        })
        .await??;
        
        Ok(path)
    }

    /// Delete the downloaded model
    pub fn delete_model(&self) -> Result<(), LocalModelError> {
        let path = self.model_path();
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }

    /// Get the model file size
    pub fn model_size(&self) -> Option<u64> {
        self.model_path().metadata().ok().map(|m| m.len())
    }
}
```

### Inference Engine (`engine.rs`)

```rust
use llama_gguf::{
    engine::{ChatEngine, ChatEngineConfig},
    gguf::GgufFile,
    sampling::SamplerConfig,
};
use std::path::PathBuf;
use std::sync::Arc;

/// Local LLM inference engine wrapper
pub struct LocalEngine {
    engine: ChatEngine,
    context_window: usize,
}

impl LocalEngine {
    /// Load a model from a GGUF file
    pub fn load(model_path: PathBuf, context_window: usize) -> Result<Self, LocalModelError> {
        let config = ChatEngineConfig {
            context_size: context_window,
            sampler: SamplerConfig::default(),
            ..ChatEngineConfig::default()
        };
        
        let engine = ChatEngine::from_file(&model_path, config)?;
        
        Ok(Self { engine, context_window })
    }

    /// Generate a response from a chat conversation
    pub fn chat(
        &mut self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<String, LocalModelError> {
        // Convert messages to engine format
        // Apply tool prompts if tools provided
        // Generate response
        // Parse tool calls from output if present
        
        // ... implementation details
    }

    /// Stream a response with callback
    pub fn chat_stream(
        &mut self,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
        on_token: impl Fn(&str) + Send,
    ) -> Result<String, LocalModelError> {
        // Streaming variant
    }
}
```

### Profile Integration (`provider.rs`)

```rust
use crate::llm::{LlmClient, Message, StreamEvent, Tool};
use crate::models::ModelProfile;
use async_trait::async_trait;

/// Local model provider that runs in-process
pub struct LocalProvider {
    engine: Arc<tokio::sync::Mutex<LocalEngine>>,
    model_path: PathBuf,
}

impl LocalProvider {
    /// Create a new local provider
    pub fn new(model_path: PathBuf, context_window: usize) -> Result<Self, LocalModelError> {
        let engine = LocalEngine::load(model_path.clone(), context_window)?;
        Ok(Self {
            engine: Arc::new(tokio::sync::Mutex::new(engine)),
            model_path,
        })
    }
}

/// Integration point for LlmClient
impl LocalProvider {
    /// Handle chat request (non-streaming)
    pub async fn request(
        &self,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<Message, LlmError> {
        let mut engine = self.engine.lock().await;
        engine.chat(messages, Some(tools))
    }

    /// Handle streaming chat request
    pub async fn request_stream<F>(
        &self,
        messages: &[Message],
        tools: &[Tool],
        on_event: F,
    ) -> Result<(), LlmError>
    where
        F: FnMut(StreamEvent) + Send,
    {
        let mut engine = self.engine.lock().await;
        engine.chat_stream(messages, Some(tools), on_event)
    }
}
```

### AuthConfig Extension

```rust
// In src/models/profile.rs

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AuthConfig {
    /// API key stored in the OS keychain, referenced by label.
    Keychain { label: String },
    
    /// In-process local model (no authentication required).
    InProcess,
}

// Update ModelProfile to support local provider
impl ModelProfile {
    /// Create a local model profile
    pub fn new_local(name: String, model_path: PathBuf, context_window: usize) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            provider_id: "local".to_string(),
            model_id: "qwen3.5-4b".to_string(),
            base_url: String::new(),
            auth: AuthConfig::InProcess,
            parameters: ModelParameters::default(),
            system_prompt: DEFAULT_SYSTEM_PROMPT.to_string(),
            context_window_size: context_window,
        }
    }
}
```

### LlmClient Routing

```rust
// In src/llm/client.rs

impl LlmClient {
    /// Create a new LLM client from a model profile
    pub fn from_profile(profile: &ModelProfile) -> StdResult<Self, LlmError> {
        // Check if this is a local provider
        if profile.provider_id == "local" {
            return Self::create_local_client(profile);
        }
        
        // Existing cloud provider logic...
    }
    
    fn create_local_client(profile: &ModelProfile) -> StdResult<Self, LlmError> {
        let model_manager = LocalModelManager::new()?;
        let model_path = model_manager.model_path();
        
        if !model_path.exists() {
            return Err(LlmError::LocalModelNotDownloaded);
        }
        
        let provider = LocalProvider::new(model_path, profile.context_window_size)?;
        
        Ok(Self {
            profile: profile.clone(),
            local_provider: Some(provider),
            // ... other fields
        })
    }
}
```

### Settings Persistence

Add to `AppSettings`:

```rust
// In src/services/app_settings.rs

/// Local model settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LocalModelSettings {
    /// Context window size in tokens
    pub context_window: usize,
    /// Custom model path (if user specifies their own)
    pub custom_model_path: Option<PathBuf>,
}

impl AppSettingsService {
    /// Get local model settings
    pub async fn get_local_model_settings(&self) -> Result<LocalModelSettings>;
    
    /// Update local model settings
    pub async fn set_local_model_settings(&self, settings: &LocalModelSettings) -> Result<()>;
}
```

## Dependencies

All versions verified as latest (April 2026):

```toml
[dependencies]
# Hardware detection (v0.38.4 - Mar 2026)
sysinfo = "0.38"

# Local LLM inference (v0.14.0 - Feb 2026)
# Features: metal (macOS GPU), huggingface (download support)
llama-gguf = { version = "0.14", default-features = false, features = ["cpu", "metal", "huggingface"] }

# Tokenizer support (v0.22.2 - Dec 2025) - for Qwen tokenization
tokenizers = { version = "0.22", optional = true }

[features]
default = []
local-model = ["tokenizers"]
```

Note: `llama-gguf` includes tokenizer support built-in from GGUF metadata, so `tokenizers` is optional for advanced use cases.

## Events

New events for the UI to react:

```rust
// In src/events/types.rs

pub enum UserEvent {
    // ... existing events
    
    /// User accepted local model download offer
    AcceptLocalModelDownload,
    
    /// User declined local model offer
    DeclineLocalModelOffer,
    
    /// Local model download progress
    LocalModelDownloadProgress {
        bytes_downloaded: u64,
        total_bytes: u64,
    },
    
    /// Local model download completed
    LocalModelDownloadComplete,
    
    /// Local model download failed
    LocalModelDownloadFailed {
        error: String,
    },
    
    /// Request to change context window size
    SetLocalModelContextWindow {
        tokens: usize,
    },
}

pub enum ViewCommand {
    // ... existing commands
    
    /// Show the local model download offer modal
    ShowLocalModelOffer {
        hardware_ok: bool,
        ram_gb: f64,
    },
    
    /// Update local model download progress UI
    LocalModelDownloadProgress {
        bytes_downloaded: u64,
        total_bytes: u64,
        percent: f64,
        time_remaining_secs: Option<u64>,
    },
    
    /// Local model is ready to use
    LocalModelReady {
        profile_id: Uuid,
        profile_name: String,
    },
    
    /// Hardware status for settings display
    LocalModelHardwareStatus {
        ram_gb: f64,
        can_run: bool,
        model_downloaded: bool,
    },
}
```

## Startup Flow

```rust
// In startup sequence (main_gpui.rs or startup.rs)

async fn check_and_offer_local_model(
    app_settings: &AppSettingsServiceImpl,
    profile_service: &ProfileServiceImpl,
    view_tx: &tokio::sync::mpsc::Sender<ViewCommand>,
) {
    // 1. Check if user already has profiles configured
    let profiles = profile_service.list().await.ok().unwrap_or_default();
    if !profiles.is_empty() {
        tracing::info!("User has existing profiles, skipping local model offer");
        return;
    }
    
    // 2. Check hardware capability
    let hw = HardwareCapabilities::detect();
    if !hw.can_run_local_model() {
        tracing::info!(
            "Hardware insufficient for local model ({}GB RAM, need 16GB)",
            hw.total_ram_gb
        );
        // Still inform UI so settings can show proper status
        let _ = view_tx.send(ViewCommand::LocalModelHardwareStatus {
            ram_gb: hw.total_ram_gb,
            can_run: false,
            model_downloaded: false,
        }).await;
        return;
    }
    
    // 3. Check if model already downloaded
    let manager = match LocalModelManager::new() {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("Failed to create LocalModelManager: {}", e);
            return;
        }
    };
    
    // 4. Send offer to UI
    let _ = view_tx.send(ViewCommand::ShowLocalModelOffer {
        hardware_ok: true,
        ram_gb: hw.total_ram_gb,
    }).await;
    
    // 5. Also send hardware status for settings
    let _ = view_tx.send(ViewCommand::LocalModelHardwareStatus {
        ram_gb: hw.total_ram_gb,
        can_run: true,
        model_downloaded: manager.is_model_downloaded(),
    }).await;
}
```

## Tool Calling Support

Qwen3.5 supports tool calling via prompt templates. The approach:

1. **Format tools in prompt** - Use Qwen's tool template format
2. **Parse tool calls from output** - Look for structured function call markers
3. **Wire to MCP executor** - Use existing `McpToolExecutor`

Example prompt format for Qwen:

```
<|im_start|>system
You are a helpful assistant with access to tools.

Available tools:
- read_file(path: string): Read file contents
- search(query: string): Search the web

To use a tool, respond with:
<tool_call>{"name": "tool_name", "arguments": {"arg": "value"}}</tool_call>
<|im_end|>
<|im_start|>user
What's in the README.md file?
<|im_end|>
<|im_start|>assistant
<tool_call>{"name": "read_file", "arguments": {"path": "README.md"}}</tool_call>
<|im_end|>
```

## Testing Strategy

### Unit Tests

1. `hardware.rs` - Mock sysinfo for deterministic testing
2. `model_manager.rs` - Mock HfClient for download testing
3. `engine.rs` - Mock engine for provider testing

### Integration Tests

1. Hardware detection on different platforms
2. Download flow (with small test model)
3. Profile creation after download
4. Chat flow with local provider

### Manual Testing

1. Full download flow on each platform (macOS, Windows, Linux)
2. Verify Metal acceleration on Apple Silicon
3. Verify CPU fallback works on all platforms
4. Memory usage under load
5. Cancel/resume download

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Download fails mid-transfer | HfClient supports resume via HTTP range requests |
| Model uses too much RAM | Hard 16GB minimum; warn if system memory pressure high |
| Tool calling quality inconsistent | Test with Qwen3.5-4B; may need prompt tuning |
| Metal/CUDA not available | CPU fallback always works, just slower |
| Cross-platform path issues | Use `dirs` crate for all platform paths |
| Binary size increase | llama-gguf is reasonably sized; model is separate download |

## Out of Scope (Future Work)

- Multiple local models loaded simultaneously
- Model fine-tuning or training
- External server mode (Ollama, llama.cpp server)
- Automatic model selection based on task
- Model quantization/conversion in-app

## Acceptance Criteria

- [ ] Hardware detection (16GB RAM minimum) implemented for macOS/Linux/Windows
- [ ] Detection runs at startup and shows status in Settings
- [ ] Default model download flow implemented (Qwen3.5-4B Q4_K_M)
- [ ] Download resumes on interruption, verifies checksum
- [ ] llama-gguf-based in-process inference working
- [ ] Local model appears in model selector alongside cloud providers
- [ ] User can chat with local model without any API configuration
- [ ] Settings allows downloading default model later if skipped initially
- [ ] Settings allows specifying custom GGUF model path
- [ ] Settings shows model management (disk usage, delete, verify)
- [ ] Settings allows changing context window (default 32K, max 256K)
- [ ] Lazy loading: model only loaded into memory when first used
- [ ] Model unloads cleanly on app quit
- [ ] Cloud API providers continue to work unchanged
- [ ] Machines with <16GB RAM show friendly "not supported" message

## Timeline

Not estimated per project rules. Work will proceed in phases:

1. **Phase 1**: Infrastructure (hardware.rs, model_manager.rs, engine.rs)
2. **Phase 2**: Profile integration (AuthConfig, provider routing)
3. **Phase 3**: UI flows (offer modal, settings, download progress)
4. **Phase 4**: Tool calling integration
5. **Phase 5**: Testing and polish
