//! Command handling for `McpConfigureView`.

use super::{McpAuthMethod, McpConfigureView, OAuthStatus};
use crate::presentation::view_command::ViewCommand;

impl McpConfigureView {
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
