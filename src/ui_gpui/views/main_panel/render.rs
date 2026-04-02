//! `impl Render for MainPanel` — view-routing render and keyboard dispatch.
//!
//! Render logic: lazy init on first render, navigation channel polling,
//! focus management, view selection, and global action/key-down handlers.
//!
//! @plan PLAN-20260325-ISSUE11B.P02
//! @plan PLAN-20250130-GPUIREDUX.P11

use crate::events::types::UserEvent;
use crate::presentation::view_command::ViewId;
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, Focusable, MouseButton};

use super::routing::{NavigateBack, NavigateToHistory, NavigateToSettings, NewConversation};
use super::startup::MainPanelAppState;
use super::MainPanel;

impl MainPanel {
    /// Lazy-init + navigation polling, called at the top of every render frame.
    fn prepare_frame(&mut self, window: &mut gpui::Window, cx: &mut gpui::Context<Self>) {
        if !self.is_initialized() {
            self.init(cx);
            window.focus(&self.focus_handle, cx);
            println!(">>> MainPanel first render - focused <<<");
        }
        if !self.runtime_started && self.is_initialized() {
            self.start_runtime(cx);
        }
        if self.child_observations.is_empty() {
            if let Some(ref chat_view) = self.chat_view {
                self.child_observations.push(cx.observe_in(
                    chat_view,
                    window,
                    |_, _, _window, cx| cx.notify(),
                ));
            }
        }
        self.poll_navigation_channel(cx);
    }

    fn poll_navigation_channel(&mut self, cx: &mut gpui::Context<Self>) {
        if !crate::ui_gpui::navigation_channel().has_pending() {
            return;
        }
        if let Some(view_id) = crate::ui_gpui::navigation_channel().take_pending() {
            println!(">>> MainPanel::render - Processing navigation to {view_id:?} <<<");
            tracing::info!("MainPanel: Processing navigation request to {:?}", view_id);
            if view_id == ViewId::ModelSelector {
                if let Some(ref model_selector) = self.model_selector_view {
                    model_selector.update(cx, |view, _cx| {
                        view.request_models();
                    });
                }
            }
            if view_id == ViewId::ProfileEditor {
                if let Some(app_state) = cx.try_global::<MainPanelAppState>() {
                    let _ = app_state.gpui_bridge.emit(UserEvent::RefreshApiKeys);
                }
            }
            self.navigation.navigate(view_id);
            cx.notify();
        }
    }

    /// Focus the child view that matches the current navigation target.
    fn focus_current_view(
        &self,
        current: ViewId,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) {
        macro_rules! focus_child {
            ($view:expr) => {
                if let Some(v) = &$view {
                    let focus = v.read(cx).focus_handle(cx).clone();
                    window.focus(&focus, cx);
                    return;
                }
            };
        }
        match current {
            ViewId::Chat => focus_child!(self.chat_view),
            ViewId::Settings => focus_child!(self.settings_view),
            ViewId::History => focus_child!(self.history_view),
            ViewId::ProfileEditor => focus_child!(self.profile_editor_view),
            ViewId::McpAdd => focus_child!(self.mcp_add_view),
            ViewId::ModelSelector => focus_child!(self.model_selector_view),
            ViewId::ApiKeyManager => focus_child!(self.api_key_manager_view),
            ViewId::ErrorLog => focus_child!(self.error_log_view),
            ViewId::McpConfigure => focus_child!(self.mcp_configure_view),
        }
        window.focus(&self.focus_handle, cx);
    }

    /// Render the currently-selected view content area.
    fn render_view_content(&self, current: ViewId) -> impl IntoElement {
        div()
            .flex_1()
            .min_h_0()
            .overflow_hidden()
            .when(current == ViewId::Chat, |d| {
                Self::render_child_or_placeholder(d, self.chat_view.as_ref(), "Loading chat...")
            })
            .when(current == ViewId::History, |d| {
                Self::render_child_or_placeholder(
                    d,
                    self.history_view.as_ref(),
                    "Loading history...",
                )
            })
            .when(current == ViewId::Settings, |d| {
                Self::render_child_or_placeholder(
                    d,
                    self.settings_view.as_ref(),
                    "Loading settings...",
                )
            })
            .when(current == ViewId::ModelSelector, |d| {
                Self::render_child_or_placeholder(
                    d,
                    self.model_selector_view.as_ref(),
                    "Loading model selector...",
                )
            })
            .when(current == ViewId::ProfileEditor, |d| {
                Self::render_child_or_placeholder(
                    d,
                    self.profile_editor_view.as_ref(),
                    "Loading profile editor...",
                )
            })
            .when(current == ViewId::McpAdd, |d| {
                Self::render_child_or_placeholder(
                    d,
                    self.mcp_add_view.as_ref(),
                    "Loading MCP add...",
                )
            })
            .when(current == ViewId::McpConfigure, |d| {
                Self::render_child_or_placeholder(
                    d,
                    self.mcp_configure_view.as_ref(),
                    "Loading MCP configure...",
                )
            })
            .when(current == ViewId::ApiKeyManager, |d| {
                Self::render_child_or_placeholder(
                    d,
                    self.api_key_manager_view.as_ref(),
                    "Loading API key manager...",
                )
            })
            .when(current == ViewId::ErrorLog, |d| {
                Self::render_child_or_placeholder(
                    d,
                    self.error_log_view.as_ref(),
                    "Loading error log...",
                )
            })
    }

    fn render_child_or_placeholder<V: gpui::Render>(
        d: gpui::Div,
        view: Option<&gpui::Entity<V>>,
        placeholder: &str,
    ) -> gpui::Div {
        if let Some(v) = view {
            d.child(v.clone())
        } else {
            d.child(div().child(placeholder.to_string()))
        }
    }
}

impl gpui::Render for MainPanel {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        self.prepare_frame(window, cx);

        let current_view = self.navigation.current();
        self.focus_current_view(current_view, window, cx);

        let focus_handle = self.focus_handle.clone();

        div()
            .id("main-panel")
            .flex()
            .flex_col()
            .size_full()
            .bg(Theme::bg_darkest())
            .track_focus(&self.focus_handle)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |_this, _, window, cx| {
                    println!(">>> MainPanel clicked - requesting focus <<<");
                    window.focus(&focus_handle, cx);
                    cx.notify();
                }),
            )
            .on_action(cx.listener(|_this, _: &NavigateToHistory, _window, _cx| {
                crate::ui_gpui::navigation_channel().request_navigate(ViewId::History);
            }))
            .on_action(cx.listener(|_this, _: &NavigateToSettings, _window, _cx| {
                crate::ui_gpui::navigation_channel().request_navigate(ViewId::Settings);
            }))
            .on_action(cx.listener(|_this, _: &NewConversation, _window, _cx| {
                crate::ui_gpui::navigation_channel().request_navigate(ViewId::Chat);
            }))
            .on_action(cx.listener(|this, _: &NavigateBack, _window, _cx| {
                if this.navigation.current() != ViewId::Chat {
                    this.navigation.navigate_back();
                }
            }))
            .on_key_down(
                cx.listener(|this, event: &gpui::KeyDownEvent, _window, _cx| {
                    let key = &event.keystroke.key;
                    let modifiers = &event.keystroke.modifiers;
                    if modifiers.platform || modifiers.control {
                        return;
                    }
                    let current = this.navigation.current();
                    match current {
                        ViewId::Settings => match key.as_str() {
                            "escape" => {
                                this.navigation.navigate_back();
                            }
                            "+" => {
                                crate::ui_gpui::navigation_channel()
                                    .request_navigate(ViewId::ProfileEditor);
                            }
                            "=" if modifiers.shift => {
                                crate::ui_gpui::navigation_channel()
                                    .request_navigate(ViewId::ProfileEditor);
                            }
                            "m" => {
                                crate::ui_gpui::navigation_channel()
                                    .request_navigate(ViewId::McpAdd);
                            }
                            _ => {}
                        },
                        ViewId::ModelSelector => {
                            if key == "escape" {
                                this.navigation.navigate_back();
                            }
                        }
                        _ => {}
                    }
                }),
            )
            .child(self.render_view_content(current_view))
    }
}
