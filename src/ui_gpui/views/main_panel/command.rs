//! `MainPanel::handle_command` — `ViewCommand` dispatch to child views.
//!
//! Routes each `ViewCommand` variant to the appropriate child view
//! (`ChatView`, `HistoryView`, `SettingsView`, etc.) and handles
//! navigation transitions triggered by commands.
//!
//! Store-managed commands (conversation lifecycle, streaming, thinking,
//! profiles, etc.) are NOT forwarded here. They flow exclusively through
//! the `AppStore` → snapshot subscription → `apply_store_snapshot` path.
//! Only ephemeral / non-store commands are dispatched through this method.
//!
//! @plan PLAN-20260325-ISSUE11B.P02
//! @plan PLAN-20250130-GPUIREDUX.P11
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
//! @requirement REQ-WIRE-002

use super::MainPanel;
use crate::presentation::view_command::{ViewCommand, ViewId};

impl MainPanel {
    fn is_export_notification(message: &str) -> bool {
        message.contains("Conversation saved") || message.contains("No active conversation to save")
    }

    fn is_export_error(title: &str) -> bool {
        title == "Save Conversation"
    }

    /// Handle `ViewCommand` from the presentation layer.
    ///
    /// Store-managed commands are filtered out in the bridge pump before
    /// reaching this method. Only non-store commands (navigation, model
    /// selector, MCP, notifications, export feedback, `ConversationCleared`,
    /// `ToggleThinkingVisibility`) are dispatched here.
    ///
    /// @plan PLAN-20250130-GPUIREDUX.P11
    /// @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
    /// @requirement REQ-WIRE-002
    pub fn handle_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        #[allow(clippy::enum_glob_use)]
        use ViewCommand::*;
        match cmd {
            // ── navigation ──────────────────────────────────────────────
            NavigateTo { view } => {
                tracing::info!("MainPanel: NavigateTo {:?}", view);
                self.navigation.navigate(view);
                cx.notify();
            }
            NavigateBack => {
                tracing::info!("MainPanel: NavigateBack");
                self.navigation.navigate_back();
                cx.notify();
            }

            // ── chat-only ephemeral commands (forward to chat_view) ────
            ConversationCleared
            | ToggleThinkingVisibility
            | ShowConversationExportFormat { .. } => self.forward_to_chat(cmd, cx),

            // ── model selector + profile editor ─────────────────────────
            ModelSearchResults { .. } | ModelSelected { .. } | ProfileEditorLoad { .. } => {
                self.handle_model_profile_command(cmd, cx);
            }

            // ── settings + profiles (non-store) ────────────────────────
            ShowSettingsTheme { .. }
            | ProfileCreated { .. }
            | ProfileUpdated { .. }
            | ProfileDeleted { .. } => self.handle_settings_profile_command(cmd, cx),

            // ── MCP ─────────────────────────────────────────────────────
            McpStatusChanged { .. }
            | McpServerStarted { .. }
            | McpServerFailed { .. }
            | McpConfigSaved { .. }
            | McpDeleted { .. }
            | McpRegistrySearchResults { .. }
            | McpConfigureDraftLoaded { .. } => self.handle_mcp_command(cmd, cx),

            // ── notifications + API keys ────────────────────────────────
            ShowNotification { .. }
            | ShowError { .. }
            | ApiKeysListed { .. }
            | ApiKeyStored { .. }
            | ApiKeyDeleted { .. } => self.handle_notification_api_command(cmd, cx),

            other => {
                tracing::debug!("MainPanel: command {:?} not forwarded to child view", other);
            }
        }
    }

    // ── helpers ─────────────────────────────────────────────────────────

    fn forward_to_chat(&self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        if let Some(ref chat) = self.chat_view {
            chat.update(cx, |view, cx| {
                view.handle_command(cmd, cx);
            });
        }
    }

    fn handle_model_profile_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ModelSearchResults { models } => {
                if let Some(ref model_selector) = self.model_selector_view {
                    model_selector.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ModelSearchResults { models }, cx);
                    });
                }
            }
            ViewCommand::ModelSelected {
                provider_id,
                model_id,
                provider_api_url,
                context_length,
            } => {
                if let Some(ref profile_editor) = self.profile_editor_view {
                    profile_editor.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ModelSelected {
                                provider_id,
                                model_id,
                                provider_api_url,
                                context_length,
                            },
                            cx,
                        );
                    });
                }
                self.navigation.navigate(ViewId::ProfileEditor);
                cx.notify();
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
                context_limit,
                show_thinking,
                enable_thinking,
                thinking_budget,
                system_prompt,
            } => {
                if let Some(ref profile_editor) = self.profile_editor_view {
                    profile_editor.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ProfileEditorLoad {
                                id,
                                name,
                                provider_id,
                                model_id,
                                base_url,
                                api_key_label,
                                temperature,
                                max_tokens,
                                context_limit,
                                show_thinking,
                                enable_thinking,
                                thinking_budget,
                                system_prompt,
                            },
                            cx,
                        );
                    });
                }
                self.navigation.navigate(ViewId::ProfileEditor);
                cx.notify();
            }
            _ => {}
        }
    }

    fn handle_settings_profile_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ShowSettingsTheme {
                options,
                selected_slug,
            } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ShowSettingsTheme {
                                options,
                                selected_slug,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::ProfileCreated { id, name } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ProfileCreated { id, name }, cx);
                    });
                }
            }
            ViewCommand::ProfileUpdated { id, name } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ProfileUpdated { id, name }, cx);
                    });
                }
            }
            ViewCommand::ProfileDeleted { id } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ProfileDeleted { id }, cx);
                    });
                }
            }
            _ => {}
        }
    }

    fn handle_mcp_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::McpStatusChanged { id, status } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::McpStatusChanged { id, status }, cx);
                    });
                }
            }
            ViewCommand::McpServerStarted {
                id,
                name,
                tool_count,
                enabled,
            } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::McpServerStarted {
                                id,
                                name,
                                tool_count,
                                enabled,
                            },
                            cx,
                        );
                    });
                }
            }
            ViewCommand::McpServerFailed { id, error } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::McpServerFailed { id, error }, cx);
                    });
                }
            }
            ViewCommand::McpConfigSaved { id, name } => {
                if let Some(ref settings) = self.settings_view {
                    let name_clone = name.clone();
                    settings.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::McpConfigSaved {
                                id,
                                name: name_clone,
                            },
                            cx,
                        );
                    });
                }
                if let Some(ref mcp_configure) = self.mcp_configure_view {
                    mcp_configure.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::McpConfigSaved { id, name }, cx);
                    });
                }
            }
            ViewCommand::McpDeleted { id } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::McpDeleted { id }, cx);
                    });
                }
            }
            cmd @ (ViewCommand::McpRegistrySearchResults { .. }
            | ViewCommand::McpConfigureDraftLoaded { .. }) => {
                self.handle_mcp_registry_or_draft(cmd, cx);
            }
            _ => {}
        }
    }

    fn handle_mcp_registry_or_draft(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::McpRegistrySearchResults { results } => {
                if let Some(ref mcp_add) = self.mcp_add_view {
                    mcp_add.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::McpRegistrySearchResults { results }, cx);
                    });
                }
            }
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
                if let Some(ref mcp_configure) = self.mcp_configure_view {
                    mcp_configure.update(cx, |view, cx| {
                        view.handle_command(
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
                            },
                            cx,
                        );
                    });
                }
            }
            _ => {}
        }
    }

    fn forward_export_notification_to_chat(&self, message: &str, cx: &mut gpui::Context<Self>) {
        if let Some(ref chat) = self.chat_view {
            if Self::is_export_notification(message) {
                let chat_message = message.to_string();
                chat.update(cx, |view, cx| {
                    view.handle_command(
                        ViewCommand::ShowNotification {
                            message: chat_message,
                        },
                        cx,
                    );
                });
            }
        }
    }

    fn forward_notification_to_settings(&self, message: String, cx: &mut gpui::Context<Self>) {
        if let Some(ref settings) = self.settings_view {
            settings.update(cx, |view, cx| {
                view.handle_command(ViewCommand::ShowNotification { message }, cx);
            });
        }
    }

    fn forward_export_error_to_chat(
        &self,
        title: &str,
        message: &str,
        severity: crate::presentation::view_command::ErrorSeverity,
        cx: &mut gpui::Context<Self>,
    ) {
        if let Some(ref chat) = self.chat_view {
            if Self::is_export_error(title) {
                let t = title.to_string();
                let m = message.to_string();
                chat.update(cx, |view, cx| {
                    view.handle_command(
                        ViewCommand::ShowError {
                            title: t,
                            message: m,
                            severity,
                        },
                        cx,
                    );
                });
            }
        }
    }

    fn forward_error_to_mcp_add(
        &self,
        title: &str,
        message: &str,
        severity: crate::presentation::view_command::ErrorSeverity,
        cx: &mut gpui::Context<Self>,
    ) {
        if let Some(ref mcp_add) = self.mcp_add_view {
            let t = title.to_string();
            let m = message.to_string();
            mcp_add.update(cx, |view, cx| {
                view.handle_command(
                    ViewCommand::ShowError {
                        title: t,
                        message: m,
                        severity,
                    },
                    cx,
                );
            });
        }
    }

    fn forward_error_to_mcp_configure(
        &self,
        title: String,
        message: String,
        severity: crate::presentation::view_command::ErrorSeverity,
        cx: &mut gpui::Context<Self>,
    ) {
        if let Some(ref mcp_configure) = self.mcp_configure_view {
            mcp_configure.update(cx, |view, cx| {
                view.handle_command(
                    ViewCommand::ShowError {
                        title,
                        message,
                        severity,
                    },
                    cx,
                );
            });
        }
    }

    fn handle_notification_api_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ShowNotification { message } => {
                self.forward_export_notification_to_chat(&message, cx);
                self.forward_notification_to_settings(message, cx);
            }
            ViewCommand::ShowError {
                title,
                message,
                severity,
            } => {
                self.forward_export_error_to_chat(&title, &message, severity, cx);
                self.forward_error_to_mcp_add(&title, &message, severity, cx);
                self.forward_error_to_mcp_configure(title, message, severity, cx);
            }
            ViewCommand::ApiKeysListed { keys } => {
                tracing::info!(
                    key_count = keys.len(),
                    "MainPanel: forwarding ApiKeysListed to GPUI views"
                );
                if let Some(ref akm) = self.api_key_manager_view {
                    let keys_clone = keys.clone();
                    akm.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ApiKeysListed { keys: keys_clone }, cx);
                    });
                }
                if let Some(ref pe) = self.profile_editor_view {
                    pe.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ApiKeysListed { keys }, cx);
                    });
                }
            }
            ViewCommand::ApiKeyStored { label } => {
                if let Some(ref akm) = self.api_key_manager_view {
                    akm.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ApiKeyStored { label }, cx);
                    });
                }
            }
            ViewCommand::ApiKeyDeleted { label } => {
                if let Some(ref akm) = self.api_key_manager_view {
                    akm.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ApiKeyDeleted { label }, cx);
                    });
                }
            }
            _ => {}
        }
    }
}
