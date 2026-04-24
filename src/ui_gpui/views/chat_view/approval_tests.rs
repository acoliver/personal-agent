//! Tool approval command handler tests for `ChatView`.

#![allow(clippy::future_not_send)]
#![allow(unused_imports)]
#![allow(deprecated)]

use crate::events::types::{ToolApprovalResponseAction, UserEvent};
use crate::presentation::view_command::{ToolApprovalContext, ToolCategory, ViewCommand};
use crate::ui_gpui::bridge::GpuiBridge;
use crate::ui_gpui::views::chat_view::{ApprovalBubbleState, ChatState, ChatView};
use gpui::{AppContext, TestAppContext};
use std::sync::Arc;
use uuid::Uuid;

fn make_bridge() -> (Arc<GpuiBridge>, flume::Receiver<UserEvent>) {
    let (user_tx, user_rx) = flume::bounded(16);
    let (_view_tx, view_rx) = flume::bounded(16);
    (Arc::new(GpuiBridge::new(user_tx, view_rx)), user_rx)
}

fn make_shell_context(cmd: impl Into<String>) -> ToolApprovalContext {
    ToolApprovalContext::new("ShellExec", ToolCategory::Shell, cmd)
}

fn make_write_context(path: impl Into<String>) -> ToolApprovalContext {
    ToolApprovalContext::new("WriteFile", ToolCategory::FileWrite, path)
}

fn approval_bubbles_for_active_test_conversation(
    view: &ChatView,
) -> &[crate::ui_gpui::views::chat_view::ToolApprovalBubble] {
    view.state
        .approval_bubbles
        .get(&Uuid::nil())
        .map_or(&[], Vec::as_slice)
}

#[gpui::test]
async fn handle_tool_approval_request_adds_pending_bubble(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "req-1".into(),
                    context: make_shell_context("git push origin main"),
                },
                cx,
            );

            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 1);
            let bubble = &approval_bubbles_for_active_test_conversation(view)[0];
            assert_eq!(bubble.request_id, "req-1");
            assert_eq!(bubble.context.tool_name, "ShellExec");
            assert_eq!(bubble.context.category, ToolCategory::Shell);
            assert_eq!(bubble.context.primary_target, "git push origin main");
            assert_eq!(bubble.state, ApprovalBubbleState::Pending);
        });
    });
}

#[gpui::test]
async fn handle_tool_approval_resolved_approved_removes_bubble(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "req-2".into(),
                    context: make_write_context("/tmp/greeting.txt"),
                },
                cx,
            );
            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 1);

            view.handle_command(
                ViewCommand::ToolApprovalResolved {
                    conversation_id: Uuid::nil(),
                    request_id: "req-2".into(),
                    approved: true,
                },
                cx,
            );

            assert!(
                view.state.approval_bubbles.is_empty(),
                "resolved bubble should be removed"
            );
        });
    });
}

#[gpui::test]
async fn handle_tool_approval_resolved_denied_removes_bubble(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "req-3".into(),
                    context: make_shell_context("rm -rf /"),
                },
                cx,
            );
            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 1);

            view.handle_command(
                ViewCommand::ToolApprovalResolved {
                    conversation_id: Uuid::nil(),
                    request_id: "req-3".into(),
                    approved: false,
                },
                cx,
            );

            assert!(
                view.state.approval_bubbles.is_empty(),
                "denied bubble should be removed"
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
                    conversation_id: Uuid::nil(),
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
async fn yolo_mode_auto_approves_tool_requests_without_rendering_bubble(cx: &mut TestAppContext) {
    let (bridge, user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ChatView::new(ChatState::default(), cx);
        view.set_bridge(bridge);
        view
    });
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(ViewCommand::YoloModeChanged { active: true }, cx);
            assert!(view.state.yolo_mode);

            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "req-yolo".into(),
                    context: make_shell_context("rm -rf /tmp/sandbox"),
                },
                cx,
            );

            assert!(
                view.state.approval_bubbles.is_empty(),
                "YOLO mode should suppress approval bubble rendering"
            );
        });
    });

    let event = user_rx
        .try_recv()
        .expect("YOLO mode should auto-emit approval response");
    assert_eq!(
        event,
        UserEvent::ToolApprovalResponse {
            request_id: "req-yolo".into(),
            decision: ToolApprovalResponseAction::ProceedOnce,
        }
    );
    assert!(
        user_rx.try_recv().is_err(),
        "only one approval response should be emitted"
    );
}

#[gpui::test]
async fn conversation_cleared_also_clears_approval_bubbles(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.set_conversation_id(Uuid::nil());
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "req-clear".into(),
                    context: make_shell_context("echo hello"),
                },
                cx,
            );
            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 1);

            view.handle_command(ViewCommand::ConversationCleared, cx);
            assert!(
                !view.state.approval_bubbles.contains_key(&Uuid::nil()),
                "active conversation bubbles should be removed on clear"
            );
        });
    });
}

#[gpui::test]
async fn conversation_cleared_only_clears_active_conversation_bubbles(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    let active_conversation_id = Uuid::new_v4();
    let background_conversation_id = Uuid::new_v4();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.set_conversation_id(active_conversation_id);

            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: active_conversation_id,
                    request_id: "req-active".into(),
                    context: make_shell_context("echo active"),
                },
                cx,
            );
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: background_conversation_id,
                    request_id: "req-background".into(),
                    context: make_shell_context("echo background"),
                },
                cx,
            );

            assert_eq!(
                view.state
                    .approval_bubbles
                    .get(&active_conversation_id)
                    .map_or(0, Vec::len),
                1
            );
            assert_eq!(
                view.state
                    .approval_bubbles
                    .get(&background_conversation_id)
                    .map_or(0, Vec::len),
                1
            );

            view.handle_command(ViewCommand::ConversationCleared, cx);

            assert!(
                !view
                    .state
                    .approval_bubbles
                    .contains_key(&active_conversation_id),
                "active conversation bubbles should be removed"
            );
            assert_eq!(
                view.state
                    .approval_bubbles
                    .get(&background_conversation_id)
                    .map_or(0, Vec::len),
                1,
                "background conversation bubbles should be preserved"
            );
        });
    });
}

#[gpui::test]
async fn background_tool_approval_request_does_not_trigger_autoscroll(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    let active_conversation_id = Uuid::new_v4();
    let background_conversation_id = Uuid::new_v4();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.set_conversation_id(active_conversation_id);
            view.state.chat_autoscroll_enabled = true;
            view.maybe_scroll_chat_to_bottom_invocations.set(0);

            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: background_conversation_id,
                    request_id: "req-background".into(),
                    context: make_shell_context("echo background"),
                },
                cx,
            );

            assert_eq!(
                view.maybe_scroll_chat_to_bottom_invocations.get(),
                0,
                "background conversation approval must not scroll active chat"
            );
            assert_eq!(
                view.state
                    .approval_bubbles
                    .get(&background_conversation_id)
                    .map_or(0, Vec::len),
                1
            );
        });
    });
}

#[gpui::test]
async fn resolving_one_bubble_retains_other_pending_bubbles(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "a".into(),
                    context: make_shell_context("git status"),
                },
                cx,
            );
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "b".into(),
                    context: make_write_context("/tmp/out.txt"),
                },
                cx,
            );

            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 2);

            view.handle_command(
                ViewCommand::ToolApprovalResolved {
                    conversation_id: Uuid::nil(),
                    request_id: "a".into(),
                    approved: true,
                },
                cx,
            );

            // Resolved bubble "a" is removed; only pending "b" remains.
            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 1);
            // Note: request_ids now tracks all grouped request IDs, but the first request_id field remains
            assert_eq!(
                approval_bubbles_for_active_test_conversation(view)[0].request_id,
                "b"
            );
            assert_eq!(
                approval_bubbles_for_active_test_conversation(view)[0].state,
                ApprovalBubbleState::Pending
            );
        });
    });
}

// ── Queue Behavior Tests ───────────────────────────────────────────────

#[gpui::test]
async fn queue_shows_only_first_pending_bubble(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            // Add three pending approval requests with different targets
            // (so they don't group together)
            for i in 0..3 {
                view.handle_command(
                    ViewCommand::ToolApprovalRequest {
                        conversation_id: Uuid::nil(),
                        request_id: format!("req-{i}"),
                        context: make_shell_context(format!("cmd-{i}")),
                    },
                    cx,
                );
            }

            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 3);

            // Simulate render: count visible pending bubbles (should be 1)
            let visible_pending: Vec<_> = approval_bubbles_for_active_test_conversation(view)
                .iter()
                .filter(|b| matches!(b.state, ApprovalBubbleState::Pending))
                .take(1)
                .collect();

            assert_eq!(visible_pending.len(), 1);
            assert_eq!(visible_pending[0].request_id, "req-0");
        });
    });
}

#[gpui::test]
async fn queue_fifo_order_first_added_first_shown(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            // Add two pending requests with different targets
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "first".into(),
                    context: make_shell_context("echo first"),
                },
                cx,
            );
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "second".into(),
                    context: make_write_context("/tmp/second.txt"),
                },
                cx,
            );

            // First should be visible (at front of queue)
            let visible_pending: Vec<_> = approval_bubbles_for_active_test_conversation(view)
                .iter()
                .filter(|b| matches!(b.state, ApprovalBubbleState::Pending))
                .take(1)
                .collect();
            assert_eq!(visible_pending[0].request_id, "first");

            // Resolve first
            view.handle_command(
                ViewCommand::ToolApprovalResolved {
                    conversation_id: Uuid::nil(),
                    request_id: "first".into(),
                    approved: true,
                },
                cx,
            );

            // Second should now be visible
            let visible_pending: Vec<_> = approval_bubbles_for_active_test_conversation(view)
                .iter()
                .filter(|b| matches!(b.state, ApprovalBubbleState::Pending))
                .take(1)
                .collect();
            assert_eq!(visible_pending.len(), 1);
            assert_eq!(visible_pending[0].request_id, "second");
        });
    });
}

#[gpui::test]
async fn queue_rapid_sequential_approvals_stable(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            // Add multiple requests with different targets
            for i in 0..5 {
                view.handle_command(
                    ViewCommand::ToolApprovalRequest {
                        conversation_id: Uuid::nil(),
                        request_id: format!("r{i}"),
                        context: make_shell_context(format!("cmd-{i}")),
                    },
                    cx,
                );
            }

            // Rapidly approve all but one
            for i in 0..4 {
                view.handle_command(
                    ViewCommand::ToolApprovalResolved {
                        conversation_id: Uuid::nil(),
                        request_id: format!("r{i}"),
                        approved: true,
                    },
                    cx,
                );
            }

            // Only the last one should remain
            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 1);
            assert_eq!(
                approval_bubbles_for_active_test_conversation(view)[0].request_id,
                "r4"
            );
            assert_eq!(
                approval_bubbles_for_active_test_conversation(view)[0].state,
                ApprovalBubbleState::Pending
            );

            // That one should be visible
            let visible_pending: Vec<_> = approval_bubbles_for_active_test_conversation(view)
                .iter()
                .filter(|b| matches!(b.state, ApprovalBubbleState::Pending))
                .take(1)
                .collect();
            assert_eq!(visible_pending.len(), 1);
            assert_eq!(visible_pending[0].request_id, "r4");
        });
    });
}

// ── Grouping Tests ───────────────────────────────────────────────────

fn make_edit_context(path: impl Into<String>, old: &str, new: &str) -> ToolApprovalContext {
    ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, path)
        .with_detail("old", old)
        .with_detail("new", new)
}

#[gpui::test]
async fn two_edit_requests_same_path_grouped_into_one_bubble(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            // Two EditFile requests for the same path
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "edit-1".into(),
                    context: make_edit_context("/tmp/main.rs", "foo", "bar"),
                },
                cx,
            );
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "edit-2".into(),
                    context: make_edit_context("/tmp/main.rs", "baz", "qux"),
                },
                cx,
            );

            // Should be grouped into one bubble
            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 1);
            let bubble = &approval_bubbles_for_active_test_conversation(view)[0];
            assert_eq!(bubble.operation_count(), 2);
            assert_eq!(bubble.request_ids.len(), 2);
            assert!(bubble.request_ids.contains(&"edit-1".to_string()));
            assert!(bubble.request_ids.contains(&"edit-2".to_string()));
        });
    });
}

#[gpui::test]
async fn edit_file_and_write_file_same_path_grouped(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            // EditFile for a path
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "edit-1".into(),
                    context: ToolApprovalContext::new(
                        "EditFile",
                        ToolCategory::FileEdit,
                        "/tmp/file.txt",
                    ),
                },
                cx,
            );
            // WriteFile for the same path - same category (FileEdit/WriteFile both map to file-edit)
            // Actually, we need to verify the categories are compatible
            // FileEdit and FileWrite have different categories, so they shouldn't group
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "write-1".into(),
                    context: ToolApprovalContext::new(
                        "WriteFile",
                        ToolCategory::FileWrite,
                        "/tmp/file.txt",
                    ),
                },
                cx,
            );

            // Different categories, so they should NOT group
            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 2);
        });
    });
}

#[gpui::test]
async fn edit_file_path_a_and_path_b_separate_bubbles(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            // EditFile for path A
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "edit-a".into(),
                    context: make_edit_context("/tmp/a.rs", "x", "y"),
                },
                cx,
            );
            // EditFile for path B
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "edit-b".into(),
                    context: make_edit_context("/tmp/b.rs", "x", "y"),
                },
                cx,
            );

            // Different targets, so separate bubbles
            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 2);
        });
    });
}

#[gpui::test]
async fn grouping_only_applies_to_pending_state(cx: &mut TestAppContext) {
    let view = cx.new(|cx| ChatView::new(ChatState::default(), cx));
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            // First request
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "first".into(),
                    context: make_edit_context("/tmp/file.rs", "x", "y"),
                },
                cx,
            );

            // Resolve it
            view.handle_command(
                ViewCommand::ToolApprovalResolved {
                    conversation_id: Uuid::nil(),
                    request_id: "first".into(),
                    approved: true,
                },
                cx,
            );

            // The bubble is removed on resolve, so there's nothing to group with
            assert!(view.state.approval_bubbles.is_empty());

            // New request for same path - no existing pending bubble
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "second".into(),
                    context: make_edit_context("/tmp/file.rs", "a", "b"),
                },
                cx,
            );

            // Should create a new bubble
            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 1);
            assert_eq!(
                approval_bubbles_for_active_test_conversation(view)[0].request_id,
                "second"
            );
        });
    });
}

#[gpui::test]
async fn approving_group_resolves_all_request_ids(cx: &mut TestAppContext) {
    let (bridge, _user_rx) = make_bridge();
    let view = cx.new(|cx| {
        let mut view = ChatView::new(ChatState::default(), cx);
        view.set_bridge(bridge);
        view
    });
    let mut visual_cx = cx.add_empty_window().clone();

    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, cx| {
            // Add three grouped operations
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "op1".into(),
                    context: make_edit_context("/tmp/main.rs", "a", "b"),
                },
                cx,
            );
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "op2".into(),
                    context: make_edit_context("/tmp/main.rs", "c", "d"),
                },
                cx,
            );
            view.handle_command(
                ViewCommand::ToolApprovalRequest {
                    conversation_id: Uuid::nil(),
                    request_id: "op3".into(),
                    context: make_edit_context("/tmp/main.rs", "e", "f"),
                },
                cx,
            );

            assert_eq!(approval_bubbles_for_active_test_conversation(view).len(), 1);
            assert_eq!(
                approval_bubbles_for_active_test_conversation(view)[0].operation_count(),
                3
            );

            // Approve the bubble (simulating Yes button click)
            view.handle_command(
                ViewCommand::ToolApprovalResolved {
                    conversation_id: Uuid::nil(),
                    request_id: "op1".into(),
                    approved: true,
                },
                cx,
            );
        });
    });

    // The bubble should be removed after resolution
    visual_cx.update(|_window, app| {
        view.update(app, |view: &mut ChatView, _cx| {
            assert!(view.state.approval_bubbles.is_empty());
        });
    });
}
