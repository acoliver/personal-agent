//! Profile Editor View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P08
//! @requirement REQ-UI-PE

mod ime;
mod render;
mod render_account;
mod render_advanced;

use gpui::FocusHandle;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::default_api_base_url_for_provider;
use crate::events::types::{ModelProfileAuth, ModelProfileParameters, UserEvent};
use crate::presentation::view_command::{ViewCommand, ViewId};
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
    /// `ChatGPT` subscription over the Responses websocket.
    ChatGptCodex,
    /// Any Open Responses-compatible endpoint, URL supplied by the user.
    OpenResponses,
    Local,
    Custom(String),
}

impl ApiType {
    /// Every type offered in the picker, in the order it is offered.
    ///
    /// `Custom` is absent: it exists to carry a provider id loaded from disk,
    /// not as something a user picks.
    pub const CHOICES: [Self; 5] = [
        Self::Anthropic,
        Self::OpenAI,
        Self::ChatGptCodex,
        Self::OpenResponses,
        Self::Local,
    ];

    #[must_use]
    pub fn display(&self) -> String {
        match self {
            Self::Anthropic => "Anthropic".to_string(),
            Self::OpenAI => "OpenAI".to_string(),
            Self::ChatGptCodex => "ChatGPT (Codex)".to_string(),
            Self::OpenResponses => "Open Responses".to_string(),
            Self::Local => "Local Model".to_string(),
            Self::Custom(provider) => provider.clone(),
        }
    }

    /// The provider id this type persists as.
    #[must_use]
    pub fn provider_id(&self) -> String {
        match self {
            Self::Anthropic => "anthropic".to_string(),
            Self::OpenAI => "openai".to_string(),
            Self::ChatGptCodex => "openai-codex".to_string(),
            Self::OpenResponses => "open-responses".to_string(),
            Self::Local => "local".to_string(),
            Self::Custom(provider) => provider.clone(),
        }
    }

    /// Returns `true` if this API type requires an API key.
    #[must_use]
    pub const fn requires_api_key(&self) -> bool {
        match self {
            Self::Anthropic | Self::OpenAI | Self::OpenResponses | Self::Custom(_) => true,
            Self::ChatGptCodex | Self::Local => false,
        }
    }

    /// Returns `true` if this API type authenticates with a signed-in account.
    #[must_use]
    pub const fn requires_oauth_account(&self) -> bool {
        matches!(self, Self::ChatGptCodex)
    }

    /// Whether a turn on this type carries temperature, top-p, and a token
    /// cap.
    ///
    /// The Responses endpoint refuses all three: it answers
    /// `Unsupported parameter: temperature`, and once that is dropped, the
    /// same for `max_output_tokens`. The client omits them, so offering the
    /// controls would invite the user to set a value that is silently thrown
    /// away. Reasoning effort is the knob these models actually take, and the
    /// thinking controls below still apply.
    #[must_use]
    pub const fn honours_sampling_parameters(&self) -> bool {
        !matches!(self, Self::ChatGptCodex | Self::OpenResponses)
    }

    /// The endpoint this type manages on the user's behalf, when it manages
    /// one. A managed endpoint is shown read-only until the user unlocks it.
    #[must_use]
    pub const fn managed_endpoint(&self) -> Option<&'static str> {
        match self {
            Self::ChatGptCodex => Some("wss://chatgpt.com/backend-api/codex/responses"),
            _ => None,
        }
    }

    /// Map a provider id string (as used by the model registry / persisted
    /// profiles) to the corresponding `ApiType` variant.
    ///
    /// Centralised so every load / model-selection path stays in sync — see
    /// issue #182 where omitting the `"local"` arm caused local-provider
    /// profiles to be classified as `Custom`, which falsely required an API
    /// key and disabled Save during an edit.
    #[must_use]
    pub fn from_provider_id(provider_id: &str) -> Self {
        match provider_id {
            "anthropic" => Self::Anthropic,
            "openai" => Self::OpenAI,
            "openai-codex" => Self::ChatGptCodex,
            "open-responses" => Self::OpenResponses,
            "local" => Self::Local,
            other => Self::Custom(other.to_string()),
        }
    }

    /// The next type in the picker, wrapping at the end.
    ///
    /// `Custom` is not in the list, so a profile loaded with an unrecognised
    /// provider id leaves via the first choice.
    #[must_use]
    pub fn next(&self) -> Self {
        Self::CHOICES
            .iter()
            .position(|choice| choice == self)
            .map_or_else(
                || Self::CHOICES[0].clone(),
                |index| Self::CHOICES[(index + 1) % Self::CHOICES.len()].clone(),
            )
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

/// An account the user has already signed into.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CodexAccountChoice {
    /// Slug stored on the profile and used as the keychain key.
    pub account: String,
    /// Email when known, otherwise a shortened account id.
    pub label: String,
    /// Plan name, empty when the provider did not report one.
    pub plan: String,
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
    /// OAuth account slug for OAuth profiles (empty = not signed in).
    pub oauth_account: String,
    /// Human-readable label for the signed-in account, when known.
    pub oauth_account_label: String,
    /// Accounts already signed in, so an existing one can be attached without
    /// running a fresh sign-in.
    pub available_accounts: Vec<CodexAccountChoice>,
    /// Plan name reported by the identity provider, when known.
    pub oauth_account_plan: String,
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
    /// Default `context_limit` used when no explicit value has been chosen
    /// (matches the value assigned by [`ProfileEditorData::new`]).
    pub const DEFAULT_CONTEXT_LIMIT: u32 = 128_000;

    #[must_use]
    pub fn new() -> Self {
        Self {
            temperature: 1.0,
            max_tokens: "4096".to_string(),
            max_tokens_field_name: "max_tokens".to_string(),
            extra_request_fields: "{}".to_string(),
            context_limit: Self::DEFAULT_CONTEXT_LIMIT,
            show_thinking: true,
            thinking_budget: 10000,
            system_prompt: crate::models::profile::DEFAULT_SYSTEM_PROMPT.to_string(),
            ..Default::default()
        }
    }

    /// Reconcile the rest of the form after the API type changes.
    ///
    /// Credentials do not carry across types: a key label means nothing to an
    /// account-authenticated provider and vice versa, and leaving the old one
    /// in place would let Save light up on a credential that cannot be used.
    pub fn apply_api_type_change(&mut self) {
        if !self.api_type.requires_api_key() {
            self.key_label.clear();
        }
        if !self.api_type.requires_oauth_account() {
            self.oauth_account.clear();
            self.oauth_account_label.clear();
            self.oauth_account_plan.clear();
        }

        if let Some(managed) = self.api_type.managed_endpoint() {
            self.base_url = managed.to_string();
        } else if self.base_url.trim().is_empty() || self.base_url_is_managed_elsewhere() {
            // A managed endpoint belongs to the type that manages it. Carrying
            // ChatGPT's websocket URL onto a plain HTTP provider would save an
            // endpoint that provider cannot serve, and Save would allow it
            // because the field is not empty.
            self.base_url = default_api_base_url_for_provider(&self.api_type.provider_id());
        }
    }

    /// Whether `base_url` is the managed endpoint of some other API type, and
    /// so was inherited rather than chosen.
    fn base_url_is_managed_elsewhere(&self) -> bool {
        let current = self.base_url.trim();
        ApiType::CHOICES
            .iter()
            .filter_map(ApiType::managed_endpoint)
            .any(|managed| managed == current)
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
        // Account-authenticated types need a signed-in account instead.
        if self.api_type.requires_oauth_account() && self.oauth_account.trim().is_empty() {
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
    /// Validation message for the advanced request JSON field.
    pub(super) advanced_json_validation_message: Option<String>,
}

impl ProfileEditorState {
    fn has_advanced_request_parameters(data: &ProfileEditorData) -> bool {
        (!data.max_tokens_field_name.trim().is_empty()
            && data.max_tokens_field_name.trim() != "max_tokens")
            || data.extra_request_fields.trim() != "{}"
    }

    #[must_use]
    pub fn new_profile() -> Self {
        Self {
            data: ProfileEditorData::new(),
            is_new: true,
            active_field: None,
            advanced_request_parameters_expanded: false,
            advanced_json_validation_message: None,
        }
    }

    #[must_use]
    pub fn edit_profile(data: ProfileEditorData) -> Self {
        let advanced_expanded = Self::has_advanced_request_parameters(&data);
        Self {
            data,
            is_new: false,
            active_field: None,
            advanced_request_parameters_expanded: advanced_expanded,
            advanced_json_validation_message: None,
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

    /// Ask for the signed-in accounts, so the row can offer them.
    ///
    /// Only for account-authenticated types: a profile using an API key has
    /// no use for the list, and asking reads the keychain.
    fn request_account_refresh(&self) {
        if self.state.data.api_type.requires_oauth_account() {
            self.emit(&UserEvent::ListCodexAccounts);
        }
    }

    /// Remember the accounts the user can attach, and refresh what is shown
    /// for the one already selected.
    pub(super) fn adopt_accounts(
        &mut self,
        accounts: &[crate::presentation::view_command::CodexAccountInfo],
    ) {
        self.state.data.available_accounts = accounts
            .iter()
            .map(|account| CodexAccountChoice {
                account: account.account.clone(),
                label: account.label.clone(),
                plan: account.plan.clone().unwrap_or_default(),
            })
            .collect();
        self.adopt_account_details();
    }

    /// Fill in the display name and plan for the selected account from the
    /// account list.
    ///
    /// A profile stores only the slug, so a freshly loaded profile has nothing
    /// to show until the accounts arrive.
    pub(super) fn adopt_account_details(&mut self) {
        let selected = self.state.data.oauth_account.trim().to_string();
        if selected.is_empty() {
            return;
        }
        if let Some(found) = self
            .state
            .data
            .available_accounts
            .iter()
            .find(|choice| choice.account == selected)
        {
            self.state.data.oauth_account_label.clone_from(&found.label);
            self.state.data.oauth_account_plan.clone_from(&found.plan);
        }
    }

    /// Attach the next known account, so a profile can use a sign-in that
    /// already happened instead of running another one.
    pub(super) fn cycle_oauth_account(&mut self) {
        let accounts = self.state.data.available_accounts.clone();
        if accounts.is_empty() {
            return;
        }
        let current = self.state.data.oauth_account.trim();
        let next = accounts
            .iter()
            .position(|choice| choice.account == current)
            .map_or(0, |at| (at + 1) % accounts.len());
        let choice = &accounts[next];
        self.state.data.oauth_account.clone_from(&choice.account);
        self.state
            .data
            .oauth_account_label
            .clone_from(&choice.label);
        self.state.data.oauth_account_plan.clone_from(&choice.plan);
    }

    fn request_api_key_refresh(&self) {
        self.emit(&UserEvent::RefreshApiKeys);
    }

    /// Set profile data from presenter
    pub fn set_profile(&mut self, data: ProfileEditorData, is_new: bool) {
        self.state.data = data;
        self.state.is_new = is_new;
        self.state.advanced_request_parameters_expanded =
            ProfileEditorState::has_advanced_request_parameters(&self.state.data);

        self.state.active_field = None;
    }

    // Collapsing the inner `if` into a match guard would make the outer match
    // non-exhaustive because the `Some(ActiveField::...)` arms are enumerated
    // without a wildcard. Keeping the nested form preserves exhaustiveness.
    #[allow(clippy::collapsible_match)]
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

    /// Ask for a sign-in and open the sheet.
    ///
    /// The browser is the method requested; the flow falls through to a device
    /// code on its own when the fixed callback port is unavailable.
    pub fn start_codex_sign_in(&self) {
        self.emit(&UserEvent::StartCodexSignIn {
            method: crate::events::types::CodexSignInMethod::Browser,
        });
        crate::ui_gpui::navigation_channel().request_navigate(ViewId::CodexSignIn);
    }

    /// Forget the signed-in account this profile uses.
    pub fn sign_out_codex_account(&mut self, account: String) {
        self.state.data.oauth_account.clear();
        self.state.data.oauth_account_label.clear();
        self.state.data.oauth_account_plan.clear();
        self.emit(&UserEvent::SignOutCodexAccount { account });
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
            // Issue #182: carry the editor's "CONTEXT LIMIT" field through
            // to the presenter so it actually gets persisted.
            context_window_size: Some(self.state.data.context_limit as usize),
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
                self.apply_model_selected(&provider_id, model_id, provider_api_url, context_length);
            }
            ViewCommand::ProfileEditorLoad {
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
                system_prompt,
            } => {
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
                self.state.data.max_tokens =
                    max_tokens.map_or_else(String::new, |value| value.to_string());
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
                self.state.data.system_prompt = system_prompt;
                self.state.active_field = None;
            }

            ViewCommand::ApiKeysListed { keys } => {
                self.state.data.available_keys = keys.iter().map(|k| k.label.clone()).collect();
            }

            ViewCommand::CodexAccountsListed { accounts, .. } => {
                self.adopt_accounts(&accounts);
            }

            ViewCommand::CodexSignInCompleted {
                account,
                label,
                plan,
            } => {
                // The editor is what asked for the sign-in, so it adopts the
                // account without the user selecting anything.
                self.state.data.oauth_account = account;
                self.state.data.oauth_account_label = label;
                self.state.data.oauth_account_plan = plan.unwrap_or_default();
            }

            ViewCommand::ProfileEditorReset => {
                tracing::info!(
                    "ProfileEditorView: resetting to blank new-profile state (ProfileEditorReset)"
                );
                self.reset_to_new_profile();
                self.request_api_key_refresh();
            }

            _ => {}
        }
        cx.notify();
    }

    /// Apply a `ViewCommand::ModelSelected` payload to the editor state.
    ///
    /// Preserves `is_new`, `id`, `key_label`, `name`, `system_prompt`, and any
    /// user-customised `base_url` / `context_limit`. Only fields that are
    /// unset (or at their defaults) are filled from the selected model. See
    /// issue #182 — the Browse flow during an edit must not clobber user
    /// work or silently spawn duplicate profiles.
    fn apply_model_selected(
        &mut self,
        provider_id: &str,
        model_id: String,
        provider_api_url: Option<String>,
        context_length: Option<u32>,
    ) {
        self.state.data.model_id.clone_from(&model_id);
        self.state.data.api_type = ApiType::from_provider_id(provider_id);
        if self.state.data.name.trim().is_empty() {
            self.state.data.name = model_id;
        }
        if self.state.data.base_url.trim().is_empty() {
            self.state.data.base_url = provider_api_url
                .filter(|url| !url.trim().is_empty())
                .unwrap_or_else(|| default_api_base_url_for_provider(provider_id));
        }
        if let Some(limit) = context_length {
            if self.state.data.context_limit == 0
                || self.state.data.context_limit == ProfileEditorData::DEFAULT_CONTEXT_LIMIT
            {
                self.state.data.context_limit = limit;
            }
        }
        self.state.active_field = None;
    }

    /// Reset the editor's `state` to a blank new-profile while preserving the
    /// cached list of available API key labels (so the dropdown stays populated
    /// without waiting for a fresh `ApiKeysListed` command).
    ///
    /// Used by both the Cancel/Esc/Cmd-W handlers and the `ProfileEditorReset`
    /// view command. See issue #182.
    pub(super) fn reset_to_new_profile(&mut self) {
        let available_keys = std::mem::take(&mut self.state.data.available_keys);
        self.state = ProfileEditorState::new_profile();
        self.state.data.available_keys = available_keys;
    }
}

#[cfg(test)]
mod tests;
