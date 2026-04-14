//! Profile Editor View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P08
//! @requirement REQ-UI-PE

mod ime;
mod render;
mod render_advanced;

use gpui::FocusHandle;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::default_api_base_url_for_provider;
use crate::events::types::{ModelProfileAuth, ModelProfileParameters, UserEvent};
use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::bridge::GpuiBridge;

/// Auth method enum for display
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AuthMethod {
    #[default]
    Keychain,
}

impl AuthMethod {
    #[must_use]
    pub const fn display(&self) -> &'static str {
        match self {
            Self::Keychain => "Keychain",
        }
    }
}

/// API type enum
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum ApiType {
    #[default]
    Anthropic,
    OpenAI,
    Local,
    Custom(String),
}

impl ApiType {
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Anthropic => "Anthropic".to_string(),
            Self::OpenAI => "OpenAI".to_string(),
            Self::Local => "Local Model".to_string(),
            Self::Custom(provider) => provider.clone(),
        }
    }

    fn provider_id(&self) -> String {
        match self {
            Self::Anthropic => "anthropic".to_string(),
            Self::OpenAI => "openai".to_string(),
            Self::Local => "local".to_string(),
            Self::Custom(provider) => provider.clone(),
        }
    }

    /// Returns `true` if this API type requires an API key.
    #[must_use]
    pub const fn requires_api_key(&self) -> bool {
        match self {
            Self::Anthropic | Self::OpenAI | Self::Custom(_) => true,
            Self::Local => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActiveField {
    Name,
    Model,
    BaseUrl,
    MaxTokens,
    MaxTokensFieldName,
    ExtraRequestFields,
    ContextLimit,
    ThinkingBudget,
    SystemPrompt,
}

/// Profile data for the editor
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Debug, Default)]
pub struct ProfileEditorData {
    pub id: Option<String>,
    pub name: String,
    pub model_id: String,
    pub api_type: ApiType,
    pub base_url: String,
    /// Keychain label for the API key (empty = none selected).
    pub key_label: String,
    /// Available keychain labels populated by `ApiKeysListed`.
    pub available_keys: Vec<String>,
    pub temperature: f32,
    pub max_tokens: String,
    pub max_tokens_field_name: String,
    pub extra_request_fields: String,
    pub context_limit: u32,
    pub show_thinking: bool,
    pub enable_extended_thinking: bool,
    pub thinking_budget: u32,
    pub system_prompt: String,
}

impl ProfileEditorData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: "4096".to_string(),
            max_tokens_field_name: "max_tokens".to_string(),
            extra_request_fields: "{}".to_string(),
            context_limit: 128_000,
            show_thinking: true,
            thinking_budget: 10000,
            system_prompt: crate::models::profile::DEFAULT_SYSTEM_PROMPT.to_string(),
            ..Default::default()
        }
    }

    /// Check if save should be enabled
    #[must_use]
    pub fn can_save(&self) -> bool {
        if self.name.trim().is_empty() {
            return false;
        }
        if self.model_id.trim().is_empty() {
            return false;
        }
        if self.base_url.trim().is_empty() {
            return false;
        }
        // Only require key_label for API types that need authentication
        if self.api_type.requires_api_key() && self.key_label.trim().is_empty() {
            return false;
        }
        true
    }
}

/// Profile Editor view state
/// @plan PLAN-20250130-GPUIREDUX.P08
#[derive(Clone, Default)]
pub struct ProfileEditorState {
    pub data: ProfileEditorData,
    pub is_new: bool,
    pub(super) active_field: Option<ActiveField>,
    pub(super) advanced_request_parameters_expanded: bool,
}

impl ProfileEditorState {
    #[must_use]
    pub fn new_profile() -> Self {
        Self {
            data: ProfileEditorData::new(),
            is_new: true,
            active_field: None,
            advanced_request_parameters_expanded: false,
        }
    }

    #[must_use]
    pub fn edit_profile(data: ProfileEditorData) -> Self {
        let advanced_expanded =
            data.max_tokens_field_name != "max_tokens" || data.extra_request_fields.trim() != "{}";
        Self {
            data,
            is_new: false,
            active_field: None,
            advanced_request_parameters_expanded: advanced_expanded,
        }
    }
}

/// Profile Editor view component
/// @plan PLAN-20250130-GPUIREDUX.P08
pub struct ProfileEditorView {
    pub(super) state: ProfileEditorState,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
    pub(super) focus_handle: FocusHandle,
    /// Number of bytes inserted by IME marked text (dead key composition).
    /// When composition completes, these bytes are removed before inserting the final text.
    pub(super) ime_marked_byte_count: usize,
}

impl ProfileEditorView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: ProfileEditorState::new_profile(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            ime_marked_byte_count: 0,
        }
    }

    /// Set the event bridge
    /// @plan PLAN-20250130-GPUIREDUX.P08
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
        self.request_api_key_refresh();
    }

    fn request_api_key_refresh(&self) {
        self.emit(&UserEvent::RefreshApiKeys);
    }

    /// Set profile data from presenter
    pub fn set_profile(&mut self, data: ProfileEditorData, is_new: bool) {
        self.state.data = data;
        self.state.is_new = is_new;
        self.state.active_field = None;
    }

    fn append_to_active_field(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        match self.state.active_field {
            Some(ActiveField::Name) => self.state.data.name.push_str(text),
            Some(ActiveField::Model) => self.state.data.model_id.push_str(text),
            Some(ActiveField::BaseUrl) => self.state.data.base_url.push_str(text),
            Some(ActiveField::MaxTokens) => {
                if text.chars().all(|c| c.is_ascii_digit()) {
                    let mut s = self.state.data.max_tokens.clone();
                    if s == "0" {
                        s.clear();
                    }
                    s.push_str(text);
                    if s.parse::<u32>().is_ok() {
                        self.state.data.max_tokens = s;
                    }
                }
            }
            Some(ActiveField::MaxTokensFieldName) => {
                self.state.data.max_tokens_field_name.push_str(text);
            }
            Some(ActiveField::ExtraRequestFields) => {
                self.state.data.extra_request_fields.push_str(text);
            }
            Some(ActiveField::ContextLimit) => {
                if text.chars().all(|c| c.is_ascii_digit()) {
                    let mut s = self.state.data.context_limit.to_string();
                    if s == "0" {
                        s.clear();
                    }
                    s.push_str(text);
                    if let Ok(parsed) = s.parse::<u32>() {
                        self.state.data.context_limit = parsed;
                    }
                }
            }
            Some(ActiveField::ThinkingBudget) => {
                if text.chars().all(|c| c.is_ascii_digit()) {
                    let mut s = self.state.data.thinking_budget.to_string();
                    if s == "0" {
                        s.clear();
                    }
                    s.push_str(text);
                    if let Ok(parsed) = s.parse::<u32>() {
                        self.state.data.thinking_budget = parsed;
                    }
                }
            }
            Some(ActiveField::SystemPrompt) => {
                self.state.data.system_prompt.push_str(text);
            }
            None => {}
        }
    }

    fn backspace_active_field(&mut self) {
        match self.state.active_field {
            Some(ActiveField::Name) => {
                self.state.data.name.pop();
            }
            Some(ActiveField::Model) => {
                self.state.data.model_id.pop();
            }
            Some(ActiveField::BaseUrl) => {
                self.state.data.base_url.pop();
            }
            Some(ActiveField::MaxTokens) => {
                let mut s = self.state.data.max_tokens.clone();
                s.pop();
                self.state.data.max_tokens = s;
            }
            Some(ActiveField::MaxTokensFieldName) => {
                self.state.data.max_tokens_field_name.pop();
            }
            Some(ActiveField::ExtraRequestFields) => {
                self.state.data.extra_request_fields.pop();
            }
            Some(ActiveField::ContextLimit) => {
                let mut s = self.state.data.context_limit.to_string();
                s.pop();
                self.state.data.context_limit = if s.is_empty() {
                    0
                } else {
                    s.parse::<u32>().unwrap_or(self.state.data.context_limit)
                };
            }
            Some(ActiveField::ThinkingBudget) => {
                let mut s = self.state.data.thinking_budget.to_string();
                s.pop();
                self.state.data.thinking_budget = if s.is_empty() {
                    0
                } else {
                    s.parse::<u32>().unwrap_or(self.state.data.thinking_budget)
                };
            }
            Some(ActiveField::SystemPrompt) => {
                self.state.data.system_prompt.pop();
            }
            None => {}
        }
    }

    /// Cycle to the next editable field on Tab
    fn cycle_active_field(&mut self) {
        let mut fields = vec![
            ActiveField::Name,
            ActiveField::Model,
            ActiveField::BaseUrl,
            ActiveField::MaxTokens,
        ];
        if self.state.advanced_request_parameters_expanded {
            fields.push(ActiveField::MaxTokensFieldName);
            fields.push(ActiveField::ExtraRequestFields);
        }
        fields.extend([
            ActiveField::ContextLimit,
            ActiveField::ThinkingBudget,
            ActiveField::SystemPrompt,
        ]);
        let current_idx = self
            .state
            .active_field
            .and_then(|f| fields.iter().position(|&x| x == f));
        let next = current_idx.map_or_else(|| fields[0], |i| fields[(i + 1) % fields.len()]);
        self.state.active_field = Some(next);
    }

    /// Active field text content for `InputHandler`
    fn remove_trailing_bytes_from_active_field(&mut self, byte_count: usize) {
        if byte_count == 0 {
            return;
        }
        match self.state.active_field {
            Some(ActiveField::Name) => {
                let len = self.state.data.name.len();
                self.state
                    .data
                    .name
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::Model) => {
                let len = self.state.data.model_id.len();
                self.state
                    .data
                    .model_id
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::BaseUrl) => {
                let len = self.state.data.base_url.len();
                self.state
                    .data
                    .base_url
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::MaxTokensFieldName) => {
                let len = self.state.data.max_tokens_field_name.len();
                self.state
                    .data
                    .max_tokens_field_name
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::ExtraRequestFields) => {
                let len = self.state.data.extra_request_fields.len();
                self.state
                    .data
                    .extra_request_fields
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::SystemPrompt) => {
                let len = self.state.data.system_prompt.len();
                self.state
                    .data
                    .system_prompt
                    .truncate(len.saturating_sub(byte_count));
            }
            _ => {}
        }
    }

    fn active_field_text(&self) -> &str {
        match self.state.active_field {
            Some(ActiveField::Name) => &self.state.data.name,
            Some(ActiveField::Model) => &self.state.data.model_id,
            Some(ActiveField::BaseUrl) => &self.state.data.base_url,
            Some(ActiveField::MaxTokensFieldName) => &self.state.data.max_tokens_field_name,
            Some(ActiveField::ExtraRequestFields) => &self.state.data.extra_request_fields,
            Some(
                ActiveField::MaxTokens | ActiveField::ContextLimit | ActiveField::ThinkingBudget,
            )
            | None => "",
            Some(ActiveField::SystemPrompt) => &self.state.data.system_prompt,
        }
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P08
    fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    fn emit_save_profile(&self) {
        let id = self
            .state
            .data
            .id
            .as_deref()
            .and_then(|raw| Uuid::parse_str(raw).ok())
            .unwrap_or_else(Uuid::new_v4);

        let provider_id = Some(self.state.data.api_type.provider_id());

        let auth = if self.state.data.api_type.requires_api_key() {
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

        let parameters = Some(ModelProfileParameters {
            temperature: Some(f64::from(self.state.data.temperature)),
            max_tokens: self.state.data.max_tokens.parse::<u32>().ok(),
            max_tokens_field_name: Some(self.state.data.max_tokens_field_name.clone()),
            extra_request_fields,
            show_thinking: Some(self.state.data.show_thinking),
            enable_thinking: Some(self.state.data.enable_extended_thinking),
            thinking_budget: if self.state.data.enable_extended_thinking {
                Some(self.state.data.thinking_budget)
            } else {
                None
            },
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

    /// Handle `ViewCommand` from presenter
    /// @plan PLAN-20250130-GPUIREDUX.P08
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-002
    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::ModelSelected {
                provider_id,
                model_id,
                provider_api_url,
                context_length,
            } => {
                self.state.is_new = true;
                self.state.data.model_id.clone_from(&model_id);
                self.state.data.api_type = match provider_id.as_str() {
                    "anthropic" => ApiType::Anthropic,
                    "openai" => ApiType::OpenAI,
                    _ => ApiType::Custom(provider_id.clone()),
                };
                if self.state.data.name.trim().is_empty() {
                    self.state.data.name = model_id;
                }
                if self.state.data.base_url.trim().is_empty() {
                    self.state.data.base_url = provider_api_url
                        .filter(|url| !url.trim().is_empty())
                        .unwrap_or_else(|| default_api_base_url_for_provider(&provider_id));
                }
                if let Some(limit) = context_length {
                    self.state.data.context_limit = limit;
                }
                self.state.active_field = None;
            }
            ViewCommand::ProfileEditorLoad {
                id,
                name,
                provider_id,
                model_id,
                base_url,
                api_key_label,
                temperature,
                max_tokens,
                max_tokens_field_name,
                extra_request_fields,
                context_limit,
                show_thinking,
                enable_thinking,
                thinking_budget,
                system_prompt,
            } => {
                self.state.is_new = false;
                self.state.data.id = Some(id.to_string());
                self.state.data.name = name;
                self.state.data.model_id = model_id;
                self.state.data.base_url = base_url;
                self.state.data.api_type = match provider_id.as_str() {
                    "anthropic" => ApiType::Anthropic,
                    "openai" => ApiType::OpenAI,
                    _ => ApiType::Custom(provider_id.clone()),
                };
                self.state.data.key_label = api_key_label;
                #[allow(clippy::cast_possible_truncation)]
                {
                    self.state.data.temperature = temperature as f32;
                }
                self.state.data.max_tokens =
                    max_tokens.map_or_else(String::new, |value| value.to_string());
                self.state.data.max_tokens_field_name = max_tokens_field_name;
                self.state.data.extra_request_fields = extra_request_fields;
                self.state.advanced_request_parameters_expanded =
                    self.state.data.max_tokens_field_name != "max_tokens"
                        || self.state.data.extra_request_fields.trim() != "{}";
                if let Some(limit) = context_limit {
                    self.state.data.context_limit = limit;
                }
                self.state.data.show_thinking = show_thinking;
                self.state.data.enable_extended_thinking = enable_thinking;
                self.state.data.thinking_budget = thinking_budget.unwrap_or(10_000);
                self.state.data.system_prompt = system_prompt;
                self.state.active_field = None;
            }

            ViewCommand::ApiKeysListed { keys } => {
                self.state.data.available_keys = keys.iter().map(|k| k.label.clone()).collect();
            }

            _ => {}
        }
        cx.notify();
    }
}
