//! Applying the editor's `ViewCommand` payloads and building the resulting
//! `SaveProfile` event, split out of `mod.rs` for size.
//!
//! @plan PLAN-20250130-GPUIREDUX.P08
//! @requirement REQ-WIRE-002

use super::{ApiType, ModelProfileAuth, ProfileEditorState, ProfileEditorView, Uuid};
use crate::events::types::{ModelProfileParameters, UserEvent};
use crate::models::effort_from_stored;

/// The destructured `ViewCommand::ProfileEditorLoad` payload, so the apply
/// helper takes a single argument.
pub(super) struct ProfileEditorLoadPayload {
    pub(super) id: Uuid,
    pub(super) name: String,
    pub(super) provider_id: String,
    pub(super) model_id: String,
    pub(super) base_url: String,
    pub(super) api_key_label: String,
    pub(super) oauth_account: String,
    pub(super) temperature: f64,
    pub(super) max_tokens: Option<u32>,
    pub(super) max_tokens_field_name: String,
    pub(super) extra_request_fields: String,
    pub(super) context_limit: Option<u32>,
    pub(super) show_thinking: bool,
    pub(super) enable_thinking: bool,
    pub(super) thinking_budget: Option<u32>,
    pub(super) reasoning_effort: String,
    pub(super) system_prompt: String,
}

impl ProfileEditorView {
    /// Apply a `ViewCommand::ProfileEditorLoad` payload to the editor state.
    pub(super) fn apply_profile_editor_load(&mut self, payload: ProfileEditorLoadPayload) {
        let ProfileEditorLoadPayload {
            id,
            name,
            provider_id,
            model_id,
            base_url,
            api_key_label,
            oauth_account,
            temperature,
            max_tokens,
            max_tokens_field_name,
            extra_request_fields,
            context_limit,
            show_thinking,
            enable_thinking,
            thinking_budget,
            reasoning_effort,
            system_prompt,
        } = payload;
        self.state.is_new = false;
        self.state.data.id = Some(id.to_string());
        self.state.data.name = name;
        self.state.data.model_id = model_id;
        self.state.data.base_url = base_url;
        self.state.data.api_type = ApiType::from_provider_id(&provider_id);
        self.state.data.key_label = api_key_label;
        self.state.data.oauth_account = oauth_account;
        self.request_account_refresh();
        // The load payload carries the slug only. Keeping the previous
        // profile's label and plan would caption this account with
        // someone else's name.
        self.state.data.oauth_account_label.clear();
        self.state.data.oauth_account_plan.clear();
        #[allow(clippy::cast_possible_truncation)]
        {
            self.state.data.temperature = temperature as f32;
        }
        self.state.data.max_tokens = max_tokens.map_or_else(String::new, |value| value.to_string());
        self.state.data.max_tokens_field_name = max_tokens_field_name;

        self.state.data.extra_request_fields = extra_request_fields;
        self.state.advanced_request_parameters_expanded =
            ProfileEditorState::has_advanced_request_parameters(&self.state.data);
        if let Some(limit) = context_limit {
            self.state.data.context_limit = limit;
        }
        self.state.data.show_thinking = show_thinking;
        self.state.data.enable_extended_thinking = enable_thinking;
        self.state.data.thinking_budget = thinking_budget.unwrap_or(10_000);
        self.state.data.reasoning_effort = effort_from_stored(&reasoning_effort);
        self.state.data.system_prompt = system_prompt;
        self.state.active_field = None;
        self.refresh_local_engine_state();
    }

    pub(super) fn emit_save_profile(&self) {
        let id = self
            .state
            .data
            .id
            .as_deref()
            .and_then(|raw| Uuid::parse_str(raw).ok())
            .unwrap_or_else(Uuid::new_v4);

        let provider_id = Some(self.state.data.api_type.provider_id());

        let auth = if self.state.data.api_type.requires_oauth_account() {
            Some(ModelProfileAuth::OAuth {
                account: self.state.data.oauth_account.clone(),
            })
        } else if self.state.data.api_type.requires_api_key() {
            Some(ModelProfileAuth::Keychain {
                label: self.state.data.key_label.clone(),
            })
        } else {
            Some(ModelProfileAuth::None)
        };

        let extra_request_fields =
            serde_json::from_str::<serde_json::Value>(&self.state.data.extra_request_fields)
                .ok()
                .filter(serde_json::Value::is_object);

        let max_tokens = self.state.data.max_tokens.parse::<u32>().ok();

        let max_tokens_field_name = {
            let name = self.state.data.max_tokens_field_name.trim();
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        };

        let parameters = Some(ModelProfileParameters {
            temperature: Some(f64::from(self.state.data.temperature)),
            max_tokens,
            max_tokens_field_name,
            extra_request_fields,
            show_thinking: Some(self.state.data.show_thinking),
            enable_thinking: Some(self.state.data.enable_extended_thinking),
            thinking_budget: if self.state.data.enable_extended_thinking {
                Some(self.state.data.thinking_budget)
            } else {
                None
            },
            reasoning_effort: self.capabilities().takes_reasoning_effort().then(|| {
                self.state
                    .data
                    .reasoning_effort
                    .as_ref()
                    .map(|effort| effort.as_str().to_string())
                    .unwrap_or_default()
            }),
            // Issue #182: carry the editor's "CONTEXT LIMIT" field through
            // to the presenter so it actually gets persisted. Local profiles
            // carry None: their budget is the engine's Context size (Settings
            // → Local Model), and persisting an editor value here would let
            // the two drift. @requirement:REQ-LM-001
            context_window_size: (self.state.data.api_type != ApiType::Local)
                .then_some(self.state.data.context_limit as usize),
        });

        self.emit(&UserEvent::SaveProfile {
            profile: Box::new(crate::events::types::ModelProfile {
                id,
                name: self.state.data.name.clone(),
                provider_id,
                model_id: Some(self.state.data.model_id.clone()),
                base_url: Some(self.state.data.base_url.clone()),
                auth,
                parameters,
                system_prompt: Some(self.state.data.system_prompt.clone()),
            }),
        });
    }
}
