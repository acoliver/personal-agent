//! Chat view bar and dropdown render subtrees.
//! Contains `render_top_bar`, `render_title_bar`, `render_conversation_dropdown`,
//! and `render_profile_dropdown`. These are navigation-chrome render methods that
//! sit above the chat area.
//! @plan PLAN-20260325-ISSUE11B.P02

use super::state::StreamingState;
use super::ChatView;
use crate::events::types::UserEvent;
use crate::presentation::view_command::{AppMode, ConversationSummary, ProfileSummary};
use crate::ui_gpui::components::copy_icons::copy_icon;
use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::views::main_panel::MainPanelAppState;
use gpui::{div, prelude::*, px, FontWeight, MouseButton, SharedString};
/// Height of the top bar.
const TOP_BAR_HEIGHT: f32 = 44.0;

/// Height of the title bar where selectors live.
const TITLE_BAR_HEIGHT: f32 = 32.0;

/// Gap below bars before dropdown appears (negative = move up).
const DROPDOWN_GAP: f32 = -1.0;

macro_rules! icon_btn {
    ($id:expr, $label:expr, $active:expr, $handler:expr) => {
        div()
            .id($id)
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when($active, |d| d.bg(Theme::bg_dark()))
            .when(!$active, |d| {
                d.bg(Theme::bg_darker()).hover(|s| s.bg(Theme::bg_dark()))
            })
            .text_size(px(Theme::font_size_body()))
            .text_color(Theme::text_primary())
            .child($label)
            .on_mouse_down(MouseButton::Left, $handler)
    };
}

const TOOLBAR_ICON_SIZE: f32 = 16.0;

impl ChatView {
    fn render_copy_conversation_button(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("btn-copy-conversation")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(Theme::bg_darker())
            .hover(|s| s.bg(Theme::bg_dark()))
            .active(|s| s.bg(Theme::bg_dark()))
            .child(copy_icon(TOOLBAR_ICON_SIZE).text_color(Theme::text_primary()))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    let Some(conversation_id) = this.current_or_active_conversation_id() else {
                        this.state.export_feedback_message =
                            Some("No active conversation to copy".to_string());
                        this.state.export_feedback_is_error = true;
                        this.state.export_feedback_path = None;
                        cx.notify();
                        return;
                    };

                    match super::render_bars_export::build_conversation_export_content(
                        conversation_id,
                        &this.state.conversation_title,
                        this.state
                            .selected_conversation()
                            .map(|conversation| conversation.title.as_str()),
                        this.state.selected_profile_id,
                        &this.state.messages,
                        this.state.conversation_export_format,
                    ) {
                        Ok(content) => {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(content));
                            this.state.export_feedback_message =
                                Some("Conversation copied to clipboard".to_string());
                            this.state.export_feedback_is_error = false;
                            this.state.export_feedback_path = None;
                            cx.notify();
                        }
                        Err(message) => {
                            this.state.export_feedback_message = Some(message);
                            this.state.export_feedback_is_error = true;
                            this.state.export_feedback_path = None;
                            cx.notify();
                        }
                    }
                }),
            )
    }

    /// Render the top bar with icon, title, YOLO badge, and toolbar buttons
    /// @plan PLAN-20250130-GPUIREDUX.P04
    pub(super) fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let yolo_active = self.state.yolo_mode;
        let is_popout = cx
            .try_global::<MainPanelAppState>()
            .is_some_and(|s| s.app_mode == AppMode::Popout);

        div()
            .id("chat-top-bar")
            .flex_shrink_0()
            .h(px(44.0 * Theme::ui_scale()))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .overflow_hidden()
            .pr(px(12.0))
            .pl(px(if is_popout { 72.0 } else { 12.0 }))
            .flex()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_body()))
                            .font_weight(FontWeight::BOLD)
                            .text_color(Theme::text_primary())
                            .child("PersonalAgent"),
                    )
                    .when(yolo_active, |d| d.child(Self::render_yolo_badge())),
            )
            .child(self.render_toolbar_buttons(cx))
    }

    /// Persistent YOLO mode indicator badge.
    fn render_yolo_badge() -> impl IntoElement {
        div()
            .id("yolo-badge")
            .px(px(6.0))
            .py(px(2.0))
            .rounded(px(Theme::RADIUS_SM))
            .bg(Theme::warning())
            .text_size(px(Theme::font_size_ui()))
            .text_color(Theme::bg_darkest())
            .font_weight(FontWeight::BOLD)
            .child("YOLO")
    }

    /// Emoji filter toggle button with smiley icon.
    fn render_emoji_filter_button(
        filter_emoji: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("btn-emoji-filter")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when(filter_emoji, |d| d.bg(Theme::bg_dark()))
            .when(!filter_emoji, |d| {
                d.bg(Theme::bg_darker()).hover(|s| s.bg(Theme::bg_dark()))
            })
            .child(if filter_emoji {
                crate::ui_gpui::components::emoji_filter_icon::smile_icon(16.0)
                    .text_color(Theme::text_primary())
            } else {
                crate::ui_gpui::components::emoji_filter_icon::smile_x_icon(16.0)
                    .text_color(Theme::text_primary())
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    let current = this.state.filter_emoji;
                    tracing::info!(
                        "Emoji filter button CLICKED! Current state: {}, will toggle to: {}",
                        current,
                        !current
                    );
                    this.emit(UserEvent::ToggleEmojiFilter);
                    cx.notify();
                }),
            )
    }

    /// Right-side toolbar: [T][E][Y][R][H (popup only)][MD/TXT/JSON][Save][Popout/Popin][Settings][Exit]
    fn render_toolbar_buttons(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let show_thinking = self.state.show_thinking;
        let filter_emoji = self.state.filter_emoji;
        let yolo_active = self.state.yolo_mode;
        let app_mode = cx
            .try_global::<MainPanelAppState>()
            .map(|s| s.app_mode)
            .unwrap_or_default();
        let is_popout = app_mode == AppMode::Popout;
        let show_history_btn = !is_popout || !self.state.sidebar_visible;

        div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(icon_btn!(
                "btn-thinking",
                "T",
                show_thinking,
                cx.listener(|this, _, _window, _cx| {
                    tracing::info!("Toggle thinking clicked - emitting UserEvent");
                    this.emit(UserEvent::ToggleThinking);
                })
            ))
            .child(Self::render_emoji_filter_button(filter_emoji, cx))
            .child(icon_btn!(
                "btn-yolo",
                "Y",
                yolo_active,
                cx.listener(|this, _, _window, cx| {
                    let next = !this.state.yolo_mode;
                    this.emit(UserEvent::SetToolApprovalYoloMode { enabled: next });
                    cx.notify();
                })
            ))
            .child(icon_btn!(
                "btn-rename",
                "R",
                false,
                cx.listener(|this, _, _window, cx| {
                    this.start_rename_conversation(cx);
                })
            ))
            .when(show_history_btn, |d| {
                d.child(icon_btn!(
                    "btn-history",
                    "H",
                    false,
                    cx.listener(|_this, _, _window, _cx| {
                        tracing::info!("History button clicked - using navigation_channel");
                        crate::ui_gpui::navigation_channel()
                            .request_navigate(crate::presentation::view_command::ViewId::History);
                    })
                ))
            })
            .child(icon_btn!(
                "btn-export-format",
                self.state.conversation_export_format.display_label(),
                false,
                cx.listener(|this, _, _window, _cx| {
                    let format = this.state.conversation_export_format.next();
                    this.emit(UserEvent::SelectConversationExportFormat { format });
                })
            ))
            .child(Self::render_copy_conversation_button(cx))
            .child(icon_btn!(
                "btn-save-conversation",
                "\u{2B07}",
                false,
                cx.listener(|this, _, _window, _cx| {
                    this.emit(UserEvent::SaveConversation);
                })
            ))
            .child(Self::render_popout_toggle_button(is_popout, cx))
            .child(icon_btn!(
                "btn-settings",
                "\u{2699}",
                false,
                cx.listener(|_this, _, _window, _cx| {
                    tracing::info!("Settings button clicked - using navigation_channel");
                    crate::ui_gpui::navigation_channel()
                        .request_navigate(crate::presentation::view_command::ViewId::Settings);
                })
            ))
            .child(Self::render_exit_button(cx))
    }

    fn render_popout_toggle_button(
        is_popout: bool,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("btn-popout")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .when(is_popout, |d| d.bg(Theme::bg_dark()))
            .when(!is_popout, |d| {
                d.bg(Theme::bg_darker()).hover(|s| s.bg(Theme::bg_dark()))
            })
            .child(if is_popout {
                crate::ui_gpui::components::window_icons::popin_icon(16.0)
                    .text_color(Theme::text_primary())
            } else {
                crate::ui_gpui::components::window_icons::popout_icon(16.0)
                    .text_color(Theme::text_primary())
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, _cx| {
                    this.emit(UserEvent::ToggleWindowMode);
                }),
            )
    }

    fn render_exit_button(cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("btn-exit")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .bg(Theme::bg_darker())
            .hover(|s| s.bg(Theme::danger()))
            .active(|s| s.bg(Theme::danger()))
            .text_size(px(Theme::font_size_body()))
            .text_color(Theme::text_primary())
            .child("\u{23FB}")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_this, _, _window, _cx| {
                    std::process::exit(0);
                }),
            )
    }

    pub(super) fn render_export_feedback_bar(&self) -> impl IntoElement {
        let is_error = self.state.export_feedback_is_error;
        let text_color = if is_error {
            Theme::error()
        } else {
            Theme::text_secondary()
        };

        let container = div()
            .id("chat-export-feedback")
            .h(px(24.0))
            .w_full()
            .bg(Theme::bg_darker())
            .px(px(12.0))
            .flex()
            .items_center();

        if let (Some(ref file_path), false) = (&self.state.export_feedback_path, is_error) {
            let path_for_open = file_path.clone();
            let dir_path = std::path::Path::new(file_path)
                .parent()
                .map_or_else(String::new, |p| p.display().to_string());
            let display_path = file_path.clone();

            container.child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .overflow_hidden()
                    .child(
                        div()
                            .id("export-open-file")
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::accent())
                            .cursor_pointer()
                            .hover(|s| s.text_color(Theme::accent_hover()))
                            .child(display_path)
                            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                let _ = std::process::Command::new("open")
                                    .arg(&path_for_open)
                                    .spawn();
                            }),
                    )
                    .child(
                        div()
                            .id("export-open-dir")
                            .flex_shrink_0()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::accent())
                            .cursor_pointer()
                            .hover(|s| s.text_color(Theme::accent_hover()))
                            .child("(dir)")
                            .on_mouse_down(MouseButton::Left, move |_, _, _| {
                                let _ = std::process::Command::new("open").arg(&dir_path).spawn();
                            }),
                    ),
            )
        } else {
            let message = self
                .state
                .export_feedback_message
                .clone()
                .unwrap_or_default();
            container.child(
                div()
                    .w_full()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(Theme::font_size_ui()))
                    .text_color(text_color)
                    .child(message),
            )
        }
    }

    /// Render the title bar with conversation dropdown and model label
    /// @plan PLAN-20250130-GPUIREDUX.P03
    pub(super) fn render_title_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let unviewed = crate::ui_gpui::error_log::ErrorLogStore::global().unviewed_count();
        let app_mode = cx
            .try_global::<MainPanelAppState>()
            .map(|s| s.app_mode)
            .unwrap_or_default();
        let is_popout = app_mode == AppMode::Popout;
        let sidebar_visible = self.state.sidebar_visible;

        div()
            .id("chat-title-bar")
            .flex_shrink_0()
            .h(px(32.0 * Theme::ui_scale()))
            .w_full()
            .bg(Theme::bg_darker())
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            // Sidebar toggle (popout mode only)
            .when(is_popout, |d| {
                d.child(
                    div()
                        .id("btn-sidebar-toggle")
                        .size(px(28.0))
                        .rounded(px(4.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .when(sidebar_visible, |d| d.bg(Theme::bg_dark()))
                        .when(!sidebar_visible, |d| {
                            d.bg(Theme::bg_darker()).hover(|s| s.bg(Theme::bg_dark()))
                        })
                        .child(
                            crate::ui_gpui::components::window_icons::sidebar_icon(16.0)
                                .text_color(Theme::text_primary()),
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _window, cx| {
                                this.toggle_sidebar(cx);
                            }),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    // Conversation selector: always shown in popup; shown in popout when sidebar hidden
                    .when(!is_popout || !sidebar_visible, |d| {
                        d.child(self.render_conversation_selector(cx))
                            .child(self.render_new_conversation_btn(cx))
                    })
                    .child(self.render_profile_selector(cx)),
            )
            .child(self.render_bug_icon_btn(unviewed, cx))
    }

    /// Bug icon button with unviewed error count badge.
    ///
    /// Hidden via opacity when there are no unviewed errors, preserving title-bar
    /// layout stability (equivalent to CSS `visibility: hidden`).
    #[allow(clippy::unused_self)] // cx.listener borrows the entity, not &self directly
    fn render_bug_icon_btn(
        &self,
        unviewed: usize,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let count_label = if unviewed > 99 {
            "99+".to_string()
        } else {
            unviewed.to_string()
        };

        div()
            .id("btn-error-log")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .relative()
            // Preserve layout when no errors (like CSS visibility:hidden)
            .opacity(if unviewed == 0 { 0.0 } else { 1.0 })
            .when(unviewed > 0, |d| {
                d.cursor_pointer()
                    .hover(|s| s.bg(Theme::bg_dark()))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            crate::ui_gpui::navigation_channel().request_navigate(
                                crate::presentation::view_command::ViewId::ErrorLog,
                            );
                        }),
                    )
            })
            .child(crate::ui_gpui::components::bug_icon::bug_icon(14.0).text_color(Theme::error()))
            // Count badge — top-right corner, styled like the YOLO badge
            .when(unviewed > 0, |d| {
                d.child(
                    div()
                        .id("error-log-badge")
                        .absolute()
                        .top(px(1.0))
                        .right(px(1.0))
                        .min_w(px(13.0))
                        .h(px(13.0))
                        .rounded(px(7.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .px(px(2.0))
                        .bg(Theme::error())
                        .text_size(px(Theme::font_size_small()))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::selection_fg())
                        .child(count_label),
                )
            })
    }

    /// Conversation title / rename field.
    fn render_conversation_selector(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        if self.state.conversation_title_editing {
            let input = self.state.conversation_title_input.clone();
            div()
                .id("conversation-title-input")
                .min_w(px(220.0))
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(4.0))
                .bg(Theme::bg_dark())
                .border_1()
                .border_color(Theme::accent())
                .child(
                    div()
                        .text_size(px(Theme::font_size_mono()))
                        .text_color(Theme::text_primary())
                        .child(if input.is_empty() {
                            "Enter conversation name".to_string()
                        } else {
                            input
                        }),
                )
        } else {
            let title = self.state.conversation_title.clone();
            let open = self.state.conversation_dropdown_open;
            div()
                .id("conversation-dropdown")
                .min_w(px(220.0))
                .px(px(8.0))
                .py(px(4.0))
                .rounded(px(4.0))
                .bg(Theme::bg_dark())
                .border_1()
                .border_color(if open {
                    Theme::accent()
                } else {
                    Theme::border()
                })
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(Theme::font_size_mono()))
                        .text_color(Theme::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(Theme::font_size_ui()))
                        .text_color(Theme::text_primary())
                        .child(if open { "\u{25B2}" } else { "\u{25BC}" }),
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _window, cx| {
                        this.toggle_conversation_dropdown(cx);
                    }),
                )
        }
    }

    /// "+" new conversation button.
    #[allow(clippy::unused_self)] // cx.listener borrows the entity, not &self directly
    fn render_new_conversation_btn(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("btn-new")
            .size(px(28.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|s| s.bg(Theme::bg_dark()))
            .text_size(px(Theme::font_size_body()))
            .text_color(Theme::text_primary())
            .child("+")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    tracing::info!("New conversation clicked - emitting UserEvent");
                    this.emit(UserEvent::NewConversation);
                    this.state.messages.clear();
                    this.state.input_text.clear();
                    this.state.cursor_position = 0;
                    this.state.streaming = StreamingState::Idle;
                    this.state.thinking_content = None;
                    this.state.active_conversation_id = None;
                    this.conversation_id = None;
                    this.state.conversation_title = "New Conversation".to_string();
                    this.state.conversation_dropdown_open = false;
                    this.state.conversation_title_editing = false;
                    this.state.conversation_title_input.clear();
                    this.state.profile_dropdown_open = false;
                    this.state.chat_autoscroll_enabled = true;
                    this.chat_scroll_handle.scroll_to_bottom();
                    cx.notify();
                }),
            )
    }

    /// Profile selector pill in the title bar.
    fn render_profile_selector(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let selected_profile = self.state.selected_profile().map_or_else(
            || "Select profile".to_string(),
            |profile| profile.name.clone(),
        );
        let open = self.state.profile_dropdown_open;

        div()
            .id("chat-profile-dropdown")
            .max_w(px(225.0 * Theme::ui_scale()))
            .min_w(px(100.0))
            .px(px(Theme::spacing_sm_scaled()))
            .py(px(Theme::spacing_xs_scaled()))
            .rounded(px(4.0))
            .bg(Theme::bg_dark())
            .border_1()
            .border_color(if open {
                Theme::accent()
            } else {
                Theme::border()
            })
            .cursor_pointer()
            .overflow_hidden()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis_start()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_primary())
                            .child(selected_profile),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(Theme::font_size_small()))
                            .text_color(Theme::text_secondary())
                            .child(if open { "\u{25B2}" } else { "\u{25BC}" }),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    this.toggle_profile_dropdown(cx);
                }),
            )
    }

    /// Render conversation dropdown overlay at root level so it can float over chat area.
    pub(super) fn render_conversation_dropdown(
        &self,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let active_id = self.state.active_conversation_id;
        let highlighted = self.state.conversation_dropdown_index;
        let sidebar_toggle_offset = Self::sidebar_toggle_offset(cx);

        div()
            .id("chat-conversation-dropdown-overlay")
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    if this.state.conversation_dropdown_open {
                        this.state.conversation_dropdown_open = false;
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .id("chat-conversation-dropdown-menu")
                    .absolute()
                    .top(px(
                        (TOP_BAR_HEIGHT + TITLE_BAR_HEIGHT + DROPDOWN_GAP) * Theme::ui_scale()
                    ))
                    .left(px(12.0 + sidebar_toggle_offset))
                    .min_w(px(220.0))
                    .max_w(px(520.0))
                    .max_h(px(220.0))
                    .overflow_y_scroll()
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, _| {}))
                    .children(
                        self.state
                            .conversations
                            .iter()
                            .enumerate()
                            .map(|(index, conv)| {
                                Self::render_conversation_item(
                                    index,
                                    conv,
                                    active_id,
                                    highlighted,
                                    cx,
                                )
                            }),
                    ),
            )
    }

    /// Single row inside the conversation dropdown.
    fn render_conversation_item(
        index: usize,
        conversation: &ConversationSummary,
        active_id: Option<uuid::Uuid>,
        highlighted_index: usize,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let conversation_id = conversation.id;
        let selected = active_id == Some(conversation_id);
        let highlighted = highlighted_index == index;
        let title = if conversation.title.trim().is_empty() {
            "Untitled Conversation".to_string()
        } else {
            conversation.title.clone()
        };
        let count_label = if conversation.message_count == 1 {
            "1 message".to_string()
        } else {
            format!("{} messages", conversation.message_count)
        };

        div()
            .id(SharedString::from(format!(
                "chat-conversation-item-{conversation_id}"
            )))
            .w_full()
            .px(px(Theme::spacing_sm_scaled()))
            .py(px(Theme::spacing_md_scaled() * 0.5))
            .cursor_pointer()
            .when(selected, |row| {
                row.bg(Theme::accent()).text_color(Theme::selection_fg())
            })
            .when(!selected && highlighted, |row| {
                row.bg(Theme::accent_hover())
                    .text_color(Theme::selection_fg())
            })
            .when(!selected && !highlighted, |row| {
                row.hover(|s| s.bg(Theme::bg_darker()))
                    .text_color(Theme::text_primary())
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(Theme::font_size_ui())).child(title))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_secondary())
                            .child(count_label),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.select_conversation_at_index(index, cx);
                    cx.stop_propagation();
                }),
            )
    }

    /// Render profile dropdown overlay at root level.
    pub(super) fn render_profile_dropdown(
        &self,
        window: &gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("chat-profile-dropdown-overlay")
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _window, cx| {
                    if this.state.profile_dropdown_open {
                        this.state.profile_dropdown_open = false;
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .id("chat-profile-dropdown-menu")
                    .absolute()
                    .top(px(
                        (TOP_BAR_HEIGHT + TITLE_BAR_HEIGHT + DROPDOWN_GAP) * Theme::ui_scale()
                    ))
                    .left(Self::compute_profile_dropdown_left(
                        window.bounds().size.width,
                        Self::sidebar_toggle_offset(cx),
                    ))
                    .w(px(260.0 * Theme::ui_scale()))
                    .max_w(px(300.0 * Theme::ui_scale()))
                    .max_h(px(220.0 * Theme::ui_scale()))
                    .overflow_y_scroll()
                    .bg(Theme::bg_dark())
                    .border_1()
                    .border_color(Theme::border())
                    .rounded(px(4.0))
                    .shadow_lg()
                    .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, _| {}))
                    .children(
                        self.state
                            .profiles
                            .iter()
                            .enumerate()
                            .map(|(index, profile)| {
                                Self::render_profile_item(
                                    index,
                                    profile,
                                    self.state.selected_profile_id,
                                    self.state.profile_dropdown_index,
                                    cx,
                                )
                            }),
                    ),
            )
    }

    /// Single row inside the profile dropdown.
    fn render_profile_item(
        index: usize,
        profile: &ProfileSummary,
        selected_id: Option<uuid::Uuid>,
        highlighted_index: usize,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let is_selected = selected_id == Some(profile.id);
        let is_highlighted = highlighted_index == index;
        let label = if profile.is_default {
            format!("{} (default)", profile.name)
        } else {
            profile.name.clone()
        };
        let model_id = profile.model_id.clone();

        div()
            .id(SharedString::from(format!(
                "chat-profile-item-{}",
                profile.id
            )))
            .w_full()
            .px(px(Theme::spacing_sm_scaled()))
            .py(px(Theme::spacing_md_scaled() * 0.5))
            .cursor_pointer()
            .when(is_selected, |row| {
                row.bg(Theme::accent()).text_color(Theme::selection_fg())
            })
            .when(!is_selected && is_highlighted, |row| {
                row.bg(Theme::accent_hover())
                    .text_color(Theme::selection_fg())
            })
            .when(!is_selected && !is_highlighted, |row| {
                row.hover(|s| s.bg(Theme::bg_darker()))
                    .text_color(Theme::text_primary())
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(Theme::font_size_ui())).child(label))
                    .child(
                        div()
                            .text_size(px(Theme::font_size_ui()))
                            .text_color(Theme::text_secondary())
                            .child(model_id),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _window, cx| {
                    this.select_profile_at_index(index, cx);
                }),
            )
    }

    /// Extra left offset when the sidebar toggle button is present in the
    /// title bar (popout mode with sidebar hidden): 28px button + 8px gap.
    fn sidebar_toggle_offset(cx: &gpui::Context<Self>) -> f32 {
        let is_popout = cx
            .try_global::<MainPanelAppState>()
            .is_some_and(|s| s.app_mode == AppMode::Popout);
        if is_popout {
            36.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn profile_dropdown_left_aligns_under_trigger_in_popup() {
        let left = ChatView::compute_profile_dropdown_left(px(760.0), 0.0);
        assert_eq!(left, px(276.0));
    }

    #[test]
    fn profile_dropdown_left_shifts_for_sidebar_toggle_in_popout() {
        let left = ChatView::compute_profile_dropdown_left(px(760.0), 36.0);
        assert_eq!(left, px(312.0));
    }

    #[test]
    fn profile_dropdown_left_clamps_to_right_bound_on_narrow_windows() {
        let clamped_right = ChatView::compute_profile_dropdown_left(px(520.0), 0.0);
        assert_eq!(clamped_right, px(248.0));
    }

    #[test]
    fn profile_dropdown_left_uses_minimum_margin_for_narrow_windows() {
        let left = ChatView::compute_profile_dropdown_left(px(200.0), 0.0);
        assert_eq!(left, px(12.0));
    }
}
