//! MCP Configure View implementation
//!
//! @plan PLAN-20250130-GPUIREDUX.P10
//! @requirement REQ-UI-MC

mod ime;
mod render;

use gpui::FocusHandle;
use std::sync::Arc;

use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::bridge::GpuiBridge;

/// Input-capable fields on the configure screen
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ActiveField {
    ApiKey,
    KeyfilePath,
}

/// Auth method for MCP configuration
/// @plan PLAN-20250130-GPUIREDUX.P10
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum McpAuthMethod {
    None,
    #[default]
    ApiKey,
    Keyfile,
    OAuth,
}

impl McpAuthMethod {
    #[must_use]
    pub const fn display(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::ApiKey => "API Key",
            Self::Keyfile => "Key File",
            Self::OAuth => "OAuth",
        }
    }
}

/// OAuth connection status
/// @plan PLAN-20250130-GPUIREDUX.P10
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum OAuthStatus {
    #[default]
    NotConnected,
    Connecting,
    Connected {
        username: String,
    },
    Error(String),
}

/// Configuration field types
/// @plan PLAN-20250130-GPUIREDUX.P10
#[derive(Clone, Debug)]
pub enum ConfigField {
    String {
        key: String,
        value: String,
        placeholder: String,
    },
    Boolean {
        key: String,
        value: bool,
    },
    Array {
        key: String,
        values: Vec<String>,
    },
}

/// MCP Configure view data
/// @plan PLAN-20250130-GPUIREDUX.P10
#[derive(Clone)]
pub struct McpConfigureData {
    pub id: Option<String>,
    pub name: String,
    pub package: String,
    pub package_type: crate::mcp::McpPackageType,
    pub runtime_hint: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    /// Draft env vars as `(name, plain value, is_secret)`; secret values stay
    /// empty and are resolved from the OS keychain at save/load time.
    pub env: Vec<(String, String, bool)>,
    /// Env var names that already have an OS keychain entry.
    pub stored_secret_names: Vec<String>,
    pub auth_method: McpAuthMethod,
    pub env_var_name: String,
    pub api_key: String,
    pub keyfile_path: String,
    pub oauth_provider: String,
    pub oauth_status: OAuthStatus,
    /// Why the last save attempt was blocked; cleared once a save is
    /// emitted so the feedback never outlives its cause.
    pub save_blocked_reason: Option<String>,
    pub config_fields: Vec<ConfigField>,
    /// Remote URL for HTTP/SSE transport MCPs (None for stdio-only).
    pub url: Option<String>,
}

impl Default for McpConfigureData {
    fn default() -> Self {
        Self {
            id: None,
            name: String::new(),
            package: String::new(),
            package_type: crate::mcp::McpPackageType::Npm,
            runtime_hint: Some("npx".to_string()),
            command: String::new(),
            args: vec![],
            env: vec![],
            stored_secret_names: vec![],
            auth_method: McpAuthMethod::default(),
            env_var_name: String::new(),
            api_key: String::new(),
            keyfile_path: String::new(),
            oauth_provider: String::new(),
            oauth_status: OAuthStatus::default(),
            save_blocked_reason: None,
            config_fields: vec![],
            url: None,
        }
    }
}

impl McpConfigureData {
    #[must_use]
    pub fn new() -> Self {
        Self {
            env_var_name: "API_KEY".to_string(),
            ..Default::default()
        }
    }

    /// Whether the draft would persist more than one secret env row under
    /// `ApiKey` auth. The runtime requires exactly one secret var for HTTP
    /// API key auth, and `typed_secrets` can only fill the first row, so
    /// such a draft must be blocked at the save gate instead of saving a
    /// config the runtime later rejects.
    fn has_multiple_api_key_secrets(&self) -> bool {
        self.auth_method == McpAuthMethod::ApiKey
            && self
                .env
                .iter()
                .filter(|(_, _, is_secret)| *is_secret)
                .count()
                > 1
    }

    /// Check if save should be enabled
    #[must_use]
    pub fn can_save(&self) -> bool {
        if self.name.trim().is_empty() {
            return false;
        }

        // Need either a command (stdio) or a URL (remote HTTP/SSE).
        let has_command = !self.command.trim().is_empty();
        let has_url = self.url.as_ref().is_some_and(|u| !u.trim().is_empty());
        if !has_command && !has_url {
            return false;
        }

        if self.has_multiple_api_key_secrets() {
            return false;
        }

        match self.auth_method {
            McpAuthMethod::None => true,
            // A stored keychain entry counts: editing an existing MCP must
            // not force re-entering its secret.
            McpAuthMethod::ApiKey => {
                !self.api_key.trim().is_empty() || !self.stored_secret_names.is_empty()
            }
            McpAuthMethod::Keyfile => !self.keyfile_path.trim().is_empty(),
            McpAuthMethod::OAuth => matches!(self.oauth_status, OAuthStatus::Connected { .. }),
        }
    }

    /// Env vars to persist: draft pairs become `EnvVarConfig` entries whose
    /// plain value rides in the config only for non-secret vars.
    ///
    /// Keychain-backed secret rows only exist under `ApiKey` auth — the
    /// runtime rejects secret rows under `Keyfile` and ignores them under
    /// `None` — so a secret flag under another method is demoted to a plain
    /// row that keeps the typed value instead of silently discarding it. An
    /// `ApiKey` draft without any secret var derives one from `env_var_name`
    /// so the typed key has a keychain slot at runtime; when a plain var
    /// already carries that name it is converted in place rather than
    /// duplicated.
    fn persisted_env_vars(&self) -> Vec<crate::mcp::EnvVarConfig> {
        let mut env_vars: Vec<crate::mcp::EnvVarConfig> = self
            .env
            .iter()
            .map(|(name, value, is_secret)| {
                let is_secret = *is_secret && self.auth_method == McpAuthMethod::ApiKey;
                crate::mcp::EnvVarConfig {
                    name: name.clone(),
                    required: true,
                    is_secret,
                    value: if is_secret { None } else { Some(value.clone()) },
                }
            })
            .collect();
        if self.auth_method == McpAuthMethod::ApiKey
            && !self.env.iter().any(|(_, _, is_secret)| *is_secret)
        {
            match env_vars
                .iter_mut()
                .find(|var| var.name == self.env_var_name)
            {
                Some(existing) => {
                    existing.is_secret = true;
                    existing.value = None;
                }
                None => env_vars.push(crate::mcp::EnvVarConfig {
                    name: self.env_var_name.clone(),
                    required: true,
                    is_secret: true,
                    value: None,
                }),
            }
        }
        env_vars
    }

    /// Secrets payload for the save event: the typed key targets the first
    /// secret var (or the derived env var name) and is emitted only when the
    /// field holds more than whitespace, leaving any stored key otherwise
    /// untouched.
    fn typed_secrets(&self) -> Vec<(String, crate::events::types::SecretValue)> {
        if self.auth_method != McpAuthMethod::ApiKey || self.api_key.trim().is_empty() {
            return Vec::new();
        }
        let var_name = self
            .env
            .iter()
            .find(|(_, _, is_secret)| *is_secret)
            .map_or_else(|| self.env_var_name.clone(), |(name, _, _)| name.clone());
        vec![(
            var_name,
            crate::events::types::SecretValue::new(&self.api_key),
        )]
    }

    /// Why `can_save` is false, matching the gate's check order so the
    /// first failing rule is the one reported.
    fn blocked_save_reason(&self) -> String {
        if self.name.trim().is_empty() {
            "Name is required".to_string()
        } else if self.command.trim().is_empty()
            && self.url.as_ref().is_none_or(|u| u.trim().is_empty())
        {
            "Command or URL is required".to_string()
        } else if self.has_multiple_api_key_secrets() {
            "API key auth supports exactly one secret env var".to_string()
        } else {
            match self.auth_method {
                McpAuthMethod::ApiKey => "API key required (no stored key)".to_string(),
                McpAuthMethod::Keyfile => "Keyfile path is required".to_string(),
                McpAuthMethod::OAuth => "OAuth connection required".to_string(),
                McpAuthMethod::None => "Draft is incomplete".to_string(),
            }
        }
    }
}

/// MCP Configure view state
/// @plan PLAN-20250130-GPUIREDUX.P10
#[derive(Clone, Default)]
pub struct McpConfigureState {
    pub data: McpConfigureData,
    pub is_new: bool,
    pub mask_api_key: bool,
}

impl McpConfigureState {
    #[must_use]
    pub fn new_mcp() -> Self {
        Self {
            data: McpConfigureData::new(),
            is_new: true,
            mask_api_key: true,
        }
    }

    #[must_use]
    pub const fn edit_mcp(data: McpConfigureData) -> Self {
        Self {
            data,
            is_new: false,
            mask_api_key: true,
        }
    }
}

/// MCP Configure view component
/// @plan PLAN-20250130-GPUIREDUX.P10
pub struct McpConfigureView {
    pub(super) state: McpConfigureState,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
    pub(super) focus_handle: FocusHandle,
    pub(super) active_field: Option<ActiveField>,
    pub(super) show_auth_dropdown: bool,
    pub(super) ime_marked_byte_count: usize,
}

impl McpConfigureView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: McpConfigureState::new_mcp(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            active_field: None,
            show_auth_dropdown: false,
            ime_marked_byte_count: 0,
        }
    }

    /// Set the event bridge
    /// @plan PLAN-20250130-GPUIREDUX.P10
    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
    }

    /// Set MCP data from presenter
    pub fn set_mcp(&mut self, data: McpConfigureData, is_new: bool) {
        self.state.data = data;
        self.state.is_new = is_new;
        // Fresh payload means no in-flight IME composition can make sense.
        self.ime_marked_byte_count = 0;
    }

    fn navigate_to_settings() {
        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::Settings);
    }

    fn save_current(&mut self, cx: &mut gpui::Context<Self>) {
        // Single gate for every save entrypoint (save button, cmd-s): an
        // incomplete draft must not emit SaveMcpConfig. Say why instead of
        // dropping the attempt silently.
        if !self.state.data.can_save() {
            tracing::info!("Save blocked: draft is not complete (can_save is false)");
            self.state.data.save_blocked_reason = Some(self.state.data.blocked_save_reason());
            cx.notify();
            return;
        }
        self.state.data.save_blocked_reason = None;
        self.emit_save_mcp_config();
    }

    fn toggle_mask_api_key(&mut self, cx: &mut gpui::Context<Self>) {
        self.state.mask_api_key = !self.state.mask_api_key;
        cx.notify();
    }

    fn start_oauth(&self) {
        let parsed_id = self
            .state
            .data
            .id
            .as_ref()
            .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
            .unwrap_or_else(uuid::Uuid::nil);
        self.emit(&UserEvent::StartMcpOAuth {
            id: parsed_id,
            provider: self.state.data.oauth_provider.clone(),
        });
    }

    fn active_field_text(&self) -> &str {
        match self.active_field {
            Some(ActiveField::ApiKey) => &self.state.data.api_key,
            Some(ActiveField::KeyfilePath) => &self.state.data.keyfile_path,
            None => "",
        }
    }

    fn append_to_active_field(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        match self.active_field {
            Some(ActiveField::ApiKey) => self.state.data.api_key.push_str(text),
            Some(ActiveField::KeyfilePath) => self.state.data.keyfile_path.push_str(text),
            None => {}
        }
    }

    fn backspace_active_field(&mut self) {
        match self.active_field {
            Some(ActiveField::ApiKey) => {
                self.state.data.api_key.pop();
            }
            Some(ActiveField::KeyfilePath) => {
                self.state.data.keyfile_path.pop();
            }
            None => {}
        }
    }

    fn remove_trailing_bytes_from_active_field(&mut self, byte_count: usize) {
        if byte_count == 0 {
            return;
        }

        match self.active_field {
            Some(ActiveField::ApiKey) => {
                let len = self.state.data.api_key.len();
                self.state
                    .data
                    .api_key
                    .truncate(len.saturating_sub(byte_count));
            }
            Some(ActiveField::KeyfilePath) => {
                let len = self.state.data.keyfile_path.len();
                self.state
                    .data
                    .keyfile_path
                    .truncate(len.saturating_sub(byte_count));
            }
            None => {}
        }
    }

    /// Paste sanitized text into the active field (testable seam for cmd-v).
    fn paste_text(&mut self, text: &str, cx: &mut gpui::Context<Self>) {
        let sanitized = crate::ui_gpui::sanitize_single_line(text);
        if sanitized.is_empty() || self.active_field.is_none() {
            return;
        }
        // A paste during active IME composition replaces the marked range,
        // mirroring `replace_text_in_range`.
        self.remove_trailing_bytes_from_active_field(self.ime_marked_byte_count);
        self.ime_marked_byte_count = 0;
        self.append_to_active_field(&sanitized);
        cx.notify();
    }

    fn activate_field(
        &mut self,
        field: ActiveField,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        self.active_field = Some(field);
        self.ime_marked_byte_count = 0;
        self.show_auth_dropdown = false;
        window.activate_window();
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn toggle_auth_dropdown(&mut self, cx: &mut gpui::Context<Self>) {
        self.show_auth_dropdown = !self.show_auth_dropdown;
        self.active_field = None;
        self.ime_marked_byte_count = 0;
        cx.notify();
    }

    fn select_auth_method(&mut self, method: McpAuthMethod, cx: &mut gpui::Context<Self>) {
        self.state.data.auth_method = method;
        self.show_auth_dropdown = false;
        cx.notify();
    }

    fn handle_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;

        if modifiers.platform && key == "v" {
            if let Some(item) = cx.read_from_clipboard() {
                if let Some(text) = item.text() {
                    self.paste_text(&text, cx);
                }
            }
            return;
        }

        if key == "escape" || (modifiers.platform && key == "w") {
            if self.show_auth_dropdown {
                self.show_auth_dropdown = false;
                cx.notify();
                return;
            }
            Self::navigate_to_settings();
            return;
        }

        if modifiers.platform && key == "s" {
            self.save_current(cx);
            return;
        }

        if modifiers.platform || modifiers.control {
            return;
        }

        if key == "backspace" {
            if self.ime_marked_byte_count > 0 {
                self.remove_trailing_bytes_from_active_field(self.ime_marked_byte_count);
                self.ime_marked_byte_count = 0;
            } else {
                self.backspace_active_field();
            }
            cx.notify();
            return;
        }

        if key == "tab" {
            // Only the field rendered for the current auth method can take
            // focus; other methods render no input at all.
            self.active_field = Self::tab_target(&self.state.data.auth_method);
            self.ime_marked_byte_count = 0;
            self.show_auth_dropdown = false;
            cx.notify();
        }
    }

    /// Tab focus target for the given auth method: the single input field
    /// that method renders, or none for methods with no input.
    const fn tab_target(auth_method: &McpAuthMethod) -> Option<ActiveField> {
        match auth_method {
            McpAuthMethod::ApiKey => Some(ActiveField::ApiKey),
            McpAuthMethod::Keyfile => Some(ActiveField::KeyfilePath),
            McpAuthMethod::None | McpAuthMethod::OAuth => None,
        }
    }

    fn emit_save_mcp_config(&self) {
        let id = self
            .state
            .data
            .id
            .clone()
            .and_then(|s| uuid::Uuid::parse_str(&s).ok())
            .unwrap_or_else(uuid::Uuid::new_v4);

        let d = &self.state.data;
        let has_url = d.url.as_ref().is_some_and(|u| !u.trim().is_empty());
        let package_type = if has_url {
            crate::mcp::McpPackageType::Http
        } else {
            d.package_type.clone()
        };

        let transport = match package_type {
            crate::mcp::McpPackageType::Http => crate::mcp::McpTransport::Http,
            crate::mcp::McpPackageType::Npm | crate::mcp::McpPackageType::Docker => {
                crate::mcp::McpTransport::Stdio
            }
        };

        let source_url = match package_type {
            crate::mcp::McpPackageType::Http => d.url.clone().unwrap_or_default(),
            crate::mcp::McpPackageType::Docker => format!("docker run {}", d.package),
            crate::mcp::McpPackageType::Npm => {
                let runtime = d.runtime_hint.as_deref().unwrap_or("npx");
                format!("{runtime} {}", d.package)
            }
        };

        let source = crate::mcp::McpSource::Manual { url: source_url };

        let package = crate::mcp::McpPackage {
            package_type: package_type.clone(),
            identifier: d.package.clone(),
            runtime_hint: match package_type {
                crate::mcp::McpPackageType::Npm => d.runtime_hint.clone(),
                crate::mcp::McpPackageType::Docker => Some("docker".to_string()),
                crate::mcp::McpPackageType::Http => None,
            },
        };

        let auth_type = match d.auth_method {
            McpAuthMethod::None => crate::mcp::McpAuthType::None,
            McpAuthMethod::ApiKey => crate::mcp::McpAuthType::ApiKey,
            McpAuthMethod::Keyfile => crate::mcp::McpAuthType::Keyfile,
            McpAuthMethod::OAuth => crate::mcp::McpAuthType::OAuth,
        };

        let env_vars = d.persisted_env_vars();
        let keyfile_path =
            if d.auth_method == McpAuthMethod::Keyfile && !d.keyfile_path.trim().is_empty() {
                Some(std::path::PathBuf::from(&d.keyfile_path))
            } else {
                None
            };

        let secrets = d.typed_secrets();

        let config = crate::mcp::McpConfig {
            id,
            name: d.name.clone(),
            enabled: true,
            source,
            package,
            transport,
            auth_type,
            env_vars,
            package_args: vec![],
            keyfile_path,
            config: serde_json::Value::Null,
            oauth_token: None,
        };

        self.emit(&UserEvent::SaveMcpConfig {
            id,
            config: Box::new(config),
            secrets,
        });
    }

    /// Emit a `UserEvent` through the bridge
    /// @plan PLAN-20250130-GPUIREDUX.P10
    fn emit(&self, event: &UserEvent) {
        if let Some(bridge) = &self.bridge {
            if !bridge.emit(event.clone()) {
                tracing::error!("Failed to emit event {:?}", event);
            }
        } else {
            tracing::warn!("No bridge set - event not emitted: {:?}", event);
        }
    }

    pub fn handle_command(&mut self, command: ViewCommand, cx: &mut gpui::Context<Self>) {
        match command {
            ViewCommand::McpConfigureDraftLoaded {
                id,
                name,
                package,
                package_type,
                runtime_hint,
                env_var_name,
                command,
                args,
                auth_type,
                oauth_connected,
                keyfile_path,
                env,
                stored_secret_names,
                url,
            } => {
                self.state.data.auth_method = match auth_type {
                    crate::mcp::McpAuthType::None => McpAuthMethod::None,
                    crate::mcp::McpAuthType::ApiKey => McpAuthMethod::ApiKey,
                    crate::mcp::McpAuthType::Keyfile => McpAuthMethod::Keyfile,
                    crate::mcp::McpAuthType::OAuth => McpAuthMethod::OAuth,
                };

                self.state.data.id = Some(id);
                self.state.data.name.clone_from(&name);
                self.state.data.package = package;
                self.state.data.package_type = package_type;
                self.state.data.runtime_hint = runtime_hint;
                self.state.data.env_var_name = env_var_name;
                self.state.data.command = command;
                self.state.data.args = args;
                self.state.data.env = env;
                self.state.data.stored_secret_names = stored_secret_names;
                self.state.data.keyfile_path = keyfile_path;
                self.state.data.url = url;
                // Fresh edit session: drop credentials captured for a
                // previous MCP, restore the persisted OAuth connection (an
                // existing OAuth MCP must stay savable without re-running
                // the flow) or reset stale state so Save cannot ride a
                // stale Connected, and close any open input UI.
                self.state.data.api_key.clear();
                self.state.mask_api_key = true;
                self.state.data.oauth_status = if oauth_connected {
                    OAuthStatus::Connected { username: name }
                } else {
                    OAuthStatus::NotConnected
                };
                self.state.data.oauth_provider.clear();
                self.active_field = None;
                self.show_auth_dropdown = false;
                self.ime_marked_byte_count = 0;
                self.state.data.save_blocked_reason = None;
                self.state.is_new = self
                    .state
                    .data
                    .id
                    .as_ref()
                    .and_then(|raw| uuid::Uuid::parse_str(raw).ok())
                    .is_none_or(|parsed| parsed.is_nil());
            }
            ViewCommand::ShowNotification { message } => {
                self.state.data.oauth_status = OAuthStatus::Connected { username: message };
            }
            ViewCommand::ShowError { message, .. } => {
                self.state.data.oauth_status = OAuthStatus::Error(message);
            }
            ViewCommand::McpConfigSaved { id, name } => {
                self.state.data.id = Some(id.to_string());
                if let Some(saved_name) = name {
                    self.state.data.name = saved_name;
                }
                self.state.is_new = id.is_nil();
            }
            _ => {}
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
