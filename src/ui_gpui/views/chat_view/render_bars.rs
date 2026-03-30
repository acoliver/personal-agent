//! Chat view bar and dropdown render subtrees.
//!
//! Contains `render_top_bar`, `render_title_bar`, `render_conversation_dropdown`,
//! and `render_profile_dropdown`. These are navigation-chrome render methods that
//! sit above the chat area.
//!
//! @plan PLAN-20260325-ISSUE11B.P02

use super::state::StreamingState;
use super::ChatView;
use crate::events::types::UserEvent;
use crate::presentation::view_command::{ConversationSummary, ProfileSummary};
use crate::ui_gpui::theme::Theme;
use gpui::{div, prelude::*, px, FontWeight, MouseButton, SharedString};

impl ChatView {
    /// Render the top bar with icon, title, and toolbar buttons
    /// @plan PLAN-20250130-GPUIREDUX.P04
    pub(super) fn render_top_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("chat-top-bar")
            .h(px(44.0))
            .w_full()
            .bg(Theme::bg_darker())
            .border_b_1()
            .border_color(Theme::border())
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div().flex().items_center().child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(Theme::text_primary())
                        .child("PersonalAgent"),
                ),
            )
            .child(self.render_toolbar_buttons(cx))
    }

    /// Right-side toolbar: [T][R][H][Settings][Exit]
    #[allow(clippy::too_many_lines)]
    fn render_toolbar_buttons(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let show_thinking = self.state.show_thinking;

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
                    .text_size(px(14.0))
                    .text_color(Theme::text_primary())
                    .child($label)
                    .on_mouse_down(MouseButton::Left, $handler)
            };
        }

        div()
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
            .child(icon_btn!(
                "btn-rename",
                "R",
                false,
                cx.listener(|this, _, _window, cx| {
                    this.start_rename_conversation(cx);
                })
            ))
            .child(icon_btn!(
                "btn-history",
                "H",
                false,
                cx.listener(|_this, _, _window, _cx| {
                    println!(">>> HISTORY BUTTON CLICKED - using navigation_channel <<<");
                    crate::ui_gpui::navigation_channel()
                        .request_navigate(crate::presentation::view_command::ViewId::History);
                })
            ))
            .child(icon_btn!(
                "btn-export-format",
                self.state.conversation_export_format.display_label(),
                false,
                cx.listener(|this, _, _window, _cx| {
                    let format = this.state.conversation_export_format.next();
                    this.emit(UserEvent::SelectConversationExportFormat { format });
                })
            ))
            .child(icon_btn!(
                "btn-save-conversation",
                "\u{2B07}",
                false,
                cx.listener(|this, _, _window, _cx| {
                    this.emit(UserEvent::SaveConversation);
                })
            ))
            .child(icon_btn!(
                "btn-settings",
                "\u{2699}",
                false,
                cx.listener(|_this, _, _window, _cx| {
                    println!(">>> SETTINGS BUTTON CLICKED - using navigation_channel <<<");
                    crate::ui_gpui::navigation_channel()
                        .request_navigate(crate::presentation::view_command::ViewId::Settings);
                })
            ))
            .child(
                div()
                    .id("btn-exit")
                    .size(px(28.0))
                    .rounded(px(4.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(Theme::bg_darker())
                    .hover(|s| s.bg(gpui::rgb(0x008B_0000)))
                    .active(|s| s.bg(gpui::rgb(0x005C_0000)))
                    .text_size(px(14.0))
                    .text_color(Theme::text_primary())
                    .child("\u{23FB}")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_this, _, _window, _cx| {
                            std::process::exit(0);
                        }),
                    ),
            )
    }

    pub(super) fn render_export_feedback_bar(&self) -> impl IntoElement {
        let message = self
            .state
            .export_feedback_message
            .clone()
            .unwrap_or_default();
        let text_color = if self.state.export_feedback_is_error {
            Theme::error()
        } else {
            Theme::text_secondary()
        };

        div()
            .id("chat-export-feedback")
            .h(px(24.0))
            .w_full()
            .bg(Theme::bg_darker())
            .px(px(12.0))
            .flex()
            .items_center()
            .child(
                div()
                    .w_full()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(11.0))
                    .text_color(text_color)
                    .child(message),
            )
    }

    /// Render the title bar with conversation dropdown and model label
    /// @plan PLAN-20250130-GPUIREDUX.P03
    pub(super) fn render_title_bar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .id("chat-title-bar")
            .h(px(32.0))
            .w_full()
            .bg(Theme::bg_darker())
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(self.render_conversation_selector(cx))
                    .child(self.render_new_conversation_btn(cx))
                    .child(self.render_profile_selector(cx)),
            )
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
                        .text_size(px(13.0))
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
                        .text_size(px(13.0))
                        .text_color(Theme::text_primary())
                        .child(title),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .text_size(px(10.0))
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
            .text_size(px(14.0))
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
            .max_w(px(225.0))
            .min_w(px(100.0))
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
                            .text_size(px(11.0))
                            .text_color(Theme::text_primary())
                            .child(selected_profile),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_size(px(9.0))
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
                    .top(px(74.0))
                    .left(px(12.0))
                    .min_w(px(320.0))
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
            .px(px(8.0))
            .py(px(6.0))
            .cursor_pointer()
            .when(selected, |row| {
                row.bg(Theme::accent()).text_color(gpui::white())
            })
            .when(!selected && highlighted, |row| {
                row.bg(Theme::accent_hover()).text_color(gpui::white())
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
                    .child(div().text_size(px(11.0)).child(title))
                    .child(
                        div()
                            .text_size(px(10.0))
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
                    .top(px(74.0))
                    .left(Self::compute_profile_dropdown_left(
                        window.bounds().size.width,
                    ))
                    .w(px(260.0))
                    .max_w(px(300.0))
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
            .px(px(8.0))
            .py(px(6.0))
            .cursor_pointer()
            .when(is_selected, |row| {
                row.bg(Theme::accent()).text_color(gpui::white())
            })
            .when(!is_selected && is_highlighted, |row| {
                row.bg(Theme::accent_hover()).text_color(gpui::white())
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
                    .child(div().text_size(px(11.0)).child(label))
                    .child(
                        div()
                            .text_size(px(10.0))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::px;

    #[test]
    fn profile_dropdown_left_aligns_under_trigger_left_edge() {
        let left = ChatView::compute_profile_dropdown_left(px(760.0));
        assert_eq!(left, px(276.0));
    }

    #[test]
    fn profile_dropdown_left_clamps_to_right_bound_on_narrow_windows() {
        let clamped_right = ChatView::compute_profile_dropdown_left(px(520.0));
        assert_eq!(clamped_right, px(248.0));
    }

    #[test]
    fn profile_dropdown_left_uses_minimum_margin_for_narrow_windows() {
        let left = ChatView::compute_profile_dropdown_left(px(200.0));
        assert_eq!(left, px(12.0));
    }
}
