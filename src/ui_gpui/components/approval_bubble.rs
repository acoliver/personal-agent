//! Inline tool approval bubble component.
//!
//! Renders a compact single-line approval request in the conversation stream:
//!
//!     [tool icon] shell: git push origin main
//!     [Yes] [Session] [Always] [No]

use gpui::{div, prelude::*, px, IntoElement, MouseButton, SharedString};

use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::views::chat_view::ApprovalBubbleState;

/// Compact inline approval bubble for a single tool call.
pub struct ApprovalBubble {
    request_id: String,
    tool_name: String,
    tool_argument: String,
    state: ApprovalBubbleState,
    on_yes: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    on_session: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    on_always: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    on_no: Option<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl ApprovalBubble {
    pub fn new(
        request_id: impl Into<String>,
        tool_name: impl Into<String>,
        tool_argument: impl Into<String>,
        state: ApprovalBubbleState,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            tool_name: tool_name.into(),
            tool_argument: tool_argument.into(),
            state,
            on_yes: None,
            on_session: None,
            on_always: None,
            on_no: None,
        }
    }

    #[must_use]
    pub fn on_yes(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_yes = Some(Box::new(f));
        self
    }

    #[must_use]
    pub fn on_session(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_session = Some(Box::new(f));
        self
    }

    #[must_use]
    pub fn on_always(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_always = Some(Box::new(f));
        self
    }

    #[must_use]
    pub fn on_no(mut self, f: impl Fn() + Send + Sync + 'static) -> Self {
        self.on_no = Some(Box::new(f));
        self
    }

    fn render_action_button(
        id: &str,
        label: &str,
        bg: gpui::Hsla,
        fg: gpui::Hsla,
        handler: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    ) -> gpui::Stateful<gpui::Div> {
        let btn = div()
            .id(SharedString::from(id.to_string()))
            .px(px(Theme::SPACING_SM))
            .py(px(3.0))
            .rounded(px(Theme::RADIUS_SM))
            .bg(bg)
            .text_size(px(Theme::FONT_SIZE_XS))
            .text_color(fg)
            .cursor_pointer()
            .hover(|s| s.opacity(0.8))
            .child(label.to_string());

        if let Some(callback) = handler {
            btn.on_mouse_down(MouseButton::Left, move |_, _, _| {
                (callback)();
            })
        } else {
            btn
        }
    }
}

impl IntoElement for ApprovalBubble {
    type Element = gpui::Stateful<gpui::Div>;

    fn into_element(self) -> Self::Element {
        let bubble_id = SharedString::from(format!("approval-bubble-{}", self.request_id));

        let tool_label = format!("\u{1F527} {}: {}", self.tool_name, self.tool_argument);

        let mut container = div()
            .id(bubble_id)
            .w_full()
            .px(px(Theme::SPACING_MD))
            .py(px(Theme::SPACING_SM))
            .rounded(px(Theme::RADIUS_LG))
            .border_1()
            .border_color(Theme::border())
            .bg(Theme::bg_darker())
            .flex()
            .flex_col()
            .gap(px(Theme::SPACING_SM));

        // Tool label row
        container = container.child(
            div()
                .w_full()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(Theme::FONT_SIZE_MD))
                .text_color(Theme::text_primary())
                .child(tool_label),
        );

        match self.state {
            ApprovalBubbleState::Pending => {
                let yes_id = format!("approval-yes-{}", self.request_id);
                let session_id = format!("approval-session-{}", self.request_id);
                let always_id = format!("approval-always-{}", self.request_id);
                let no_id = format!("approval-no-{}", self.request_id);

                container = container.child(
                    div()
                        .flex()
                        .gap(px(Theme::SPACING_SM))
                        .child(Self::render_action_button(
                            &yes_id,
                            "Yes",
                            Theme::accent(),
                            Theme::selection_fg(),
                            self.on_yes,
                        ))
                        .child(Self::render_action_button(
                            &session_id,
                            "Session",
                            Theme::bg_dark(),
                            Theme::text_primary(),
                            self.on_session,
                        ))
                        .child(Self::render_action_button(
                            &always_id,
                            "Always",
                            Theme::bg_dark(),
                            Theme::text_primary(),
                            self.on_always,
                        ))
                        .child(Self::render_action_button(
                            &no_id,
                            "No",
                            Theme::error(),
                            Theme::selection_fg(),
                            self.on_no,
                        )),
                );
            }
            ApprovalBubbleState::Approved => {
                container = container.child(
                    div()
                        .text_size(px(Theme::FONT_SIZE_XS))
                        .text_color(Theme::success())
                        .child("\u{2713} Approved"),
                );
            }
            ApprovalBubbleState::Denied => {
                container = container.child(
                    div()
                        .text_size(px(Theme::FONT_SIZE_XS))
                        .text_color(Theme::error())
                        .child("\u{2717} Denied"),
                );
            }
        }

        container
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_bubble_new_sets_fields() {
        let bubble =
            ApprovalBubble::new("req-1", "shell", "git push", ApprovalBubbleState::Pending);
        assert_eq!(bubble.request_id, "req-1");
        assert_eq!(bubble.tool_name, "shell");
        assert_eq!(bubble.tool_argument, "git push");
        assert!(matches!(bubble.state, ApprovalBubbleState::Pending));
    }

    #[test]
    fn approval_bubble_callbacks_are_settable() {
        let bubble =
            ApprovalBubble::new("req-2", "write", "/tmp/f.txt", ApprovalBubbleState::Pending)
                .on_yes(|| {})
                .on_session(|| {})
                .on_always(|| {})
                .on_no(|| {});
        assert!(bubble.on_yes.is_some());
        assert!(bubble.on_session.is_some());
        assert!(bubble.on_always.is_some());
        assert!(bubble.on_no.is_some());
    }

    #[test]
    fn approval_bubble_approved_state() {
        let bubble = ApprovalBubble::new("req-3", "shell", "ls", ApprovalBubbleState::Approved);
        assert!(matches!(bubble.state, ApprovalBubbleState::Approved));
    }

    #[test]
    fn approval_bubble_denied_state() {
        let bubble = ApprovalBubble::new("req-4", "shell", "rm -rf /", ApprovalBubbleState::Denied);
        assert!(matches!(bubble.state, ApprovalBubbleState::Denied));
    }
}
