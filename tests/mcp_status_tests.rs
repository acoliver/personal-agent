use personal_agent::mcp::{aggregate_mcp_status, McpStatus, McpStatusManager};
use uuid::Uuid;

#[test]
fn mcp_status_manager_tracks_errors() {
    let manager = McpStatusManager::new();
    let id = Uuid::new_v4();

    manager.set_status(id, McpStatus::Error("oops".to_string()));

    assert!(manager.get_status(&id).is_error());
    assert_eq!(manager.count_errors(), 1);
}

#[test]
fn aggregate_status_handles_partial_failure() {
    let statuses = vec![McpStatus::Running, McpStatus::Error("fail".to_string())];
    let aggregate = aggregate_mcp_status(&statuses);

    assert_eq!(
        aggregate,
        personal_agent::mcp::AggregateStatus::PartialFailure
    );
}
