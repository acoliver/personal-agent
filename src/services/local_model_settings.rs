//! Local model engine settings (REQ-LM-001).
//!
//! One engine serves every `provider_id == "local"` profile, so the engine
//! knobs (GGUF path, context size, GPU layers, idle unload) persist at the
//! application level rather than on any profile. Storage follows the
//! `BackupSettings` precedent: one JSON blob under a single app-settings key.
//!
// @plan:PLAN-20260903-LOCALMODEL.P01
// @requirement:REQ-LM-001

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::app_settings::AppSettingsService;
use super::{ServiceError, ServiceResult};

/// App-settings key the serialized [`LocalModelSettings`] blob lives under.
pub const LOCAL_MODEL_SETTINGS_KEY: &str = "local_model";

/// Default GGUF file name under `<data_local>/PersonalAgent/models/`.
pub const LOCAL_MODEL_FILE: &str = "granite-4.2-3b-Q8_0.gguf";

/// Environment override for the model path, read by [`default_model_path`].
pub const ENV_MODEL_PATH: &str = "PA_LOCAL_GGUF";

/// Upper bound for the context size, set to the Granite 4.2 3B training
/// window (`n_ctx_train`).
///
/// A larger hand-edited value would decode past the positions the model was
/// trained on; the engine clamps to the loaded GGUF's own limit on top of
/// this.
pub const MAX_N_CTX: u32 = 131_072;

/// Engine-level configuration for the in-process local model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalModelSettings {
    /// Path to the GGUF file the engine loads.
    #[serde(default = "default_model_path")]
    pub model_path: PathBuf,
    /// Context window in tokens. `LlamaContextParams::default()` is 512, far
    /// too small for agent conversations, so this is always set explicitly.
    #[serde(default = "default_n_ctx")]
    pub n_ctx: u32,
    /// GPU layers to offload; llama.cpp clamps the value to the layer count.
    #[serde(default = "default_gpu_layers")]
    pub gpu_layers: u32,
    /// Drop the model from memory after the idle timeout.
    #[serde(default = "default_idle_unload")]
    pub idle_unload: bool,
    /// Idle minutes before an unload when [`Self::idle_unload`] is set.
    #[serde(default = "default_idle_timeout_minutes")]
    pub idle_timeout_minutes: u32,
}

const fn default_n_ctx() -> u32 {
    32_768
}

const fn default_gpu_layers() -> u32 {
    999
}

const fn default_idle_unload() -> bool {
    true
}

const fn default_idle_timeout_minutes() -> u32 {
    5
}

/// Resolve the default model path: `PA_LOCAL_GGUF` when set, else
/// `<data_local>/PersonalAgent/models/<LOCAL_MODEL_FILE>`.
#[must_use]
pub fn default_model_path() -> PathBuf {
    if let Some(path) = std::env::var_os(ENV_MODEL_PATH) {
        if !path.is_empty() {
            return PathBuf::from(path);
        }
    }
    dirs::data_local_dir().map_or_else(
        || PathBuf::from(LOCAL_MODEL_FILE),
        |dir| {
            dir.join("PersonalAgent")
                .join("models")
                .join(LOCAL_MODEL_FILE)
        },
    )
}

impl Default for LocalModelSettings {
    fn default() -> Self {
        Self {
            model_path: default_model_path(),
            n_ctx: default_n_ctx(),
            gpu_layers: default_gpu_layers(),
            idle_unload: default_idle_unload(),
            idle_timeout_minutes: default_idle_timeout_minutes(),
        }
    }
}

impl LocalModelSettings {
    /// Bounds `n_ctx` at [`MAX_N_CTX`]; applied on every load path so a
    /// hand-edited settings file cannot exceed the trained window.
    #[must_use]
    pub fn clamped(mut self) -> Self {
        self.n_ctx = self.n_ctx.min(MAX_N_CTX);
        self
    }

    /// Load the persisted settings, returning defaults when none were saved.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError::Serialization`] when the stored blob is not
    /// valid `LocalModelSettings` JSON.
    pub async fn load(settings: &dyn AppSettingsService) -> ServiceResult<Self> {
        settings
            .get_setting(LOCAL_MODEL_SETTINGS_KEY)
            .await?
            .map_or_else(
                || Ok(Self::default()),
                |json| {
                    serde_json::from_str(&json)
                        .map(|loaded: Self| loaded.clamped())
                        .map_err(|e| {
                            ServiceError::Serialization(format!(
                                "Failed to parse local model settings: {e}"
                            ))
                        })
                },
            )
    }

    /// Persist these settings under [`LOCAL_MODEL_SETTINGS_KEY`].
    ///
    /// # Errors
    ///
    /// Returns [`ServiceError`] when serialization or the settings write fails.
    pub async fn save(&self, settings: &dyn AppSettingsService) -> ServiceResult<()> {
        let json = serde_json::to_string(self).map_err(|e| {
            ServiceError::Serialization(format!("Failed to serialize local model settings: {e}"))
        })?;
        settings.set_setting(LOCAL_MODEL_SETTINGS_KEY, json).await
    }
}

/// The app settings file the engine reads, mirroring `resolve_runtime_paths`.
#[must_use]
pub fn app_settings_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("PersonalAgent").join("app_settings.json"))
}

/// Read the persisted settings straight from disk without a settings service.
///
/// The engine's lazy settings cache needs a synchronous read (the actor thread
/// has no async runtime), which the `AppSettingsService` trait does not offer.
/// `Ok(None)` means the file or the key is absent, i.e. first install.
///
/// # Errors
///
/// Returns an error message when the file exists but cannot be read or parsed.
pub fn try_load_from_disk(path: &Path) -> Result<Option<LocalModelSettings>, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))?;
    value
        .get(LOCAL_MODEL_SETTINGS_KEY)
        .map_or(Ok(None), |blob| {
            serde_json::from_value::<LocalModelSettings>(blob.clone())
                .map(|loaded| Some(loaded.clamped()))
                .map_err(|e| format!("invalid local model settings in {}: {e}", path.display()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_plan() {
        let settings = LocalModelSettings::default();
        assert_eq!(settings.n_ctx, 32_768);
        assert_eq!(settings.gpu_layers, 999);
        assert!(settings.idle_unload);
        assert_eq!(settings.idle_timeout_minutes, 5);
        assert!(settings
            .model_path
            .ends_with("PersonalAgent/models/granite-4.2-3b-Q8_0.gguf"));
    }

    #[test]
    fn clamp_bounds_n_ctx_at_the_trained_window() {
        let settings = LocalModelSettings {
            n_ctx: 131_072,
            ..LocalModelSettings::default()
        };
        assert_eq!(settings.clamped().n_ctx, 131_072);
        let settings = LocalModelSettings {
            n_ctx: 200_000,
            ..LocalModelSettings::default()
        };
        assert_eq!(settings.clamped().n_ctx, 131_072);
    }
}
