//! Tool approval command handler tests for `ChatView`.

#![allow(clippy::future_not_send)]

use crate::presentation::view_command::ViewCommand;
use crate::ui_gpui::views::chat_view::{ApprovalBubbleState, ChatState, ChatView};
use gpui::{AppContext, TestAppContext};

#[gpui::test]
async fn handle_tool_approval_request_adds_pending_bubble(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    request_id: "req-1".into(),
                    tool_name: "shell".into(),
                    tool_argument: "git push origin main".into(),
                },
                cx,
            );

            assert_eq!(view.state.approval_bubbles.len(), 1);
            let bubble = &view.state.approval_bubbles[0];
            assert_eq!(bubble.request_id, "req-1");
            assert_eq!(bubble.tool_name, "shell");
            assert_eq!(bubble.tool_argument, "git push origin main");
            assert_eq!(bubble.state, ApprovalBubbleState::Pending);
        });
    });
}

#[gpui::test]
async fn handle_tool_approval_resolved_approved(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    request_id: "req-2".into(),
                    tool_name: "write".into(),
                    tool_argument: "/tmp/greeting.txt".into(),
                },
                cx,
            );
            view.handle_command(
                ViewCommand::ToolApprovalResolved {
                    request_id: "req-2".into(),
                    approved: true,
                },
                cx,
            );

            assert_eq!(
                view.state.approval_bubbles[0].state,
                ApprovalBubbleState::Approved
            );
        });
    });
}

#[gpui::test]
async fn handle_tool_approval_resolved_denied(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    request_id: "req-3".into(),
                    tool_name: "shell".into(),
                    tool_argument: "rm -rf /".into(),
                },
                cx,
            );
            view.handle_command(
                ViewCommand::ToolApprovalResolved {
                    request_id: "req-3".into(),
                    approved: false,
                },
                cx,
            );

            assert_eq!(
                view.state.approval_bubbles[0].state,
                ApprovalBubbleState::Denied
            );
        });
    });
}

#[gpui::test]
async fn handle_tool_approval_resolved_ignores_unknown_request(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::ToolApprovalResolved {
                    request_id: "nonexistent".into(),
                    approved: true,
                },
                cx,
            );
            assert!(view.state.approval_bubbles.is_empty());
        });
    });
}

#[gpui::test]
async fn handle_yolo_mode_changed(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            assert!(!view.state.yolo_mode);

            view.handle_command(ViewCommand::YoloModeChanged { active: true }, cx);
            assert!(view.state.yolo_mode);

            view.handle_command(ViewCommand::YoloModeChanged { active: false }, cx);
            assert!(!view.state.yolo_mode);
        });
    });
}

#[gpui::test]
async fn conversation_cleared_also_clears_approval_bubbles(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    request_id: "req-clear".into(),
                    tool_name: "shell".into(),
                    tool_argument: "echo hello".into(),
                },
                cx,
            );
            assert_eq!(view.state.approval_bubbles.len(), 1);

            view.handle_command(ViewCommand::ConversationCleared, cx);
            assert!(view.state.approval_bubbles.is_empty());
        });
    });
}

#[gpui::test]
async fn multiple_approval_bubbles_tracked_independently(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    request_id: "a".into(),
                    tool_name: "shell".into(),
                    tool_argument: "git status".into(),
                },
                cx,
            );
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    request_id: "b".into(),
                    tool_name: "write".into(),
                    tool_argument: "/tmp/out.txt".into(),
                },
                cx,
            );

            assert_eq!(view.state.approval_bubbles.len(), 2);

            view.handle_command(
                ViewCommand::ToolApprovalResolved {
                    request_id: "a".into(),
                    approved: true,
                },
                cx,
            );

            assert_eq!(
                view.state.approval_bubbles[0].state,
                ApprovalBubbleState::Approved
            );
            assert_eq!(
                view.state.approval_bubbles[1].state,
                ApprovalBubbleState::Pending
            );
        });
    });
}
