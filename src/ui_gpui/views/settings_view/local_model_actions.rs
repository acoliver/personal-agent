//! Local-model panel actions for `SettingsView`.
//!
// @plan:PLAN-20260903-LOCALMODEL.P04
// @requirement:REQ-LM-006

use super::SettingsView;
use crate::events::types::UserEvent;
use crate::services::local_model_settings::LocalModelSettings;

impl SettingsView {
    /// Ask the presenter to load persisted settings and start status polling.
    pub(super) fn emit_load_local_model_settings(&self) {
        self.emit(&UserEvent::LoadLocalModelSettings);
    }

    pub(super) fn emit_save_local_model_settings(&self, settings: LocalModelSettings) {
        self.emit(&UserEvent::SaveLocalModelSettings { settings });
    }

    pub(super) fn emit_unload_local_model(&self) {
        self.emit(&UserEvent::UnloadLocalModel);
    }

    /// Flip the idle-unload toggle; persists only on Save.
    pub(super) const fn set_local_model_idle_unload(&mut self, enabled: bool) {
        self.state.local_model_idle_unload = enabled;
    }

    /// Parse the edit buffers into [`LocalModelSettings`] and emit a save.
    /// Invalid input surfaces an inline status message and emits nothing.
    pub fn save_local_model_edits(&mut self) {
        match self.build_local_model_edits() {
            Ok(settings) => {
                self.state.status_is_error = false;
                self.state.status_message = Some("Saving local model settings...".to_string());
                self.emit_save_local_model_settings(settings);
            }
            Err(message) => {
                self.state.status_is_error = true;
                self.state.status_message = Some(message);
            }
        }
    }

    /// Combine the persisted snapshot with the edit buffers. Numeric fields
    /// must parse as whole numbers; `n_ctx` and the idle timeout are floored
    /// at 1 because llama.cpp cannot use a zero-sized context.
    fn build_local_model_edits(&self) -> Result<LocalModelSettings, String> {
        let path = self.state.local_model_path_input.trim();
        if path.is_empty() {
            return Err("Model file path cannot be empty.".to_string());
        }
        let n_ctx = parse_u32_field("Context size", &self.state.local_model_ctx_input)?.max(1);
        let gpu_layers = parse_u32_field("GPU layers", &self.state.local_model_gpu_layers_input)?;
        let idle_timeout_minutes =
            parse_u32_field("Idle timeout", &self.state.local_model_idle_minutes_input)?.max(1);

        Ok(LocalModelSettings {
            model_path: std::path::PathBuf::from(path),
            n_ctx,
            gpu_layers,
            idle_unload: self.state.local_model_idle_unload,
            idle_timeout_minutes,
        })
    }

    /// Open a file picker for the GGUF path (v1 also allows plain text entry).
    #[allow(clippy::unused_self)]
    pub(super) fn choose_local_model_file(&mut self, cx: &mut gpui::Context<Self>) {
        let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select Model File (GGUF)".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                if let Some(path) = paths.first() {
                    let path_str = path.to_string_lossy().to_string();
                    cx.update(|cx| {
                        this.update(cx, |view, cx| {
                            view.state.local_model_path_input = path_str;
                            cx.notify();
                        })
                    })
                    .ok();
                }
            }
        })
        .detach();
    }
}

/// Parse a `u32` form field, producing a user-facing error on bad input.
fn parse_u32_field(field: &str, input: &str) -> Result<u32, String> {
    let trimmed = input.trim();
    trimmed
        .parse::<u32>()
        .map_err(|_| format!("{field} must be a whole number (got '{trimmed}')."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_u32_field_rejects_garbage_and_negatives() {
        assert_eq!(parse_u32_field("Context size", " 8192 ").unwrap(), 8192);
        assert!(parse_u32_field("Context size", "abc").is_err());
        assert!(parse_u32_field("Context size", "-1").is_err());
        assert!(parse_u32_field("Context size", "").is_err());
    }
}
