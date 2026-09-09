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
    pub env: Option<Vec<(String, String)>>,
    pub auth_method: McpAuthMethod,
    pub env_var_name: String,
    pub api_key: String,
    pub keyfile_path: String,
    pub oauth_provider: String,
    pub oauth_status: OAuthStatus,
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
            env: None,
            auth_method: McpAuthMethod::default(),
            env_var_name: String::new(),
            api_key: String::new(),
            keyfile_path: String::new(),
            oauth_provider: String::new(),
            oauth_status: OAuthStatus::default(),
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
            package_type: crate::mcp::McpPackageType::Npm,
            runtime_hint: Some("npx".to_string()),
            command: String::new(),
            args: vec![],
            env: None,
            ..Default::default()
        }
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

        match self.auth_method {
            McpAuthMethod::None => true,
            McpAuthMethod::ApiKey => !self.api_key.trim().is_empty(),
            McpAuthMethod::Keyfile => !self.keyfile_path.trim().is_empty(),
            McpAuthMethod::OAuth => matches!(self.oauth_status, OAuthStatus::Connected { .. }),
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
    }

    fn navigate_to_settings() {
        crate::ui_gpui::navigation_channel()
            .request_navigate(crate::presentation::view_command::ViewId::Settings);
    }

    fn save_current(&self) {
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

    fn sanitized_clipboard_text(text: &str) -> String {
        text.trim_matches(|c| c == '\r' || c == '\n').to_string()
    }

    /// Paste sanitized text into the active field (testable seam for cmd-v).
    fn paste_text(&mut self, text: &str, cx: &mut gpui::Context<Self>) {
        let sanitized = Self::sanitized_clipboard_text(text);
        if sanitized.is_empty() || self.active_field.is_none() {
            return;
        }
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
        self.show_auth_dropdown = false;
        window.activate_window();
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    fn toggle_auth_dropdown(&mut self, cx: &mut gpui::Context<Self>) {
        self.show_auth_dropdown = !self.show_auth_dropdown;
        self.active_field = None;
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
            self.save_current();
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
            self.active_field = Some(match self.active_field {
                Some(ActiveField::ApiKey) => ActiveField::KeyfilePath,
                Some(ActiveField::KeyfilePath) | None => ActiveField::ApiKey,
            });
            self.show_auth_dropdown = false;
            cx.notify();
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

        let mut env_vars: Vec<crate::mcp::EnvVarConfig> =
            d.env.as_ref().map_or_else(Vec::new, |pairs| {
                pairs
                    .iter()
                    .map(|(k, _)| crate::mcp::EnvVarConfig {
                        name: k.clone(),
                        required: true,
                        is_secret: true,
                    })
                    .collect()
            });
        if env_vars.is_empty() && d.auth_method == McpAuthMethod::ApiKey {
            env_vars.push(crate::mcp::EnvVarConfig {
                name: d.env_var_name.clone(),
                required: true,
                is_secret: true,
            });
        }

        let keyfile_path =
            if d.auth_method == McpAuthMethod::Keyfile && !d.keyfile_path.trim().is_empty() {
                Some(std::path::PathBuf::from(&d.keyfile_path))
            } else {
                None
            };

        // The typed key travels only in the secrets payload (env var name →
        // value) so the presenter can store it in the OS keychain; it is never
        // serialized into the config.
        let secrets = if d.auth_method == McpAuthMethod::ApiKey && !d.api_key.is_empty() {
            let var_name = d
                .env
                .as_ref()
                .and_then(|pairs| pairs.first())
                .map_or_else(|| d.env_var_name.clone(), |(k, _)| k.clone());
            vec![(var_name, d.api_key.clone())]
        } else {
            Vec::new()
        };

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
                env,
                url,
            } => {
                // Infer auth method from env vars: if no env vars exist the
                // server needs no credentials (e.g. Exa remote HTTP).
                let has_env = env
                    .as_ref()
                    .is_some_and(|v| v.iter().any(|(k, _)| !k.is_empty()));
                self.state.data.auth_method = if has_env {
                    McpAuthMethod::ApiKey
                } else {
                    McpAuthMethod::None
                };

                self.state.data.id = Some(id);
                self.state.data.name = name;
                self.state.data.package = package;
                self.state.data.package_type = package_type;
                self.state.data.runtime_hint = runtime_hint;
                self.state.data.env_var_name = env_var_name;
                self.state.data.command = command;
                self.state.data.args = args;
                self.state.data.env = env;
                self.state.data.url = url;
                // Fresh edit session: drop credentials captured for a previous
                // MCP and close any open input UI.
                self.state.data.api_key.clear();
                self.state.data.keyfile_path.clear();
                self.state.mask_api_key = true;
                self.active_field = None;
                self.show_auth_dropdown = false;
                self.ime_marked_byte_count = 0;
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
                self.state.data.oauth_status = OAuthStatus::Connected {
                    username: "Saved".to_string(),
                };
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
    use gpui::{AppContext, TestAppContext};
    use uuid::Uuid;

    use crate::events::types::UserEvent;
    use crate::presentation::view_command::{ErrorSeverity, ViewCommand};

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
    async fn draft_loaded_sets_auth_transport_and_save_payload_for_remote_http(
        cx: &mut TestAppContext,
    ) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);

        view.update(cx, |view: &mut McpConfigureView, cx| {
            view.set_bridge(Arc::clone(&bridge));
            view.handle_command(
                ViewCommand::McpConfigureDraftLoaded {
                    id: Uuid::nil().to_string(),
                    name: "Exa Remote".to_string(),
                    package: "exa-remote".to_string(),
                    package_type: crate::mcp::McpPackageType::Http,
                    runtime_hint: None,
                    env_var_name: String::new(),
                    command: String::new(),
                    args: vec![],
                    env: None,
                    url: Some("https://exa.example/mcp".to_string()),
                },
                cx,
            );
            assert!(view.state.is_new);
            assert_eq!(view.state.data.auth_method, McpAuthMethod::None);
            assert_eq!(
                view.state.data.url.as_deref(),
                Some("https://exa.example/mcp")
            );
            assert!(view.state.data.can_save());
            view.emit_save_mcp_config();
        });

        match user_rx.recv().expect("save mcp config event") {
            UserEvent::SaveMcpConfig {
                id,
                config,
                secrets,
            } => {
                assert_eq!(id, Uuid::nil());
                assert_eq!(config.name, "Exa Remote");
                assert_eq!(
                    config.package.package_type,
                    crate::mcp::McpPackageType::Http
                );
                assert_eq!(config.transport, crate::mcp::McpTransport::Http);
                assert_eq!(
                    config.source,
                    crate::mcp::McpSource::Manual {
                        url: "https://exa.example/mcp".to_string()
                    }
                );
                assert!(config.env_vars.is_empty());
                assert!(secrets.is_empty());
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn draft_loaded_with_env_requires_api_key_and_status_commands_update_oauth_state(
        cx: &mut TestAppContext,
    ) {
        let view = cx.new(McpConfigureView::new);
        let saved_id = Uuid::new_v4();

        view.update(cx, |view: &mut McpConfigureView, cx| {
            view.handle_command(
                ViewCommand::McpConfigureDraftLoaded {
                    id: saved_id.to_string(),
                    name: "Filesystem".to_string(),
                    package: "@modelcontextprotocol/server-filesystem".to_string(),
                    package_type: crate::mcp::McpPackageType::Npm,
                    runtime_hint: Some("npx".to_string()),
                    env_var_name: "FILESYSTEM_TOKEN".to_string(),
                    command: "npx".to_string(),
                    args: vec![
                        "-y".to_string(),
                        "@modelcontextprotocol/server-filesystem".to_string(),
                    ],
                    env: Some(vec![("FILESYSTEM_TOKEN".to_string(), String::new())]),
                    url: None,
                },
                cx,
            );
            assert!(!view.state.is_new);
            assert_eq!(view.state.data.auth_method, McpAuthMethod::ApiKey);
            assert_eq!(view.state.data.env_var_name, "FILESYSTEM_TOKEN");
            assert_eq!(view.state.data.runtime_hint.as_deref(), Some("npx"));
            assert!(!view.state.data.can_save());

            view.handle_command(
                ViewCommand::ShowNotification {
                    message: "alice".to_string(),
                },
                cx,
            );
            assert_eq!(
                view.state.data.oauth_status,
                OAuthStatus::Connected {
                    username: "alice".to_string()
                }
            );

            view.handle_command(
                ViewCommand::ShowError {
                    title: "oauth failed".to_string(),
                    message: "denied".to_string(),
                    severity: ErrorSeverity::Error,
                },
                cx,
            );
            assert_eq!(
                view.state.data.oauth_status,
                OAuthStatus::Error("denied".to_string())
            );

            view.handle_command(
                ViewCommand::McpConfigSaved {
                    id: saved_id,
                    name: Some("Filesystem Saved".to_string()),
                },
                cx,
            );
            assert_eq!(
                view.state.data.id.as_deref(),
                Some(saved_id.to_string().as_str())
            );
            assert_eq!(view.state.data.name, "Filesystem Saved");
            assert!(!view.state.is_new);
            assert_eq!(
                view.state.data.oauth_status,
                OAuthStatus::Connected {
                    username: "Saved".to_string()
                }
            );
        });
    }

    #[gpui::test]
    async fn set_mcp_with_keyfile_auth_and_docker_package_emits_stdio_save_payload(
        cx: &mut TestAppContext,
    ) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);
        let saved_id = Uuid::new_v4();

        view.update(cx, |view: &mut McpConfigureView, _cx| {
            view.set_bridge(Arc::clone(&bridge));

            let mut data = McpConfigureData::new();
            data.id = Some(saved_id.to_string());
            data.name = "Docker Filesystem".to_string();
            data.package = "ghcr.io/example/filesystem-mcp:latest".to_string();
            data.package_type = crate::mcp::McpPackageType::Docker;
            data.command = "docker".to_string();
            data.args = vec!["run".to_string(), "--rm".to_string()];
            data.env = Some(vec![
                ("FILESYSTEM_TOKEN".to_string(), String::new()),
                ("ROOT".to_string(), String::new()),
            ]);
            data.auth_method = McpAuthMethod::Keyfile;
            data.keyfile_path = "/tmp/filesystem-key.json".to_string();

            view.set_mcp(data, false);
            assert!(!view.state.is_new);
            assert!(view.state.data.can_save());

            view.state.data.keyfile_path.clear();
            assert!(!view.state.data.can_save());
            view.state.data.keyfile_path = "/tmp/filesystem-key.json".to_string();
            assert!(view.state.data.can_save());

            view.emit_save_mcp_config();
        });

        match user_rx.recv().expect("save docker mcp event") {
            UserEvent::SaveMcpConfig {
                id,
                config,
                secrets,
            } => {
                assert_eq!(id, saved_id);
                assert_eq!(config.name, "Docker Filesystem");
                assert_eq!(
                    config.package.package_type,
                    crate::mcp::McpPackageType::Docker
                );
                assert_eq!(
                    config.package.identifier,
                    "ghcr.io/example/filesystem-mcp:latest"
                );
                assert_eq!(config.package.runtime_hint.as_deref(), Some("docker"));
                assert_eq!(config.transport, crate::mcp::McpTransport::Stdio);
                assert_eq!(
                    config.source,
                    crate::mcp::McpSource::Manual {
                        url: "docker run ghcr.io/example/filesystem-mcp:latest".to_string()
                    }
                );
                assert_eq!(
                    config.env_vars,
                    vec![
                        crate::mcp::EnvVarConfig {
                            name: "FILESYSTEM_TOKEN".to_string(),
                            required: true,
                            is_secret: true,
                        },
                        crate::mcp::EnvVarConfig {
                            name: "ROOT".to_string(),
                            required: true,
                            is_secret: true,
                        },
                    ]
                );
                assert_eq!(config.auth_type, crate::mcp::McpAuthType::Keyfile);
                assert_eq!(
                    config.keyfile_path,
                    Some(std::path::PathBuf::from("/tmp/filesystem-key.json"))
                );
                assert!(secrets.is_empty());
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn set_mcp_with_oauth_only_saves_when_connected_and_emits_npm_payload(
        cx: &mut TestAppContext,
    ) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);

        view.update(cx, |view: &mut McpConfigureView, cx| {
            view.set_bridge(Arc::clone(&bridge));

            let mut data = McpConfigureData::new();
            data.name = "OAuth MCP".to_string();
            data.package = "@example/oauth-mcp".to_string();
            data.package_type = crate::mcp::McpPackageType::Npm;
            data.runtime_hint = Some("npx".to_string());
            data.command = "npx".to_string();
            data.auth_method = McpAuthMethod::OAuth;
            data.oauth_status = OAuthStatus::NotConnected;

            view.set_mcp(data, true);
            assert!(view.state.is_new);
            assert!(!view.state.data.can_save());

            view.handle_command(
                ViewCommand::ShowNotification {
                    message: "carol".to_string(),
                },
                cx,
            );
            assert_eq!(
                view.state.data.oauth_status,
                OAuthStatus::Connected {
                    username: "carol".to_string()
                }
            );
            assert!(view.state.data.can_save());

            view.handle_command(
                ViewCommand::ShowError {
                    title: "oauth failed".to_string(),
                    message: "expired".to_string(),
                    severity: ErrorSeverity::Error,
                },
                cx,
            );
            assert_eq!(
                view.state.data.oauth_status,
                OAuthStatus::Error("expired".to_string())
            );
            assert!(!view.state.data.can_save());

            view.handle_command(
                ViewCommand::ShowNotification {
                    message: "carol".to_string(),
                },
                cx,
            );
            assert!(view.state.data.can_save());
            view.emit_save_mcp_config();
        });

        match user_rx.recv().expect("save npm mcp event") {
            UserEvent::SaveMcpConfig {
                id,
                config,
                secrets,
            } => {
                assert_ne!(id, Uuid::nil());
                assert_eq!(config.name, "OAuth MCP");
                assert_eq!(config.package.package_type, crate::mcp::McpPackageType::Npm);
                assert_eq!(config.package.identifier, "@example/oauth-mcp");
                assert_eq!(config.package.runtime_hint.as_deref(), Some("npx"));
                assert_eq!(config.transport, crate::mcp::McpTransport::Stdio);
                assert_eq!(
                    config.source,
                    crate::mcp::McpSource::Manual {
                        url: "npx @example/oauth-mcp".to_string()
                    }
                );
                assert!(config.env_vars.is_empty());
                assert!(secrets.is_empty());
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn helper_actions_and_key_shortcuts_emit_oauth_save_and_navigation_events(
        cx: &mut TestAppContext,
    ) {
        clear_navigation_requests();
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);
        let saved_id = Uuid::new_v4();

        view.update(cx, |view: &mut McpConfigureView, view_cx| {
            view.set_bridge(Arc::clone(&bridge));

            let mut data = McpConfigureData::new();
            data.id = Some(saved_id.to_string());
            data.name = "Weather MCP".to_string();
            data.package = "@example/weather-mcp".to_string();
            data.package_type = crate::mcp::McpPackageType::Npm;
            data.runtime_hint = Some("npx".to_string());
            data.command = "npx".to_string();
            data.auth_method = McpAuthMethod::ApiKey;
            data.env_var_name = "WEATHER_API_KEY".to_string();
            data.api_key = "secret-token".to_string();
            data.oauth_provider = "ExampleAuth".to_string();
            view.set_mcp(data, false);

            assert!(view.state.mask_api_key);
            view.toggle_mask_api_key(view_cx);
            assert!(!view.state.mask_api_key);
            view.toggle_mask_api_key(view_cx);
            assert!(view.state.mask_api_key);

            view.start_oauth();
            view.save_current();
        });

        let mut visual_cx = cx.add_empty_window().clone();
        visual_cx.update(|window, app| {
            view.update(app, |view: &mut McpConfigureView, cx| {
                view.handle_key_down(&key_event("cmd-s"), window, cx);
            });
        });
        visual_cx.update(|window, app| {
            view.update(app, |view: &mut McpConfigureView, cx| {
                view.handle_key_down(&key_event("escape"), window, cx);
            });
        });
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(crate::presentation::view_command::ViewId::Settings)
        );

        McpConfigureView::navigate_to_settings();
        assert_eq!(
            crate::ui_gpui::navigation_channel().take_pending(),
            Some(crate::presentation::view_command::ViewId::Settings)
        );

        assert_eq!(
            user_rx.recv().expect("oauth start event"),
            UserEvent::StartMcpOAuth {
                id: saved_id,
                provider: "ExampleAuth".to_string(),
            }
        );

        match user_rx.recv().expect("explicit save event") {
            UserEvent::SaveMcpConfig {
                id,
                config,
                secrets,
            } => {
                assert_eq!(id, saved_id);
                assert_eq!(config.name, "Weather MCP");
                assert_eq!(config.package.identifier, "@example/weather-mcp");
                assert_eq!(config.auth_type, crate::mcp::McpAuthType::ApiKey);
                assert_eq!(
                    secrets,
                    vec![("WEATHER_API_KEY".to_string(), "secret-token".to_string())]
                );
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }

        match user_rx.recv().expect("cmd-s save event") {
            UserEvent::SaveMcpConfig { id, config, .. } => {
                assert_eq!(id, saved_id);
                assert_eq!(config.name, "Weather MCP");
                assert_eq!(config.package.identifier, "@example/weather-mcp");
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }

        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional mcp configure events"
        );
    }

    fn key_event(key: &str) -> gpui::KeyDownEvent {
        gpui::KeyDownEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap_or_else(|_| panic!("{key} keystroke")),
            is_held: false,
            prefer_character_input: false,
        }
    }

    #[gpui::test]
    async fn paste_populates_active_field_and_sanitizes(cx: &mut TestAppContext) {
        let view = cx.new(McpConfigureView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut McpConfigureView, cx| {
                view.active_field = Some(ActiveField::ApiKey);
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                    "sk-test-123\r\n".to_string(),
                ));
                view.handle_key_down(&key_event("cmd-v"), window, cx);
                assert_eq!(view.state.data.api_key, "sk-test-123");

                view.active_field = Some(ActiveField::KeyfilePath);
                view.handle_key_down(&key_event("cmd-v"), window, cx);
                assert_eq!(view.state.data.keyfile_path, "sk-test-123");
            });
        });
    }

    #[gpui::test]
    async fn paste_without_active_field_is_a_no_op(cx: &mut TestAppContext) {
        let view = cx.new(McpConfigureView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut McpConfigureView, cx| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                    "sk-nothing\r\n".to_string(),
                ));
                view.handle_key_down(&key_event("cmd-v"), window, cx);
                assert!(view.state.data.api_key.is_empty());
                assert!(view.state.data.keyfile_path.is_empty());
            });
        });
    }

    #[gpui::test]
    async fn backspace_pops_last_character(cx: &mut TestAppContext) {
        let view = cx.new(McpConfigureView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut McpConfigureView, cx| {
                view.state.data.api_key = "sk-test".to_string();
                view.active_field = Some(ActiveField::ApiKey);
                view.handle_key_down(&key_event("backspace"), window, cx);
                assert_eq!(view.state.data.api_key, "sk-tes");

                view.state.data.keyfile_path = "/tmp/k.json".to_string();
                view.active_field = Some(ActiveField::KeyfilePath);
                view.handle_key_down(&key_event("backspace"), window, cx);
                assert_eq!(view.state.data.keyfile_path, "/tmp/k.jso");
            });
        });
    }

    #[gpui::test]
    async fn tab_cycles_active_fields(cx: &mut TestAppContext) {
        let view = cx.new(McpConfigureView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut McpConfigureView, cx| {
                view.handle_key_down(&key_event("tab"), window, cx);
                assert_eq!(view.active_field, Some(ActiveField::ApiKey));

                view.handle_key_down(&key_event("tab"), window, cx);
                assert_eq!(view.active_field, Some(ActiveField::KeyfilePath));

                view.handle_key_down(&key_event("tab"), window, cx);
                assert_eq!(view.active_field, Some(ActiveField::ApiKey));
            });
        });
    }

    #[gpui::test]
    async fn ime_replace_lands_in_active_field(cx: &mut TestAppContext) {
        use gpui::EntityInputHandler;

        let view = cx.new(McpConfigureView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut McpConfigureView, cx| {
                view.active_field = Some(ActiveField::ApiKey);

                view.replace_text_in_range(None, "sk-ime", window, cx);
                assert_eq!(view.state.data.api_key, "sk-ime");

                view.replace_and_mark_text_in_range(None, "!", None, window, cx);
                assert_eq!(view.state.data.api_key, "sk-ime!");
                assert_eq!(view.marked_text_range(window, cx), Some(6..7));

                view.replace_text_in_range(None, "?", window, cx);
                assert_eq!(view.state.data.api_key, "sk-ime?");
                assert_eq!(view.marked_text_range(window, cx), None);
            });
        });
    }

    #[gpui::test]
    async fn auth_dropdown_state_transitions(cx: &mut TestAppContext) {
        clear_navigation_requests();
        let view = cx.new(McpConfigureView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut McpConfigureView, cx| {
                view.active_field = Some(ActiveField::ApiKey);

                view.toggle_auth_dropdown(cx);
                assert!(view.show_auth_dropdown);
                assert_eq!(view.active_field, None);

                view.handle_key_down(&key_event("escape"), window, cx);
                assert!(!view.show_auth_dropdown);
                assert_eq!(crate::ui_gpui::navigation_channel().take_pending(), None);

                view.toggle_auth_dropdown(cx);
                view.select_auth_method(McpAuthMethod::OAuth, cx);
                assert_eq!(view.state.data.auth_method, McpAuthMethod::OAuth);
                assert!(!view.show_auth_dropdown);

                // cmd-w still navigates once the dropdown is closed
                view.handle_key_down(&key_event("cmd-w"), window, cx);
                assert_eq!(
                    crate::ui_gpui::navigation_channel().take_pending(),
                    Some(crate::presentation::view_command::ViewId::Settings)
                );
            });
        });
    }

    #[gpui::test]
    async fn selecting_auth_method_updates_can_save(cx: &mut TestAppContext) {
        let view = cx.new(McpConfigureView::new);

        view.update(cx, |view: &mut McpConfigureView, cx| {
            let mut data = McpConfigureData::new();
            data.name = "Exa".to_string();
            data.url = Some("https://exa.example/mcp".to_string());
            data.auth_method = McpAuthMethod::None;
            view.set_mcp(data, true);
            assert!(view.state.data.can_save());

            view.select_auth_method(McpAuthMethod::ApiKey, cx);
            assert!(!view.state.data.can_save());
            view.state.data.api_key = "secret".to_string();
            assert!(view.state.data.can_save());

            view.select_auth_method(McpAuthMethod::Keyfile, cx);
            assert!(!view.state.data.can_save());
            view.state.data.keyfile_path = "/tmp/k.json".to_string();
            assert!(view.state.data.can_save());
        });
    }

    #[gpui::test]
    async fn save_payload_carries_auth_env_and_secrets(cx: &mut TestAppContext) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);
        let saved_id = Uuid::new_v4();

        view.update(cx, |view: &mut McpConfigureView, _cx| {
            view.set_bridge(Arc::clone(&bridge));

            let mut data = McpConfigureData::new();
            data.id = Some(saved_id.to_string());
            data.name = "Exa".to_string();
            data.url = Some("https://exa.example/mcp".to_string());
            data.auth_method = McpAuthMethod::ApiKey;
            data.env_var_name = "EXA_API_KEY".to_string();
            data.env = Some(vec![("EXA_API_KEY".to_string(), String::new())]);
            data.api_key = "secret".to_string();
            view.set_mcp(data, false);
            view.emit_save_mcp_config();
        });

        match user_rx.recv().expect("save mcp config event") {
            UserEvent::SaveMcpConfig {
                id,
                config,
                secrets,
            } => {
                assert_eq!(id, saved_id);
                assert_eq!(config.auth_type, crate::mcp::McpAuthType::ApiKey);
                assert_eq!(
                    config.env_vars,
                    vec![crate::mcp::EnvVarConfig {
                        name: "EXA_API_KEY".to_string(),
                        required: true,
                        is_secret: true,
                    }]
                );
                assert_eq!(
                    secrets,
                    vec![("EXA_API_KEY".to_string(), "secret".to_string())]
                );
                assert_eq!(config.keyfile_path, None);
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn save_payload_derives_env_var_when_missing(cx: &mut TestAppContext) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);

        view.update(cx, |view: &mut McpConfigureView, _cx| {
            view.set_bridge(Arc::clone(&bridge));

            let mut data = McpConfigureData::new();
            data.name = "Manual MCP".to_string();
            data.package = "@example/manual".to_string();
            data.command = "npx".to_string();
            data.auth_method = McpAuthMethod::ApiKey;
            data.env_var_name = "API_KEY".to_string();
            data.env = None;
            data.api_key = "typed-key".to_string();
            view.set_mcp(data, true);
            view.emit_save_mcp_config();
        });

        match user_rx.recv().expect("save mcp config event") {
            UserEvent::SaveMcpConfig {
                config, secrets, ..
            } => {
                assert_eq!(config.auth_type, crate::mcp::McpAuthType::ApiKey);
                assert_eq!(
                    config.env_vars,
                    vec![crate::mcp::EnvVarConfig {
                        name: "API_KEY".to_string(),
                        required: true,
                        is_secret: true,
                    }]
                );
                assert_eq!(
                    secrets,
                    vec![("API_KEY".to_string(), "typed-key".to_string())]
                );
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }
    }

    #[gpui::test]
    async fn save_payload_carries_keyfile_path(cx: &mut TestAppContext) {
        let (bridge, user_rx) = make_bridge();
        let view = cx.new(McpConfigureView::new);

        view.update(cx, |view: &mut McpConfigureView, _cx| {
            view.set_bridge(Arc::clone(&bridge));

            let mut data = McpConfigureData::new();
            data.name = "Keyfile MCP".to_string();
            data.package = "@example/keyfile".to_string();
            data.command = "npx".to_string();
            data.auth_method = McpAuthMethod::Keyfile;
            data.keyfile_path = "/tmp/service-key.json".to_string();
            data.env = Some(vec![("FILESYSTEM_TOKEN".to_string(), String::new())]);
            view.set_mcp(data, true);
            view.emit_save_mcp_config();
        });

        match user_rx.recv().expect("save mcp config event") {
            UserEvent::SaveMcpConfig {
                config, secrets, ..
            } => {
                assert_eq!(config.auth_type, crate::mcp::McpAuthType::Keyfile);
                assert_eq!(
                    config.keyfile_path,
                    Some(std::path::PathBuf::from("/tmp/service-key.json"))
                );
                assert_eq!(
                    config.env_vars,
                    vec![crate::mcp::EnvVarConfig {
                        name: "FILESYSTEM_TOKEN".to_string(),
                        required: true,
                        is_secret: true,
                    }]
                );
                assert!(secrets.is_empty());
            }
            other => panic!("expected SaveMcpConfig event, got {other:?}"),
        }
    }
}
