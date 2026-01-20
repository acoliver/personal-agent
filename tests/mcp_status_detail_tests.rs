use personal_agent::mcp::{McpStatus, McpStatusManager};
use uuid::Uuid;

#[test]
fn status_manager_reports_status_and_counts() {
    let manager = McpStatusManager::new();
    let id = Uuid::new_v4();

    assert_eq!(manager.get_status(&id), McpStatus::Stopped);

    manager.set_status(id, McpStatus::Running);
    assert_eq!(manager.get_status(&id), McpStatus::Running);
    assert_eq!(manager.count_running(), 1);
}
