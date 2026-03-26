//! API Key Manager View — CRUD screen for OS keychain-stored API keys.
//!
//! Displays a list of stored keys with masked values and "Used by" cross-refs.
//! Allows adding new keys, editing existing ones, and deleting keys.

mod ime;
mod render;

use gpui::FocusHandle;
use std::sync::Arc;

use crate::events::types::UserEvent;
use crate::presentation::view_command::{ApiKeyInfo, ViewCommand};
use crate::ui_gpui::bridge::GpuiBridge;

/// Editing mode for the add/edit form.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum EditMode {
    /// Not editing — just viewing the list.
    Idle,
    /// Adding a new key.
    Adding,
    /// Editing an existing key (label is fixed).
    Editing { label: String },
}

/// Active text field in the form.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum ActiveField {
    Label,
    Value,
}

pub struct ApiKeyManagerState {
    /// All known API key entries from the presenter.
    pub keys: Vec<ApiKeyInfo>,
    /// Current editing mode.
    pub(super) edit_mode: EditMode,
    /// Label text field content.
    pub(super) label_input: String,
    /// Value (secret) text field content.
    pub(super) value_input: String,
    /// Whether the value field is visually masked.
    pub(super) mask_value: bool,
    /// Which field is active for text input.
    pub(super) active_field: Option<ActiveField>,
    /// Error message to display (e.g. validation).
    pub(super) error: Option<String>,
}

impl ApiKeyManagerState {
    const fn new() -> Self {
        Self {
            keys: Vec::new(),
            edit_mode: EditMode::Idle,
            label_input: String::new(),
            value_input: String::new(),
            mask_value: true,
            active_field: None,
            error: None,
        }
    }

    fn start_adding(&mut self) {
        self.edit_mode = EditMode::Adding;
        self.label_input.clear();
        self.value_input.clear();
        self.mask_value = true;
        self.active_field = Some(ActiveField::Label);
        self.error = None;
    }

    fn start_editing(&mut self, label: &str) {
        self.edit_mode = EditMode::Editing {
            label: label.to_string(),
        };
        self.label_input = label.to_string();
        self.value_input.clear();
        self.mask_value = true;
        self.active_field = Some(ActiveField::Value);
        self.error = None;
    }

    fn cancel_edit(&mut self) {
        self.edit_mode = EditMode::Idle;
        self.label_input.clear();
        self.value_input.clear();
        self.mask_value = true;
        self.active_field = None;
        self.error = None;
    }
}

pub struct ApiKeyManagerView {
    pub(super) state: ApiKeyManagerState,
    pub(super) bridge: Option<Arc<GpuiBridge>>,
    pub(super) focus_handle: FocusHandle,
    pub(super) ime_marked_byte_count: usize,
}

impl ApiKeyManagerView {
    pub fn new(cx: &mut gpui::Context<Self>) -> Self {
        Self {
            state: ApiKeyManagerState::new(),
            bridge: None,
            focus_handle: cx.focus_handle(),
            ime_marked_byte_count: 0,
        }
    }

    pub fn set_bridge(&mut self, bridge: Arc<GpuiBridge>) {
        self.bridge = Some(bridge);
        self.emit(&UserEvent::RefreshApiKeys);
    }

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
            ViewCommand::ApiKeysListed { keys } => {
                self.state.keys = keys;
                cx.notify();
            }
            ViewCommand::ApiKeyStored { .. } | ViewCommand::ApiKeyDeleted { .. } => {
                self.state.cancel_edit();
                cx.notify();
            }
            _ => {}
        }
    }

    // ── form actions ────────────────────────────────────────────────

    fn save_current(&mut self) {
        let label = self.state.label_input.trim().to_string();
        let value = self.state.value_input.trim().to_string();

        if label.is_empty() {
            self.state.error = Some("Label cannot be empty".to_string());
            return;
        }
        if value.is_empty() {
            self.state.error = Some("API key value cannot be empty".to_string());
            return;
        }

        self.emit(&UserEvent::StoreApiKey { label, value });
    }

    fn delete_key(&mut self, label: &str) {
        self.emit(&UserEvent::DeleteApiKey {
            label: label.to_string(),
        });
    }

    fn active_text(&self) -> &str {
        match self.state.active_field {
            Some(ActiveField::Label) => &self.state.label_input,
            Some(ActiveField::Value) => &self.state.value_input,
            None => "",
        }
    }

    fn set_active_text(&mut self, text: String) {
        match self.state.active_field {
            Some(ActiveField::Label) => self.state.label_input = text,
            Some(ActiveField::Value) => self.state.value_input = text,
            None => {}
        }
    }

    fn push_active_text(&mut self, s: &str) {
        match self.state.active_field {
            Some(ActiveField::Label) => self.state.label_input.push_str(s),
            Some(ActiveField::Value) => self.state.value_input.push_str(s),
            None => {}
        }
    }

    fn truncate_active_text(&mut self, at: usize) {
        match self.state.active_field {
            Some(ActiveField::Label) => self.state.label_input.truncate(at),
            Some(ActiveField::Value) => self.state.value_input.truncate(at),
            None => {}
        }
    }

    fn active_text_len(&self) -> usize {
        self.active_text().len()
    }

    fn sanitized_clipboard_text(text: &str) -> String {
        text.trim_matches(|c| c == '\r' || c == '\n').to_string()
    }

    // ── render helpers ──────────────────────────────────────────────
}

#[cfg(test)]
mod tests {
    #![allow(clippy::future_not_send)]

    use super::*;
    use crate::presentation::view_command::ViewId;

    use gpui::{AppContext, EntityInputHandler, TestAppContext};

    fn key_info(label: &str, masked_value: &str, used_by: &[&str]) -> ApiKeyInfo {
        ApiKeyInfo {
            label: label.to_string(),
            masked_value: masked_value.to_string(),
            used_by: used_by.iter().map(|value| (*value).to_string()).collect(),
        }
    }

    #[gpui::test]
    async fn handle_command_updates_key_list_and_resets_edit_state(cx: &mut TestAppContext) {
        let view = cx.new(ApiKeyManagerView::new);

        view.update(cx, |view: &mut ApiKeyManagerView, cx| {
            view.state.start_adding();
            view.state.label_input = "anthropic".to_string();
            view.state.value_input = "sk-secret".to_string();
            view.state.error = Some("boom".to_string());

            view.handle_command(
                ViewCommand::ApiKeysListed {
                    keys: vec![
                        key_info("anthropic", "••••1234", &["Claude"]),
                        key_info("openai", "••••5678", &[]),
                    ],
                },
                cx,
            );

            assert_eq!(view.state.keys.len(), 2);
            assert_eq!(view.state.keys[0].label, "anthropic");
            assert_eq!(view.state.edit_mode, EditMode::Adding);

            view.handle_command(
                ViewCommand::ApiKeyStored {
                    label: "anthropic".to_string(),
                },
                cx,
            );

            assert_eq!(view.state.edit_mode, EditMode::Idle);
            assert!(view.state.label_input.is_empty());
            assert!(view.state.value_input.is_empty());
            assert!(view.state.error.is_none());

            view.state.start_editing("openai");
            view.state.value_input = "replacement".to_string();
            view.handle_command(
                ViewCommand::ApiKeyDeleted {
                    label: "openai".to_string(),
                },
                cx,
            );
            assert_eq!(view.state.edit_mode, EditMode::Idle);
            assert!(view.state.active_field.is_none());
        });
    }

    #[gpui::test]
    async fn save_current_validates_and_emits_store_event(cx: &mut TestAppContext) {
        let (user_tx, user_rx) = flume::bounded(8);
        let (_view_tx, view_rx) = flume::bounded(8);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(ApiKeyManagerView::new);

        view.update(cx, |view: &mut ApiKeyManagerView, _cx| {
            view.set_bridge(Arc::clone(&bridge));
        });

        assert_eq!(
            user_rx.recv().expect("refresh event"),
            UserEvent::RefreshApiKeys
        );

        view.update(cx, |view: &mut ApiKeyManagerView, _cx| {
            view.state.start_adding();
            view.state.label_input.clear();
            view.state.value_input = "secret".to_string();
            view.save_current();
            assert_eq!(view.state.error.as_deref(), Some("Label cannot be empty"));

            view.state.label_input = "anthropic".to_string();
            view.state.value_input.clear();
            view.save_current();
            assert_eq!(
                view.state.error.as_deref(),
                Some("API key value cannot be empty")
            );

            view.state.value_input = "  sk-live  ".to_string();
            view.save_current();
        });

        assert_eq!(
            user_rx.recv().expect("store event"),
            UserEvent::StoreApiKey {
                label: "anthropic".to_string(),
                value: "sk-live".to_string(),
            }
        );
    }

    #[test]
    fn text_entry_and_key_handling_follow_active_field_rules() {
        let mut view = ApiKeyManagerState::new();

        view.start_adding();
        assert_eq!(view.active_field, Some(ActiveField::Label));

        let mut manager = ApiKeyManagerState::new();
        manager.start_adding();
        assert_eq!(manager.edit_mode, EditMode::Adding);
        assert_eq!(manager.active_field, Some(ActiveField::Label));

        let mut key_manager = ApiKeyManagerState::new();
        key_manager.start_adding();

        let mut wrapper = ApiKeyManagerState::new();
        wrapper.start_adding();

        let mut state = ApiKeyManagerState::new();
        state.start_adding();
        assert_eq!(state.edit_mode, EditMode::Adding);
        assert_eq!(state.active_field, Some(ActiveField::Label));

        let mut view = ApiKeyManagerState::new();
        view.start_adding();
        assert_eq!(view.edit_mode, EditMode::Adding);

        let mut manager = ApiKeyManagerState::new();
        manager.start_adding();
        manager.label_input = "anthropic".to_string();
        assert_eq!(manager.label_input, "anthropic");

        manager.active_field = Some(ActiveField::Value);
        manager.value_input = "sk-".to_string();
        assert_eq!(manager.value_input, "sk-");

        manager.value_input.push_str("live");
        assert_eq!(manager.value_input, "sk-live");

        manager.value_input.pop();
        assert_eq!(manager.value_input, "sk-liv");

        manager.active_field = Some(ActiveField::Label);
        assert_eq!(manager.active_field, Some(ActiveField::Label));

        manager.start_editing("anthropic");
        assert_eq!(manager.active_field, Some(ActiveField::Value));
        assert_eq!(manager.label_input, "anthropic");

        manager.cancel_edit();
        assert_eq!(manager.edit_mode, EditMode::Idle);
        assert!(manager.active_field.is_none());
    }

    #[test]
    fn sanitized_clipboard_text_trims_only_newlines() {
        assert_eq!(
            ApiKeyManagerView::sanitized_clipboard_text("\nsecret\r\n"),
            "secret"
        );
        assert_eq!(
            ApiKeyManagerView::sanitized_clipboard_text("  secret  "),
            "  secret  "
        );
    }

    fn clear_navigation_requests() {
        while crate::ui_gpui::navigation_channel()
            .take_pending()
            .is_some()
        {}
    }

    #[gpui::test]
    async fn delete_key_and_escape_navigation_emit_expected_user_and_navigation_actions(
        cx: &mut TestAppContext,
    ) {
        clear_navigation_requests();
        let (user_tx, user_rx) = flume::bounded(8);
        let (_view_tx, view_rx) = flume::bounded(8);
        let bridge = Arc::new(GpuiBridge::new(user_tx, view_rx));
        let view = cx.new(ApiKeyManagerView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ApiKeyManagerView, cx| {
                view.set_bridge(Arc::clone(&bridge));
                view.handle_command(
                    ViewCommand::ApiKeysListed {
                        keys: vec![key_info("openai", "••••5678", &["Default"])],
                    },
                    cx,
                );
                view.state.start_editing("openai");
                view.state.value_input = "replacement".to_string();
                view.delete_key("openai");
                view.handle_command(
                    ViewCommand::ApiKeyDeleted {
                        label: "openai".to_string(),
                    },
                    cx,
                );
                assert_eq!(view.state.edit_mode, EditMode::Idle);
                assert!(view.state.active_field.is_none());

                view.handle_key_down(
                    &gpui::KeyDownEvent {
                        keystroke: gpui::Keystroke::parse("escape").expect("escape keystroke"),
                        is_held: false,
                        prefer_character_input: false,
                    },
                    window,
                    cx,
                );

                assert_eq!(
                    crate::ui_gpui::navigation_channel().take_pending(),
                    Some(ViewId::ProfileEditor)
                );
            });
        });

        assert_eq!(
            user_rx.recv().expect("refresh event"),
            UserEvent::RefreshApiKeys
        );
        assert_eq!(
            user_rx.recv().expect("delete event"),
            UserEvent::DeleteApiKey {
                label: "openai".to_string()
            }
        );
        assert!(
            user_rx.try_recv().is_err(),
            "unexpected additional user events"
        );
    }

    #[gpui::test]
    async fn input_handler_tracks_marked_text_replacement_and_cursor_position(
        cx: &mut TestAppContext,
    ) {
        let view = cx.new(ApiKeyManagerView::new);
        let mut visual_cx = cx.add_empty_window().clone();

        visual_cx.update(|window, app| {
            view.update(app, |view: &mut ApiKeyManagerView, cx| {
                view.state.start_adding();
                view.replace_text_in_range(None, "anth", window, cx);
                assert_eq!(view.state.label_input, "anth");
                assert_eq!(
                    view.text_for_range(0..2, &mut None, window, cx),
                    Some("an".to_string())
                );

                view.replace_and_mark_text_in_range(None, "ro", None, window, cx);
                assert_eq!(view.state.label_input, "anthro");
                assert_eq!(view.marked_text_range(window, cx), Some(4..6));

                view.replace_text_in_range(None, "pic", window, cx);
                assert_eq!(view.state.label_input, "anthpic");
                assert_eq!(view.marked_text_range(window, cx), None);

                let selection = view
                    .selected_text_range(false, window, cx)
                    .expect("selection range");
                let len = "anthpic".encode_utf16().count();
                assert_eq!(selection.range, len..len);

                view.unmark_text(window, cx);
                assert_eq!(view.marked_text_range(window, cx), None);
            });
        });
    }
}
