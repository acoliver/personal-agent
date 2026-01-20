use personal_agent::mcp::{aggregate_mcp_status, AggregateStatus, McpStatus};

#[test]
fn aggregate_status_reports_all_failed() {
    let statuses = vec![
        McpStatus::Error("oops".to_string()),
        McpStatus::Error("again".to_string()),
    ];
    assert_eq!(aggregate_mcp_status(&statuses), AggregateStatus::AllFailed);
}

#[test]
fn aggregate_status_reports_all_healthy() {
    let statuses = vec![McpStatus::Running, McpStatus::Running];
    assert_eq!(aggregate_mcp_status(&statuses), AggregateStatus::AllHealthy);
}
