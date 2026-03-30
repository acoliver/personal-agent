//! `MainPanel::handle_command` — `ViewCommand` dispatch to child views.
//!
//! Routes each `ViewCommand` variant to the appropriate child view
//! (`ChatView`, `HistoryView`, `SettingsView`, etc.) and handles
//! navigation transitions triggered by commands.
//!
//! The top-level match groups variants by destination view(s) and
//! delegates to per-group helpers to keep cyclomatic complexity low.
//!
//! @plan PLAN-20260325-ISSUE11B.P02
//! @plan PLAN-20250130-GPUIREDUX.P11
//! @plan PLAN-20260219-NEXTGPUIREMEDIATE.P05
//! @requirement REQ-WIRE-002

use super::MainPanel;
use crate::presentation::view_command::{ViewCommand, ViewId};

impl MainPanel {
    /// Handle `ViewCommand` from the presentation layer.
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

            // ── chat-only streaming / lifecycle (forward to chat_view) ─
            ConversationMessagesLoaded { .. }
            | MessageAppended { .. }
            | ShowThinking { .. }
            | HideThinking { .. }
            | AppendStream { .. }
            | FinalizeStream { .. }
            | StreamCancelled { .. }
            | StreamError { .. }
            | AppendThinking { .. }
            | ToggleThinkingVisibility
            | ConversationCleared => self.forward_to_chat(cmd, cx),

            // ── conversation lifecycle (multi-view) ─────────────────────
            ConversationListRefreshed { .. }
            | ConversationActivated { .. }
            | ConversationCreated { .. }
            | ConversationRenamed { .. }
            | ConversationTitleUpdated { .. }
            | ConversationDeleted { .. } => self.handle_conversation_command(cmd, cx),

            // ── model selector + profile editor ─────────────────────────
            ModelSearchResults { .. } | ModelSelected { .. } | ProfileEditorLoad { .. } => {
                self.handle_model_profile_command(cmd, cx);
            }

            // ── settings + profiles ─────────────────────────────────────
            ShowSettings { .. }
            | ShowSettingsTheme { .. }
            | ChatProfilesUpdated { .. }
            | ProfileCreated { .. }
            | ProfileUpdated { .. }
            | ProfileDeleted { .. }
            | DefaultProfileChanged { .. } => self.handle_settings_profile_command(cmd, cx),

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

    fn handle_conversation_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ConversationListRefreshed { conversations } => {
                tracing::info!(
                    count = conversations.len(),
                    chat = self.chat_view.is_some(),
                    history = self.history_view.is_some(),
                    "MainPanel: ConversationListRefreshed"
                );
                let history_convs = conversations.clone();
                self.forward_to_history_and_chat(
                    |_| ViewCommand::ConversationListRefreshed {
                        conversations: history_convs,
                    },
                    ViewCommand::ConversationListRefreshed { conversations },
                    cx,
                );
            }
            ViewCommand::ConversationActivated {
                id,
                selection_generation,
            } => {
                if let Some(ref chat) = self.chat_view {
                    tracing::info!(
                        chat_view_entity_id = ?chat.entity_id(),
                        conversation_id = %id,
                        "MainPanel forwarding ConversationActivated to ChatView"
                    );
                }
                self.forward_to_history_and_chat(
                    |_| ViewCommand::ConversationActivated {
                        id,
                        selection_generation,
                    },
                    ViewCommand::ConversationActivated {
                        id,
                        selection_generation,
                    },
                    cx,
                );
            }
            ViewCommand::ConversationCreated { id, profile_id } => {
                self.forward_to_chat(ViewCommand::ConversationCreated { id, profile_id }, cx);
                self.navigation.navigate(ViewId::Chat);
                cx.notify();
            }
            ViewCommand::ConversationRenamed { id, new_title } => {
                self.forward_to_chat(ViewCommand::ConversationRenamed { id, new_title }, cx);
            }
            ViewCommand::ConversationTitleUpdated { id, title } => {
                let history_title = title.clone();
                self.forward_to_history_and_chat(
                    |_| ViewCommand::ConversationTitleUpdated {
                        id,
                        title: history_title,
                    },
                    ViewCommand::ConversationTitleUpdated { id, title },
                    cx,
                );
            }
            ViewCommand::ConversationDeleted { id } => {
                self.forward_to_history_and_chat(
                    |_| ViewCommand::ConversationDeleted { id },
                    ViewCommand::ConversationDeleted { id },
                    cx,
                );
            }
            _ => {}
        }
    }

    /// Forward a command to both `history_view` and `chat_view`.
    ///
    /// `history_cmd_fn` produces the command for history (called first so
    /// the chat arm can take ownership of `chat_cmd` without cloning).
    fn forward_to_history_and_chat(
        &self,
        history_cmd_fn: impl FnOnce(&ViewCommand) -> ViewCommand,
        chat_cmd: ViewCommand,
        cx: &mut gpui::Context<Self>,
    ) {
        if let Some(ref history) = self.history_view {
            let hcmd = history_cmd_fn(&chat_cmd);
            history.update(cx, |view, cx| {
                view.handle_command(hcmd, cx);
            });
        }
        if let Some(ref chat) = self.chat_view {
            chat.update(cx, |view, cx| {
                view.handle_command(chat_cmd, cx);
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
            ViewCommand::ShowSettings {
                profiles,
                selected_profile_id,
            }
            | ViewCommand::ChatProfilesUpdated {
                profiles,
                selected_profile_id,
            } => {
                tracing::info!(
                    count = profiles.len(),
                    default = ?selected_profile_id,
                    settings = self.settings_view.is_some(),
                    chat = self.chat_view.is_some(),
                    "MainPanel: ShowSettings/ChatProfilesUpdated"
                );
                if let Some(ref settings) = self.settings_view {
                    let profiles_clone = profiles.clone();
                    settings.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ShowSettings {
                                profiles: profiles_clone,
                                selected_profile_id,
                            },
                            cx,
                        );
                    });
                }
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(
                            ViewCommand::ChatProfilesUpdated {
                                profiles,
                                selected_profile_id,
                            },
                            cx,
                        );
                    });
                }
            }
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
            ViewCommand::DefaultProfileChanged { profile_id } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::DefaultProfileChanged { profile_id }, cx);
                    });
                }
                if let Some(ref chat) = self.chat_view {
                    chat.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::DefaultProfileChanged { profile_id }, cx);
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

    fn handle_notification_api_command(&mut self, cmd: ViewCommand, cx: &mut gpui::Context<Self>) {
        match cmd {
            ViewCommand::ShowNotification { message } => {
                if let Some(ref settings) = self.settings_view {
                    settings.update(cx, |view, cx| {
                        view.handle_command(ViewCommand::ShowNotification { message }, cx);
                    });
                }
            }
            ViewCommand::ShowError {
                title,
                message,
                severity,
            } => {
                if let Some(ref mcp_add) = self.mcp_add_view {
                    let t = title.clone();
                    let m = message.clone();
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
