//! Inline tool approval bubble component.
//!
//! Renders approval requests in the conversation stream with support for
//! grouping related operations (same category + same primary target):
//!
//!     [tool icon] EditFile: /tmp/main.rs
//!     (3 operations) [expand/collapse toggle]
//!     [Yes] [Session] [Always] [No]
//!
//! When expanded, shows the list of grouped operations.

use gpui::{div, prelude::*, px, IntoElement, MouseButton, SharedString};

use crate::presentation::view_command::ToolApprovalContext;
use crate::ui_gpui::theme::Theme;
use crate::ui_gpui::views::chat_view::{ApprovalBubbleState, GroupedOperation};

/// Inline approval bubble for tool calls, with support for grouped operations.
pub struct ApprovalBubble {
    request_id: String,
    context: ToolApprovalContext,
    state: ApprovalBubbleState,
    operation_count: usize,
    expanded: bool,
    grouped_operations: Vec<GroupedOperation>,
    on_yes: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    on_session: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    on_always: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    on_no: Option<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl ApprovalBubble {
    pub fn new(
        request_id: impl Into<String>,
        context: ToolApprovalContext,
        state: ApprovalBubbleState,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            context,
            state,
            operation_count: 1,
            expanded: false,
            grouped_operations: Vec::new(),
            on_yes: None,
            on_session: None,
            on_always: None,
            on_no: None,
        }
    }

    #[must_use]
    pub const fn operation_count(mut self, count: usize) -> Self {
        self.operation_count = count;
        self
    }

    #[must_use]
    pub const fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    #[must_use]
    pub fn grouped_operations(mut self, ops: Vec<GroupedOperation>) -> Self {
        self.grouped_operations = ops;
        self
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

    const fn icon_for_category(
        category: crate::presentation::view_command::ToolCategory,
    ) -> &'static str {
        use crate::presentation::view_command::ToolCategory;
        match category {
            ToolCategory::FileEdit => "\u{270F}",   // Pencil
            ToolCategory::FileWrite => "\u{1F4DD}", // Memo
            ToolCategory::FileRead => "\u{1F4C4}",  // Page
            ToolCategory::Search => "\u{1F50D}",    // Magnifying glass
            ToolCategory::Shell => "\u{1F527}",     // Wrench
            ToolCategory::Mcp => "\u{1F9F0}",       // Toolbox
        }
    }

    fn render_action_button(
        id: &str,
        label: &str,
        button: gpui::Div,
        handler: Option<Box<dyn Fn() + Send + Sync + 'static>>,
    ) -> gpui::Stateful<gpui::Div> {
        let btn = button
            .id(SharedString::from(id.to_string()))
            .px(px(Theme::SPACING_SM))
            .py(px(3.0))
            .rounded(px(Theme::RADIUS_SM))
            .text_size(px(Theme::font_size_ui()))
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

    #[allow(clippy::too_many_lines)]
    fn into_element(self) -> Self::Element {
        let bubble_id = SharedString::from(format!("approval-bubble-{}", self.request_id));
        let icon = Self::icon_for_category(self.context.category);

        let mut container = Theme::assistant_bubble(
            div()
                .id(bubble_id)
                .w_full()
                .px(px(Theme::SPACING_MD))
                .py(px(Theme::SPACING_SM))
                .rounded(px(Theme::RADIUS_LG))
                .border_1()
                .flex()
                .flex_col()
                .gap(px(Theme::SPACING_SM)),
        );

        // Header row: icon + tool_name (+ server_name for MCP)
        let header_text = if let Some(ref server) = self.context.server_name {
            format!("{} {} (via {}) ", icon, self.context.tool_name, server)
        } else {
            format!("{} {} ", icon, self.context.tool_name)
        };

        container = container.child(
            div()
                .w_full()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(Theme::font_size_ui()))
                .child(header_text),
        );

        // Primary target row (prominent)
        container = container.child(
            div()
                .w_full()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(Theme::font_size_mono()))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(self.context.primary_target.clone()),
        );

        // Details section (if any) for primary operation
        if !self.context.details.is_empty() {
            let details_text = self
                .context
                .details
                .iter()
                .map(|(k, v)| format!("{k}: {v}"))
                .collect::<Vec<_>>()
                .join(" | ");

            container = container.child(
                div()
                    .w_full()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(Theme::font_size_small()))
                    .text_color(Theme::text_muted())
                    .child(details_text),
            );
        }

        // Grouped operations badge and expand/collapse (if more than 1)
        if self.operation_count > 1 {
            let badge_text = format!("({} operations)", self.operation_count);
            let expand_text = if self.expanded {
                "\u{25BC} Hide details"
            } else {
                "\u{25B6} Show details"
            };

            container = container.child(
                div()
                    .flex()
                    .gap(px(Theme::SPACING_SM))
                    .child(
                        div()
                            .px(px(Theme::SPACING_SM))
                            .py(px(2.0))
                            .rounded(px(Theme::RADIUS_SM))
                            .bg(Theme::bg_dark())
                            .text_size(px(Theme::font_size_small()))
                            .text_color(Theme::text_secondary())
                            .child(badge_text),
                    )
                    .child(
                        div()
                            .text_size(px(Theme::font_size_small()))
                            .text_color(Theme::text_muted())
                            .child(expand_text),
                    ),
            );

            // Expanded grouped operations list
            if self.expanded && !self.grouped_operations.is_empty() {
                let mut ops_list = div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .pl(px(Theme::SPACING_MD))
                    .border_l_1()
                    .border_color(Theme::bg_dark());

                for op in &self.grouped_operations {
                    let op_text = if op.details.is_empty() {
                        format!(
                            "\u{2022} Operation {}",
                            &op.request_id[..8.min(op.request_id.len())]
                        )
                    } else {
                        let details = op
                            .details
                            .iter()
                            .map(|(k, v)| format!("{k}: {v}"))
                            .collect::<Vec<_>>()
                            .join(" | ");
                        format!("\u{2022} {details}")
                    };

                    ops_list = ops_list.child(
                        div()
                            .text_size(px(Theme::font_size_small()))
                            .text_color(Theme::text_muted())
                            .child(op_text),
                    );
                }

                container = container.child(ops_list);
            }
        }

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
                            Theme::button_primary(div()),
                            self.on_yes,
                        ))
                        .child(Self::render_action_button(
                            &session_id,
                            "Session",
                            Theme::button_secondary(div()),
                            self.on_session,
                        ))
                        .child(Self::render_action_button(
                            &always_id,
                            "Always",
                            Theme::button_secondary(div()),
                            self.on_always,
                        ))
                        .child(Self::render_action_button(
                            &no_id,
                            "No",
                            Theme::button_danger(div()),
                            self.on_no,
                        )),
                );
            }
            ApprovalBubbleState::Approved => {
                container = container.child(
                    div()
                        .text_size(px(Theme::font_size_ui()))
                        .text_color(Theme::success())
                        .child("\u{2713} Approved"),
                );
            }
            ApprovalBubbleState::Denied => {
                container = container.child(
                    div()
                        .text_size(px(Theme::font_size_ui()))
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
    use crate::presentation::view_command::{ToolApprovalContext, ToolCategory};

    #[test]
    fn approval_bubble_new_sets_fields() {
        let context = ToolApprovalContext::new("shell", ToolCategory::Shell, "git push");
        let bubble = ApprovalBubble::new("req-1", context, ApprovalBubbleState::Pending);
        assert_eq!(bubble.request_id, "req-1");
        assert_eq!(bubble.context.tool_name, "shell");
        assert_eq!(bubble.context.primary_target, "git push");
        assert!(matches!(bubble.state, ApprovalBubbleState::Pending));
    }

    #[test]
    fn approval_bubble_callbacks_are_settable() {
        let context = ToolApprovalContext::new("write", ToolCategory::FileWrite, "/tmp/f.txt");
        let bubble = ApprovalBubble::new("req-2", context, ApprovalBubbleState::Pending)
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
        let context = ToolApprovalContext::new("shell", ToolCategory::Shell, "ls");
        let bubble = ApprovalBubble::new("req-3", context, ApprovalBubbleState::Approved);
        assert!(matches!(bubble.state, ApprovalBubbleState::Approved));
    }

    #[test]
    fn approval_bubble_denied_state() {
        let context = ToolApprovalContext::new("shell", ToolCategory::Shell, "rm -rf /");
        let bubble = ApprovalBubble::new("req-4", context, ApprovalBubbleState::Denied);
        assert!(matches!(bubble.state, ApprovalBubbleState::Denied));
    }

    #[test]
    fn approval_bubble_with_server_name() {
        let context = ToolApprovalContext::new("mcp-tool", ToolCategory::Mcp, "arg-value")
            .with_server_name("test-server");
        let bubble = ApprovalBubble::new("req-5", context, ApprovalBubbleState::Pending);
        assert_eq!(bubble.context.server_name, Some("test-server".to_string()));
    }

    #[test]
    fn approval_bubble_with_details() {
        let context = ToolApprovalContext::new("edit", ToolCategory::FileEdit, "/tmp/f.txt")
            .with_detail("line_range", "10-20")
            .with_detail("encoding", "utf-8");
        let bubble = ApprovalBubble::new("req-6", context, ApprovalBubbleState::Pending);
        assert_eq!(bubble.context.details.len(), 2);
    }
}
