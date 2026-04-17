//! Tool approval routing tests for `MainPanel`.

use crate::presentation::view_command::{ToolApprovalContext, ToolCategory, ViewCommand};
use crate::ui_gpui::views::main_panel::{
    tests::{assert_route_count, build_app_state},
    MainPanel,
};
use gpui::{AppContext, TestAppContext};
use uuid::Uuid;

#[gpui::test]
async fn route_tool_approval_policy_updated_increments_counter(cx: &mut TestAppContext) {
    let _ = cx;
    assert_route_count(
        ViewCommand::ToolApprovalPolicyUpdated {
            yolo_mode: true,
            auto_approve_reads: false,
            skills_auto_approve: false,
            mcp_approval_mode: crate::agent::McpApprovalMode::PerTool,
            persistent_allowlist: vec!["git".to_string()],
            persistent_denylist: vec!["rm".to_string()],
        },
        1,
        |targets| targets.tool_approval_policy_count,
    );
}

#[gpui::test]
async fn route_yolo_mode_changed_increments_counter(cx: &mut TestAppContext) {
    let _ = cx;
    assert_route_count(
        ViewCommand::YoloModeChanged { active: true },
        1,
        |targets| targets.yolo_mode_changed_count,
    );
}

#[gpui::test]
async fn handle_command_forwards_tool_approval_policy_to_settings_view(cx: &mut TestAppContext) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        panel.handle_command(
            ViewCommand::ToolApprovalPolicyUpdated {
                yolo_mode: true,
                auto_approve_reads: true,
                skills_auto_approve: false,
                mcp_approval_mode: crate::agent::McpApprovalMode::PerServer,
                persistent_allowlist: vec!["git".to_string(), "ls".to_string()],
                persistent_denylist: vec!["rm".to_string()],
            },
            cx,
        );

        let settings_view = panel
            .settings_view
            .as_ref()
            .expect("settings view initialized");
        settings_view.read_with(cx, |view, _| {
            let state = view.get_state();
            assert!(state.yolo_mode);
            assert!(state.auto_approve_reads);
            assert_eq!(
                state.mcp_approval_mode,
                crate::agent::McpApprovalMode::PerServer
            );
            assert_eq!(state.persistent_allowlist, vec!["git", "ls"]);
            assert_eq!(state.persistent_denylist, vec!["rm"]);
        });
    });
}

#[gpui::test]
async fn handle_command_forwards_yolo_mode_changed_to_settings_and_chat(cx: &mut TestAppContext) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        panel.handle_command(ViewCommand::YoloModeChanged { active: true }, cx);

        let settings_view = panel
            .settings_view
            .as_ref()
            .expect("settings view initialized");
        settings_view.read_with(cx, |view, _| {
            assert!(view.get_state().yolo_mode);
        });

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        chat_view.read_with(cx, |view, _| {
            assert!(view.state.yolo_mode);
        });
    });
}

#[gpui::test]
async fn handle_command_forwards_tool_approval_commands_to_chat(cx: &mut TestAppContext) {
    let (app_state, _user_rx, _first_id, _second_id, _selected_profile_id) = build_app_state();
    cx.set_global(app_state);
    let panel = cx.new(MainPanel::new);

    panel.update(cx, |panel: &mut MainPanel, cx| {
        panel.init(cx);

        let conversation_id = Uuid::new_v4();
        panel.handle_command(
            ViewCommand::ToolApprovalRequest {
                conversation_id,
                request_id: "req-1".to_string(),
                context: ToolApprovalContext::new(
                    "WriteFile",
                    ToolCategory::FileWrite,
                    "/tmp/example.txt",
                ),
            },
            cx,
        );

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        chat_view.read_with(cx, |view, _| {
            let bubbles = view
                .state
                .approval_bubbles
                .get(&conversation_id)
                .expect("conversation bucket should exist");
            assert_eq!(bubbles.len(), 1);
            assert_eq!(bubbles[0].request_id, "req-1");
            assert_eq!(bubbles[0].context.tool_name, "WriteFile");
            assert_eq!(bubbles[0].context.primary_target, "/tmp/example.txt");
            assert_eq!(
                bubbles[0].state,
                crate::ui_gpui::views::chat_view::ApprovalBubbleState::Pending
            );
        });

        panel.handle_command(
            ViewCommand::ToolApprovalResolved {
                conversation_id,
                request_id: "req-1".to_string(),
                approved: false,
            },
            cx,
        );

        let chat_view = panel.chat_view.as_ref().expect("chat view initialized");
        chat_view.read_with(cx, |view, _| {
            assert!(
                view.state.approval_bubbles.is_empty(),
                "resolved approval bubble should be removed"
            );
        });
    });
}
