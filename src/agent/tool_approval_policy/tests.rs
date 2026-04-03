use tempfile::TempDir;

use super::{
    McpApprovalMode, ToolApprovalDecision, ToolApprovalPolicy, TOOL_APPROVAL_POLICY_SETTINGS_KEY,
};
use crate::services::{AppSettingsService, AppSettingsServiceImpl};

fn create_settings_service() -> (AppSettingsServiceImpl, TempDir) {
    let temp_dir = TempDir::new().expect("tempdir should be created");
    let settings_path = temp_dir.path().join("settings.json");
    let service =
        AppSettingsServiceImpl::new(settings_path).expect("settings service should initialize");
    (service, temp_dir)
}

#[test]
fn evaluate_returns_deny_when_denylist_prefix_matches() {
    let mut policy = ToolApprovalPolicy {
        yolo_mode: true,
        auto_approve_reads: true,
        persistent_allowlist: vec!["git status".to_string()],
        persistent_denylist: vec!["git".to_string()],
        ..ToolApprovalPolicy::default()
    };
    policy.allow_for_session("git status --short");

    assert_eq!(
        policy.evaluate("git status --short"),
        ToolApprovalDecision::Deny
    );
}

#[test]
fn evaluate_returns_allow_when_allowlist_prefix_matches() {
    let policy = ToolApprovalPolicy {
        persistent_allowlist: vec!["examcp".to_string()],
        ..ToolApprovalPolicy::default()
    };

    assert_eq!(
        policy.evaluate("examcp/web_search"),
        ToolApprovalDecision::Allow
    );
}

#[test]
fn evaluate_returns_allow_when_yolo_mode_enabled() {
    let policy = ToolApprovalPolicy {
        yolo_mode: true,
        ..ToolApprovalPolicy::default()
    };

    assert_eq!(
        policy.evaluate("unlisted/tool"),
        ToolApprovalDecision::Allow
    );
}

#[test]
fn evaluate_returns_allow_for_auto_approved_read_tools() {
    let policy = ToolApprovalPolicy {
        auto_approve_reads: true,
        ..ToolApprovalPolicy::default()
    };

    assert_eq!(policy.evaluate("ReadFile"), ToolApprovalDecision::Allow);
}

#[test]
fn evaluate_returns_allow_when_session_allowlist_prefix_matches() {
    let mut policy = ToolApprovalPolicy::default();
    policy.allow_for_session("myserver/list");

    assert_eq!(
        policy.evaluate("myserver/list_profiles"),
        ToolApprovalDecision::Allow
    );
}

#[test]
fn evaluate_returns_ask_user_when_no_rules_match() {
    let policy = ToolApprovalPolicy::default();

    assert_eq!(
        policy.evaluate("unknown/tool"),
        ToolApprovalDecision::AskUser
    );
}

#[test]
fn evaluate_prioritizes_denylist_over_allowlist_on_conflict() {
    let policy = ToolApprovalPolicy {
        persistent_allowlist: vec!["git status".to_string()],
        persistent_denylist: vec!["git status".to_string()],
        ..ToolApprovalPolicy::default()
    };

    assert_eq!(
        policy.evaluate("git status --short"),
        ToolApprovalDecision::Deny
    );
}

#[test]
fn mcp_identifier_uses_server_and_tool_in_per_tool_mode() {
    let policy = ToolApprovalPolicy {
        mcp_approval_mode: McpApprovalMode::PerTool,
        ..ToolApprovalPolicy::default()
    };

    assert_eq!(
        policy.mcp_tool_identifier("examcp", "web_search"),
        "examcp/web_search"
    );
}

#[test]
fn mcp_identifier_uses_server_only_in_per_server_mode() {
    let policy = ToolApprovalPolicy {
        mcp_approval_mode: McpApprovalMode::PerServer,
        ..ToolApprovalPolicy::default()
    };

    assert_eq!(policy.mcp_tool_identifier("examcp", "web_search"), "examcp");
}

#[test]
fn extract_shell_identifier_matches_issue_examples() {
    assert_eq!(
        ToolApprovalPolicy::extract_shell_identifier("git status --short"),
        "git status"
    );
    assert_eq!(
        ToolApprovalPolicy::extract_shell_identifier("ls -la /tmp"),
        "ls"
    );
    assert_eq!(
        ToolApprovalPolicy::extract_shell_identifier("cargo test --lib"),
        "cargo test"
    );
}

#[test]
fn split_compound_command_handles_issue_examples() {
    assert_eq!(
        ToolApprovalPolicy::split_compound_command("ls && rm -rf /"),
        vec!["ls".to_string(), "rm -rf /".to_string()]
    );
    assert_eq!(
        ToolApprovalPolicy::split_compound_command("git status || echo fail"),
        vec!["git status".to_string(), "echo fail".to_string()]
    );
    assert_eq!(
        ToolApprovalPolicy::split_compound_command("cat file | grep foo"),
        vec!["cat file".to_string(), "grep foo".to_string()]
    );
}

#[test]
fn split_compound_command_respects_quoted_operators() {
    assert_eq!(
        ToolApprovalPolicy::split_compound_command("echo \"a && b\" && ls"),
        vec!["echo \"a && b\"".to_string(), "ls".to_string()]
    );
    assert_eq!(
        ToolApprovalPolicy::split_compound_command(r#"echo "x || y" ; pwd"#),
        vec![r#"echo "x || y""#.to_string(), "pwd".to_string()]
    );
}

#[test]
fn extract_shell_identifiers_handles_compound_commands() {
    assert_eq!(
        ToolApprovalPolicy::extract_shell_identifiers("RUST_LOG=debug cargo test --lib && ls -la"),
        vec!["cargo test".to_string(), "ls".to_string()]
    );
    assert_eq!(
        ToolApprovalPolicy::extract_shell_identifiers("git status || echo fail"),
        vec!["git status".to_string(), "echo".to_string()]
    );
}

#[test]
fn evaluate_compound_command_uses_most_restrictive_decision() {
    let policy = ToolApprovalPolicy {
        persistent_allowlist: vec!["ls".to_string()],
        persistent_denylist: vec!["rm".to_string()],
        ..ToolApprovalPolicy::default()
    };

    assert_eq!(
        policy.evaluate_compound_command("ls && rm -rf /"),
        ToolApprovalDecision::Deny
    );
    assert_eq!(
        policy.evaluate_compound_command("ls && echo ok"),
        ToolApprovalDecision::AskUser
    );
}

#[test]
fn evaluate_compound_command_returns_allow_when_all_segments_allowed() {
    let policy = ToolApprovalPolicy {
        persistent_allowlist: vec!["git status".to_string(), "ls".to_string()],
        ..ToolApprovalPolicy::default()
    };

    assert_eq!(
        policy.evaluate_compound_command("git status --short && ls -la"),
        ToolApprovalDecision::Allow
    );
}

#[test]
fn evaluate_auto_approve_reads_handles_mcp_snake_case_tools() {
    let policy = ToolApprovalPolicy {
        auto_approve_reads: true,
        ..ToolApprovalPolicy::default()
    };

    assert_eq!(
        policy.evaluate("examcp/read_file"),
        ToolApprovalDecision::Allow
    );
    assert_eq!(
        policy.evaluate("examcp/list_resources"),
        ToolApprovalDecision::Allow
    );
}

#[test]
fn evaluate_auto_approve_reads_does_not_allow_mutating_names() {
    let policy = ToolApprovalPolicy {
        auto_approve_reads: true,
        ..ToolApprovalPolicy::default()
    };

    assert_eq!(
        policy.evaluate("examcp/get_or_create_user"),
        ToolApprovalDecision::AskUser
    );
    assert_eq!(
        policy.evaluate("examcp/fetch_and_update_profile"),
        ToolApprovalDecision::AskUser
    );
}

#[test]
fn extract_shell_identifier_skips_env_assignments() {
    assert_eq!(
        ToolApprovalPolicy::extract_shell_identifier("RUST_LOG=debug cargo test --lib"),
        "cargo test"
    );
}

#[tokio::test]
async fn load_from_settings_returns_default_when_setting_missing() {
    let (service, _temp_dir) = create_settings_service();
    let policy = ToolApprovalPolicy::load_from_settings(&service)
        .await
        .expect("load should succeed");

    assert_eq!(policy, ToolApprovalPolicy::default());
}

#[tokio::test]
async fn load_from_settings_falls_back_to_default_on_malformed_json() {
    let (service, _temp_dir) = create_settings_service();
    service
        .set_setting(TOOL_APPROVAL_POLICY_SETTINGS_KEY, "{not-json".to_string())
        .await
        .expect("set setting should succeed");

    let policy = ToolApprovalPolicy::load_from_settings(&service)
        .await
        .expect("load should succeed");

    assert_eq!(policy, ToolApprovalPolicy::default());
}

#[tokio::test]
async fn save_and_load_round_trip_persists_non_session_fields() {
    let (service, _temp_dir) = create_settings_service();
    let mut policy = ToolApprovalPolicy {
        yolo_mode: true,
        auto_approve_reads: true,
        mcp_approval_mode: McpApprovalMode::PerServer,
        persistent_allowlist: vec!["git status".to_string()],
        persistent_denylist: vec!["git push".to_string()],
        ..ToolApprovalPolicy::default()
    };
    policy.allow_for_session("temporary/session");

    policy
        .save_to_settings(&service)
        .await
        .expect("save should succeed");

    let loaded = ToolApprovalPolicy::load_from_settings(&service)
        .await
        .expect("load should succeed");

    assert!(loaded.yolo_mode);
    assert!(loaded.auto_approve_reads);
    assert_eq!(loaded.mcp_approval_mode, McpApprovalMode::PerServer);
    assert_eq!(loaded.persistent_allowlist, vec!["git status".to_string()]);
    assert_eq!(loaded.persistent_denylist, vec!["git push".to_string()]);
    assert!(loaded.session_allowlist.is_empty());
}

#[tokio::test]
async fn allow_persistently_updates_allowlist_and_saves() {
    let (service, _temp_dir) = create_settings_service();
    let mut policy = ToolApprovalPolicy::default();

    policy
        .allow_persistently("examcp/web_search", &service)
        .await
        .expect("allow persistently should succeed");

    let loaded = ToolApprovalPolicy::load_from_settings(&service)
        .await
        .expect("load should succeed");

    assert_eq!(
        loaded.persistent_allowlist,
        vec!["examcp/web_search".to_string()]
    );
}

#[tokio::test]
async fn allow_persistently_noops_for_empty_identifier_without_saving() {
    let (service, _temp_dir) = create_settings_service();
    let mut policy = ToolApprovalPolicy::default();

    policy
        .allow_persistently("", &service)
        .await
        .expect("empty identifier should be a no-op");

    let stored = service
        .get_setting(TOOL_APPROVAL_POLICY_SETTINGS_KEY)
        .await
        .expect("reading setting should succeed");

    assert!(stored.is_none());
}

#[tokio::test]
async fn allow_persistently_noops_for_duplicate_identifier_without_resaving() {
    let (service, _temp_dir) = create_settings_service();
    let mut policy = ToolApprovalPolicy::default();

    policy
        .allow_persistently("examcp/web_search", &service)
        .await
        .expect("initial allow should persist");
    let first_saved = service
        .get_setting(TOOL_APPROVAL_POLICY_SETTINGS_KEY)
        .await
        .expect("reading first saved policy should succeed")
        .expect("first save should exist");

    policy
        .allow_persistently("examcp/web_search", &service)
        .await
        .expect("duplicate allow should be a no-op");
    let second_saved = service
        .get_setting(TOOL_APPROVAL_POLICY_SETTINGS_KEY)
        .await
        .expect("reading second saved policy should succeed")
        .expect("existing save should still exist");

    assert_eq!(first_saved, second_saved);
    assert_eq!(
        policy.persistent_allowlist,
        vec!["examcp/web_search".to_string()]
    );
}

#[tokio::test]
async fn deny_persistently_updates_denylist_and_saves() {
    let (service, _temp_dir) = create_settings_service();
    let mut policy = ToolApprovalPolicy::default();

    policy
        .deny_persistently("rm", &service)
        .await
        .expect("deny persistently should succeed");

    let loaded = ToolApprovalPolicy::load_from_settings(&service)
        .await
        .expect("load should succeed");

    assert_eq!(loaded.persistent_denylist, vec!["rm".to_string()]);
}

#[tokio::test]
async fn remove_persistent_allow_prefix_updates_allowlist_and_saves() {
    let (service, _temp_dir) = create_settings_service();
    let mut policy = ToolApprovalPolicy::default();
    policy
        .allow_persistently("examcp/web_search", &service)
        .await
        .expect("initial allow should persist");

    policy
        .remove_persistent_allow_prefix("examcp/web_search", &service)
        .await
        .expect("remove allow should succeed");

    let loaded = ToolApprovalPolicy::load_from_settings(&service)
        .await
        .expect("load should succeed");

    assert!(loaded.persistent_allowlist.is_empty());
}

#[tokio::test]
async fn remove_persistent_deny_prefix_updates_denylist_and_saves() {
    let (service, _temp_dir) = create_settings_service();
    let mut policy = ToolApprovalPolicy::default();
    policy
        .deny_persistently("rm", &service)
        .await
        .expect("initial deny should persist");

    policy
        .remove_persistent_deny_prefix("rm", &service)
        .await
        .expect("remove deny should succeed");

    let loaded = ToolApprovalPolicy::load_from_settings(&service)
        .await
        .expect("load should succeed");

    assert!(loaded.persistent_denylist.is_empty());
}

#[tokio::test]
async fn session_allowlist_does_not_persist_after_save_and_reload() {
    let (service, _temp_dir) = create_settings_service();
    let mut policy = ToolApprovalPolicy::default();
    policy.allow_for_session("session-only");

    policy
        .save_to_settings(&service)
        .await
        .expect("save should succeed");

    let loaded = ToolApprovalPolicy::load_from_settings(&service)
        .await
        .expect("load should succeed");

    assert!(loaded.session_allowlist.is_empty());
}

#[test]
fn clear_session_allowlist_removes_all_entries() {
    let mut policy = ToolApprovalPolicy::default();
    policy.allow_for_session("one");
    policy.allow_for_session("two");

    policy.clear_session_allowlist();

    assert!(policy.session_allowlist.is_empty());
}
