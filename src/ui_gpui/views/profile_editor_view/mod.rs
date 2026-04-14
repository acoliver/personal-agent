//! Profile Editor View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P08
//! @requirement REQ-UI-PE

mod ime;
mod render;

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
    pub const fn edit_profile(data: ProfileEditorData) -> Self {
        Self {
            data,
            is_new: false,
            active_field: None,
            advanced_request_parameters_expanded: false,
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
        let fields = [
            ActiveField::Name,
            ActiveField::Model,
            ActiveField::BaseUrl,
            ActiveField::MaxTokens,
            ActiveField::MaxTokensFieldName,
            ActiveField::ExtraRequestFields,
            ActiveField::ContextLimit,
            ActiveField::ThinkingBudget,
            ActiveField::SystemPrompt,
        ];
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
            serde_json::from_str::<serde_json::Value>(&self.state.data.extra_request_fields).ok();

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

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use flume;
    use gpui::{AppContext, EntityInputHandler, TestAppContext};

    use crate::config::default_api_base_url_for_provider;
    use crate::events::types::UserEvent;
    use crate::presentation::view_command::{ApiKeyInfo, ViewCommand};

    fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
        let (user_tx, user_rx) = flume::bounded(16);
        let (_view_tx, view_rx) = flume::bounded(16);
        (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
    }

    fn clear_navigation_requests() {
        while crate::ui_gpui::navigation_channel()
            .take_pending()
            .is_some()
        {}
    }

    #[gpui::test]
    async fn set_bridge_requests_api_keys_and_model_selection_can_be_saved(
        cx: &mut TestAppContext,
    ) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(ProfileEditorView::new);

        view.update(cx, |view: &mut ProfileEditorView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.handle_command(
                ViewCommand::ModelSelected {
                    provider_id: "openai".to_string(),
                    model_id: "gpt-4.1".to_string(),
                    provider_api_url: None,
                    context_length: Some(256_000),
                },
                cx,
            );
            view.state.data.key_label = "openai-key".to_string();
            view.emit_save_profile();
        });

        assert_eq!(
            user_rx.recv().expect("refresh api keys event"),
            UserEvent::RefreshApiKeys
        );
        match user_rx.recv().expect("save profile event") {
            UserEvent::SaveProfile { profile } => {
                assert_eq!(profile.name, "gpt-4.1");
                assert_eq!(profile.provider_id.as_deref(), Some("openai"));
                assert_eq!(profile.model_id.as_deref(), Some("gpt-4.1"));
                assert_eq!(
                    profile.base_url.as_deref(),
                    Some(default_api_base_url_for_provider("openai").as_str())
                );
                assert!(matches!(
                    profile.auth,
                    Some(ModelProfileAuth::Keychain { ref label }) if label == "openai-key"
                ));
                let parameters = profile.parameters.expect("parameters included");
                assert_eq!(parameters.max_tokens, Some(4096));
                assert_eq!(parameters.enable_thinking, Some(false));
                assert_eq!(parameters.thinking_budget, None);
            }
            other => panic!("expected SaveProfile event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn profile_editor_load_and_api_key_listing_replace_visible_editor_state(
        cx: &mut TestAppContext,
    ) {
        let profile_id = Uuid::new_v4();
        let view = cx.new(ProfileEditorView::new);

        view.update(cx, |view: &mut ProfileEditorView, cx| {
            view.state.active_field = Some(ActiveField::Name);
            view.handle_command(
                ViewCommand::ProfileEditorLoad {
                    id: profile_id,
                    name: "Existing Profile".to_string(),
                    provider_id: "anthropic".to_string(),
                    model_id: "claude-sonnet-4-20250514".to_string(),
                    base_url: "https://api.anthropic.com/v1".to_string(),
                    api_key_label: "anthropic-key".to_string(),
                    temperature: 0.25,
                    max_tokens: Some(8192),
                    max_tokens_field_name: "max_tokens".to_string(),
                    extra_request_fields: "{}".to_string(),

                    context_limit: Some(200_000),
                    show_thinking: false,
                    enable_thinking: true,
                    thinking_budget: None,
                    system_prompt: "Use tools when helpful".to_string(),
                },
                cx,
            );
            view.handle_command(
                ViewCommand::ApiKeysListed {
                    keys: vec![
                        ApiKeyInfo {
                            label: "anthropic-key".to_string(),
                            masked_value: "••••1234".to_string(),
                            used_by: vec!["Existing Profile".to_string()],
                        },
                        ApiKeyInfo {
                            label: "backup-key".to_string(),
                            masked_value: "••••5678".to_string(),
                            used_by: vec![],
                        },
                    ],
                },
                cx,
            );

            assert!(!view.state.is_new);
            assert_eq!(
                view.state.data.id.as_deref(),
                Some(profile_id.to_string().as_str())
            );
            assert_eq!(view.state.data.name, "Existing Profile");
            assert_eq!(view.state.data.model_id, "claude-sonnet-4-20250514");
            assert_eq!(view.state.data.api_type, ApiType::Anthropic);
            assert_eq!(view.state.data.base_url, "https://api.anthropic.com/v1");
            assert_eq!(view.state.data.key_label, "anthropic-key");
            assert!((view.state.data.temperature - 0.25_f32).abs() < f32::EPSILON);
            assert_eq!(view.state.data.max_tokens, "8192");
            assert_eq!(view.state.data.max_tokens_field_name, "max_tokens");
            assert!(!view.state.advanced_request_parameters_expanded);
            assert_eq!(view.state.data.context_limit, 200_000);
            assert!(!view.state.data.show_thinking);
            assert!(view.state.data.enable_extended_thinking);
            assert_eq!(view.state.data.thinking_budget, 10_000);
            assert_eq!(view.state.data.system_prompt, "Use tools when helpful");
            assert_eq!(
                view.state.data.available_keys,
                vec!["anthropic-key".to_string(), "backup-key".to_string()]
            );
            assert!(view.state.active_field.is_none());
            assert!(view.state.data.can_save());
        });
    }

    #[gpui::test]
    async fn input_handler_and_set_profile_cover_real_active_field_and_ime_behavior(
        cx: &mut TestAppContext,
    ) {
        let view = cx.new(ProfileEditorView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ProfileEditorView, cx| {
                let mut data = ProfileEditorData::new();
                data.name = "Preset".to_string();
                data.model_id = "claude-3-7-sonnet".to_string();
                data.base_url = "https://preset.example/v1".to_string();
                data.key_label = "preset-key".to_string();
                data.available_keys = vec!["preset-key".to_string()];
                view.set_profile(data, false);

                assert!(!view.state.is_new);
                assert!(view.state.active_field.is_none());
                assert_eq!(view.state.data.name, "Preset");
                assert!(view.state.data.can_save());

                view.state.active_field = Some(ActiveField::Name);
                view.replace_text_in_range(None, " Δ", window, cx);
                assert_eq!(view.state.data.name, "Preset Δ");
                assert_eq!(
                    view.text_for_range(0..6, &mut None, window, cx),
                    Some("Preset".to_string())
                );

                view.replace_and_mark_text_in_range(None, "é", None, window, cx);
                assert_eq!(view.state.data.name, "Preset Δé");
                assert_eq!(view.marked_text_range(window, cx), Some(8..9));
                assert_eq!(
                    view.selected_text_range(false, window, cx)
                        .expect("selection range")
                        .range,
                    9..9
                );

                view.replace_text_in_range(None, "!", window, cx);

                assert_eq!(view.state.data.name, "Preset Δ!");
                assert_eq!(view.marked_text_range(window, cx), None);
                view.unmark_text(window, cx);
                assert_eq!(view.marked_text_range(window, cx), None);

                view.state.active_field = Some(ActiveField::MaxTokens);
                view.state.data.max_tokens = "0".to_string();
                view.replace_text_in_range(None, "12", window, cx);
                assert_eq!(view.state.data.max_tokens, "12");
                view.replace_text_in_range(None, "x", window, cx);
                assert_eq!(view.state.data.max_tokens, "12");
                view.backspace_active_field();
                assert_eq!(view.state.data.max_tokens, "1");

                view.state.advanced_request_parameters_expanded = true;
                view.state.active_field = Some(ActiveField::MaxTokensFieldName);
                view.state.data.max_tokens_field_name = "max_tokens".to_string();
                view.replace_text_in_range(None, "_override", window, cx);
                assert_eq!(view.state.data.max_tokens_field_name, "max_tokens_override");
                assert_eq!(
                    view.text_for_range(0..10, &mut None, window, cx),
                    Some("max_tokens".to_string())
                );
                view.backspace_active_field();
                assert_eq!(view.state.data.max_tokens_field_name, "max_tokens_overrid");

                view.state.active_field = Some(ActiveField::SystemPrompt);
                let prompt_before = view.state.data.system_prompt.clone();
                view.replace_and_mark_text_in_range(None, " plan", None, window, cx);
                assert_eq!(
                    view.state.data.system_prompt,
                    format!("{prompt_before} plan")
                );
                assert!(view.marked_text_range(window, cx).is_some());
                view.replace_text_in_range(None, " final", window, cx);
                assert!(view.state.data.system_prompt.ends_with(" final"));
                assert_eq!(view.marked_text_range(window, cx), None);
            });
        });
    }

    #[gpui::test]
    async fn profile_editor_load_toggles_advanced_request_parameter_visibility(
        cx: &mut TestAppContext,
    ) {
        let profile_id = Uuid::new_v4();
        let view = cx.new(ProfileEditorView::new);

        view.update(cx, |view: &mut ProfileEditorView, cx| {
            view.handle_command(
                ViewCommand::ProfileEditorLoad {
                    id: profile_id,
                    name: "Existing Profile".to_string(),
                    provider_id: "openai".to_string(),
                    model_id: "gpt-4.1".to_string(),
                    base_url: "https://api.openai.com/v1".to_string(),
                    api_key_label: "openai-key".to_string(),
                    temperature: 0.3,
                    max_tokens: Some(4096),
                    max_tokens_field_name: "max_completion_tokens".to_string(),
                    extra_request_fields: "{}".to_string(),

                    context_limit: Some(128_000),
                    show_thinking: true,
                    enable_thinking: false,
                    thinking_budget: Some(2048),
                    system_prompt: "Use tools when helpful".to_string(),
                },
                cx,
            );
            assert!(view.state.advanced_request_parameters_expanded);

            view.handle_command(
                ViewCommand::ProfileEditorLoad {
                    id: profile_id,
                    name: "Existing Profile".to_string(),
                    provider_id: "anthropic".to_string(),
                    model_id: "claude-sonnet-4-20250514".to_string(),
                    base_url: "https://api.anthropic.com/v1".to_string(),
                    api_key_label: "anthropic-key".to_string(),
                    temperature: 0.25,
                    max_tokens: Some(8192),
                    max_tokens_field_name: "max_tokens".to_string(),
                    extra_request_fields: "{}".to_string(),

                    context_limit: Some(200_000),
                    show_thinking: false,
                    enable_thinking: true,
                    thinking_budget: None,
                    system_prompt: "Use tools when helpful".to_string(),
                },
                cx,
            );
            assert!(!view.state.advanced_request_parameters_expanded);
        });
    }

    #[gpui::test]
    async fn key_refresh_and_navigation_actions_emit_expected_events(cx: &mut TestAppContext) {
        clear_navigation_requests();
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(ProfileEditorView::new);

        view.update(cx, |view: &mut ProfileEditorView, _cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.state.data.name = "Profile".to_string();
            view.state.data.model_id = "gpt-4.1".to_string();
            view.state.data.base_url = "https://api.openai.com/v1".to_string();
            view.state.data.key_label = "openai-key".to_string();
            view.state.data.available_keys =
                vec!["openai-key".to_string(), "backup-key".to_string()];

            view.request_api_key_refresh();
            view.state.data.key_label.clear();
            view.request_api_key_refresh();
            assert!(view.state.data.key_label.is_empty());
            view.state.data.key_label = view.state.data.available_keys[0].clone();
        });

        assert_eq!(
            user_rx.recv().expect("initial refresh"),
            UserEvent::RefreshApiKeys
        );
        assert_eq!(
            user_rx.recv().expect("explicit refresh"),
            UserEvent::RefreshApiKeys
        );
        assert_eq!(
            user_rx.recv().expect("empty dropdown refresh"),
            UserEvent::RefreshApiKeys
        );
        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional profile events"
        );

        clear_navigation_requests();
        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::ModelSelector);
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(crate::presentation::view_command::ViewId::ModelSelector)
        );

        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::ApiKeyManager);
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(crate::presentation::view_command::ViewId::ApiKeyManager)
        );

        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::Settings);
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(crate::presentation::view_command::ViewId::Settings)
        );
    }

    #[gpui::test]
    async fn custom_model_selection_preserves_existing_identity_and_emits_extended_thinking_payload(
        cx: &mut TestAppContext,
    ) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(ProfileEditorView::new);
        let existing_id = Uuid::new_v4();

        view.update(cx, |view: &mut ProfileEditorView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.state.data.id = Some(existing_id.to_string());
            view.state.data.name = "Custom Profile".to_string();
            view.state.data.base_url = "https://gateway.example/v1".to_string();
            view.state.data.key_label = "custom-key".to_string();
            view.state.data.enable_extended_thinking = true;
            view.state.data.thinking_budget = 4096;
            view.state.data.max_tokens = "2048".to_string();

            view.handle_command(
                ViewCommand::ModelSelected {
                    provider_id: "localai".to_string(),
                    model_id: "custom-model".to_string(),
                    provider_api_url: Some("https://unused.example/v1".to_string()),
                    context_length: Some(64_000),
                },
                cx,
            );

            assert!(view.state.is_new);
            assert_eq!(view.state.data.name, "Custom Profile");
            assert_eq!(view.state.data.model_id, "custom-model");
            assert_eq!(
                view.state.data.api_type,
                ApiType::Custom("localai".to_string())
            );
            assert_eq!(view.state.data.base_url, "https://gateway.example/v1");
            assert_eq!(view.state.data.context_limit, 64_000);
            assert!(view.state.active_field.is_none());

            view.emit_save_profile();
        });

        assert_eq!(
            user_rx.recv().expect("refresh api keys event"),
            UserEvent::RefreshApiKeys
        );
        match user_rx.recv().expect("save profile event") {
            UserEvent::SaveProfile { profile } => {
                assert_eq!(profile.id, existing_id);
                assert_eq!(profile.name, "Custom Profile");
                assert_eq!(profile.provider_id.as_deref(), Some("localai"));
                assert_eq!(profile.model_id.as_deref(), Some("custom-model"));
                assert_eq!(
                    profile.base_url.as_deref(),
                    Some("https://gateway.example/v1")
                );
                assert!(matches!(
                    profile.auth,
                    Some(ModelProfileAuth::Keychain { ref label }) if label == "custom-key"
                ));
                let parameters = profile.parameters.expect("parameters included");
                assert_eq!(parameters.max_tokens, Some(2048));
                assert_eq!(parameters.enable_thinking, Some(true));
                assert_eq!(parameters.thinking_budget, Some(4096));
            }
            other => panic!("expected SaveProfile event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn input_handler_covers_model_base_url_and_numeric_fields_with_real_composition_rules(
        cx: &mut TestAppContext,
    ) {
        let view = cx.new(ProfileEditorView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ProfileEditorView, cx| {
                view.state.active_field = Some(ActiveField::Model);
                view.replace_and_mark_text_in_range(None, "claud", None, window, cx);
                assert_eq!(view.state.data.model_id, "claud");
                assert_eq!(view.marked_text_range(window, cx), Some(0..5));
                view.replace_text_in_range(None, "claude", window, cx);
                assert_eq!(view.state.data.model_id, "claude");
                assert_eq!(view.marked_text_range(window, cx), None);

                view.state.active_field = Some(ActiveField::BaseUrl);
                view.replace_text_in_range(None, "https://api.example", window, cx);
                view.replace_and_mark_text_in_range(None, "/v", None, window, cx);
                assert_eq!(view.state.data.base_url, "https://api.example/v");
                view.replace_text_in_range(None, "/v1", window, cx);
                assert_eq!(view.state.data.base_url, "https://api.example/v1");
                view.backspace_active_field();
                assert_eq!(view.state.data.base_url, "https://api.example/v");

                view.state.active_field = Some(ActiveField::ContextLimit);
                view.state.data.context_limit = 0;
                view.replace_text_in_range(None, "32", window, cx);
                assert_eq!(view.state.data.context_limit, 32);
                view.replace_text_in_range(None, "k", window, cx);
                assert_eq!(view.state.data.context_limit, 32);
                view.backspace_active_field();
                assert_eq!(view.state.data.context_limit, 3);

                view.state.active_field = Some(ActiveField::ThinkingBudget);
                view.state.data.thinking_budget = 0;
                view.replace_text_in_range(None, "4096", window, cx);
                assert_eq!(view.state.data.thinking_budget, 4096);
                view.backspace_active_field();
                assert_eq!(view.state.data.thinking_budget, 409);

                view.state.active_field = None;
                view.cycle_active_field();
                assert_eq!(view.state.active_field, Some(ActiveField::Name));
                for _ in 0..8 {
                    view.cycle_active_field();
                }

                assert_eq!(view.state.active_field, Some(ActiveField::SystemPrompt));
                view.cycle_active_field();
                assert_eq!(view.state.active_field, Some(ActiveField::Name));
            });
        });
    }

    #[gpui::test]
    async fn local_api_type_requires_no_key_and_can_be_saved(cx: &mut TestAppContext) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(ProfileEditorView::new);

        view.update(cx, |view: &mut ProfileEditorView, _cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.state.data.name = "Local Profile".to_string();
            view.state.data.model_id = "qwen-3.5-4b".to_string();
            view.state.data.base_url = "http://localhost:8080/v1".to_string();
            view.state.data.api_type = ApiType::Local;

            // Local provider should not require API key
            assert!(!view.state.data.api_type.requires_api_key());
            assert!(view.state.data.key_label.is_empty());

            // Can save without key_label for Local provider
            assert!(view.state.data.can_save());

            view.emit_save_profile();
        });

        assert_eq!(
            user_rx.recv().expect("refresh api keys event"),
            UserEvent::RefreshApiKeys
        );
        match user_rx.recv().expect("save profile event") {
            UserEvent::SaveProfile { profile } => {
                assert_eq!(profile.name, "Local Profile");
                assert_eq!(profile.provider_id.as_deref(), Some("local"));
                assert_eq!(profile.model_id.as_deref(), Some("qwen-3.5-4b"));
                // Should emit None auth for Local provider
                assert!(matches!(profile.auth, Some(ModelProfileAuth::None)));
            }
            other => panic!("expected SaveProfile event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn api_type_cycles_through_anthropic_openai_local_anthropic(cx: &mut TestAppContext) {
        let view = cx.new(ProfileEditorView::new);

        view.update(cx, |view: &mut ProfileEditorView, _cx| {
            view.state.data.api_type = ApiType::Anthropic;
            assert_eq!(view.state.data.api_type.display(), "Anthropic");
            assert!(view.state.data.api_type.requires_api_key());

            // Cycle to OpenAI
            view.state.data.api_type = match view.state.data.api_type {
                ApiType::Anthropic => ApiType::OpenAI,
                ApiType::OpenAI => ApiType::Local,
                ApiType::Local | ApiType::Custom(_) => ApiType::Anthropic,
            };
            assert_eq!(view.state.data.api_type.display(), "OpenAI");
            assert!(view.state.data.api_type.requires_api_key());

            // Cycle to Local
            view.state.data.api_type = match view.state.data.api_type {
                ApiType::Anthropic => ApiType::OpenAI,
                ApiType::OpenAI => ApiType::Local,
                ApiType::Local | ApiType::Custom(_) => ApiType::Anthropic,
            };
            assert_eq!(view.state.data.api_type.display(), "Local Model");
            assert!(!view.state.data.api_type.requires_api_key());

            // Cycle back to Anthropic
            view.state.data.api_type = match view.state.data.api_type {
                ApiType::Anthropic => ApiType::OpenAI,
                ApiType::OpenAI => ApiType::Local,
                ApiType::Local | ApiType::Custom(_) => ApiType::Anthropic,
            };
            assert_eq!(view.state.data.api_type.display(), "Anthropic");
        });
    }
}
