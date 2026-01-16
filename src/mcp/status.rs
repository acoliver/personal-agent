//! MCP Status tracking and display helpers

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum McpStatus {
    Disabled,
    Stopped,
    Starting,
    Running,
    Error(String),
    Restarting,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AggregateStatus {
    AllHealthy,
    PartialFailure,
    AllFailed,
    NoMcps,
}

impl McpStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, McpStatus::Running)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, McpStatus::Error(_))
    }

    pub fn display_name(&self) -> &str {
        match self {
            McpStatus::Disabled => "Disabled",
            McpStatus::Stopped => "Stopped",
            McpStatus::Starting => "Starting...",
            McpStatus::Running => "Running",
            McpStatus::Error(_) => "Error",
            McpStatus::Restarting => "Restarting...",
        }
    }

    pub fn status_color(&self) -> (f64, f64, f64) {
        match self {
            McpStatus::Disabled => (0.3, 0.3, 0.3),   // Dark Gray
            McpStatus::Stopped => (0.5, 0.5, 0.5),    // Gray
            McpStatus::Starting => (1.0, 0.8, 0.0),   // Yellow
            McpStatus::Running => (0.0, 0.8, 0.0),    // Green
            McpStatus::Error(_) => (0.8, 0.0, 0.0),   // Red
            McpStatus::Restarting => (1.0, 0.8, 0.0), // Yellow
        }
    }
}

/// MCP Status Manager - Thread-safe status tracking
///
/// This manager can be safely shared across threads using Arc<McpStatusManager>.
/// All operations use interior mutability via RwLock.
#[derive(Clone)]
pub struct McpStatusManager {
    statuses: Arc<RwLock<HashMap<Uuid, McpStatus>>>,
}

impl McpStatusManager {
    pub fn new() -> Self {
        Self {
            statuses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set status for an MCP (thread-safe)
    pub fn set_status(&self, id: Uuid, status: McpStatus) {
        if let Ok(mut statuses) = self.statuses.write() {
            statuses.insert(id, status);
        }
    }

    /// Get status for an MCP (thread-safe)
    pub fn get_status(&self, id: &Uuid) -> McpStatus {
        self.statuses
            .read()
            .ok()
            .and_then(|s| s.get(id).cloned())
            .unwrap_or(McpStatus::Stopped)
    }

    /// Clear status for an MCP (thread-safe)
    pub fn clear(&self, id: &Uuid) {
        if let Ok(mut statuses) = self.statuses.write() {
            statuses.remove(id);
        }
    }

    /// Get a snapshot of all statuses (thread-safe)
    pub fn get_all_statuses(&self) -> HashMap<Uuid, McpStatus> {
        self.statuses.read().map(|s| s.clone()).unwrap_or_default()
    }

    /// Count running MCPs (thread-safe)
    pub fn count_running(&self) -> usize {
        self.statuses
            .read()
            .map(|s| s.values().filter(|v| v.is_running()).count())
            .unwrap_or(0)
    }

    /// Count MCPs in error state (thread-safe)
    pub fn count_errors(&self) -> usize {
        self.statuses
            .read()
            .map(|s| s.values().filter(|v| v.is_error()).count())
            .unwrap_or(0)
    }
}

impl Default for McpStatusManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Get status for a config (before toolset creation)
pub fn get_config_status(config: &crate::mcp::McpConfig) -> McpStatus {
    if !config.enabled {
        McpStatus::Disabled
    } else {
        McpStatus::Starting
    }
}

/// Aggregate status from multiple MCPs
pub fn aggregate_mcp_status(statuses: &[McpStatus]) -> AggregateStatus {
    if statuses.is_empty() {
        return AggregateStatus::NoMcps;
    }

    let running = statuses
        .iter()
        .filter(|s| matches!(s, McpStatus::Running))
        .count();
    let errors = statuses
        .iter()
        .filter(|s| matches!(s, McpStatus::Error(_)))
        .count();

    if errors == statuses.len() {
        AggregateStatus::AllFailed
    } else if errors > 0 {
        AggregateStatus::PartialFailure
    } else if running == statuses.len() {
        AggregateStatus::AllHealthy
    } else {
        AggregateStatus::PartialFailure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_display_name() {
        assert_eq!(McpStatus::Disabled.display_name(), "Disabled");
        assert_eq!(McpStatus::Stopped.display_name(), "Stopped");
        assert_eq!(McpStatus::Starting.display_name(), "Starting...");
        assert_eq!(McpStatus::Running.display_name(), "Running");
        assert_eq!(McpStatus::Error("test".to_string()).display_name(), "Error");
        assert_eq!(McpStatus::Restarting.display_name(), "Restarting...");
    }

    #[test]
    fn test_status_is_running() {
        assert!(!McpStatus::Disabled.is_running());
        assert!(!McpStatus::Stopped.is_running());
        assert!(!McpStatus::Starting.is_running());
        assert!(McpStatus::Running.is_running());
        assert!(!McpStatus::Error("test".to_string()).is_running());
        assert!(!McpStatus::Restarting.is_running());
    }

    #[test]
    fn test_status_is_error() {
        assert!(!McpStatus::Disabled.is_error());
        assert!(!McpStatus::Stopped.is_error());
        assert!(!McpStatus::Starting.is_error());
        assert!(!McpStatus::Running.is_error());
        assert!(McpStatus::Error("test".to_string()).is_error());
        assert!(!McpStatus::Restarting.is_error());
    }

    #[test]
    fn test_status_color() {
        let dark_gray = (0.3, 0.3, 0.3);
        let gray = (0.5, 0.5, 0.5);
        let yellow = (1.0, 0.8, 0.0);
        let green = (0.0, 0.8, 0.0);
        let red = (0.8, 0.0, 0.0);

        assert_eq!(McpStatus::Disabled.status_color(), dark_gray);
        assert_eq!(McpStatus::Stopped.status_color(), gray);
        assert_eq!(McpStatus::Starting.status_color(), yellow);
        assert_eq!(McpStatus::Running.status_color(), green);
        assert_eq!(McpStatus::Error("test".to_string()).status_color(), red);
        assert_eq!(McpStatus::Restarting.status_color(), yellow);
    }

    #[test]
    fn test_status_manager_new() {
        let manager = McpStatusManager::new();
        assert_eq!(manager.get_all_statuses().len(), 0);
    }

    #[test]
    fn test_status_manager_default() {
        let manager = McpStatusManager::default();
        assert_eq!(manager.get_all_statuses().len(), 0);
    }

    #[test]
    fn test_set_and_get_status() {
        let manager = McpStatusManager::new();
        let id = Uuid::new_v4();

        manager.set_status(id, McpStatus::Running);
        assert_eq!(manager.get_status(&id), McpStatus::Running);

        manager.set_status(id, McpStatus::Error("failed".to_string()));
        assert_eq!(
            manager.get_status(&id),
            McpStatus::Error("failed".to_string())
        );
    }

    #[test]
    fn test_get_status_default() {
        let manager = McpStatusManager::new();
        let id = Uuid::new_v4();

        // Should return Stopped for unknown IDs
        assert_eq!(manager.get_status(&id), McpStatus::Stopped);
    }

    #[test]
    fn test_clear_status() {
        let manager = McpStatusManager::new();
        let id = Uuid::new_v4();

        manager.set_status(id, McpStatus::Running);
        assert_eq!(manager.get_status(&id), McpStatus::Running);

        manager.clear(&id);
        assert_eq!(manager.get_status(&id), McpStatus::Stopped);
    }

    #[test]
    fn test_get_all_statuses() {
        let manager = McpStatusManager::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        manager.set_status(id1, McpStatus::Running);
        manager.set_status(id2, McpStatus::Stopped);

        let all = manager.get_all_statuses();
        assert_eq!(all.len(), 2);
        assert_eq!(all.get(&id1), Some(&McpStatus::Running));
        assert_eq!(all.get(&id2), Some(&McpStatus::Stopped));
    }

    #[test]
    fn test_count_running() {
        let manager = McpStatusManager::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        assert_eq!(manager.count_running(), 0);

        manager.set_status(id1, McpStatus::Running);
        assert_eq!(manager.count_running(), 1);

        manager.set_status(id2, McpStatus::Running);
        assert_eq!(manager.count_running(), 2);

        manager.set_status(id3, McpStatus::Stopped);
        assert_eq!(manager.count_running(), 2);
    }

    #[test]
    fn test_count_errors() {
        let manager = McpStatusManager::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        assert_eq!(manager.count_errors(), 0);

        manager.set_status(id1, McpStatus::Error("fail1".to_string()));
        assert_eq!(manager.count_errors(), 1);

        manager.set_status(id2, McpStatus::Error("fail2".to_string()));
        assert_eq!(manager.count_errors(), 2);

        manager.set_status(id3, McpStatus::Running);
        assert_eq!(manager.count_errors(), 2);
    }

    #[test]
    fn test_status_equality() {
        assert_eq!(McpStatus::Disabled, McpStatus::Disabled);
        assert_eq!(McpStatus::Stopped, McpStatus::Stopped);
        assert_eq!(McpStatus::Running, McpStatus::Running);
        assert_eq!(
            McpStatus::Error("test".to_string()),
            McpStatus::Error("test".to_string())
        );
        assert_ne!(
            McpStatus::Error("test1".to_string()),
            McpStatus::Error("test2".to_string())
        );
        assert_ne!(McpStatus::Running, McpStatus::Stopped);
    }

    #[test]
    fn test_multiple_updates() {
        let manager = McpStatusManager::new();
        let id = Uuid::new_v4();

        manager.set_status(id, McpStatus::Starting);
        assert_eq!(manager.get_status(&id), McpStatus::Starting);

        manager.set_status(id, McpStatus::Running);
        assert_eq!(manager.get_status(&id), McpStatus::Running);

        manager.set_status(id, McpStatus::Error("crash".to_string()));
        assert_eq!(
            manager.get_status(&id),
            McpStatus::Error("crash".to_string())
        );

        manager.set_status(id, McpStatus::Restarting);
        assert_eq!(manager.get_status(&id), McpStatus::Restarting);

        manager.set_status(id, McpStatus::Running);
        assert_eq!(manager.get_status(&id), McpStatus::Running);
    }

    #[test]
    fn test_multiple_mcps() {
        let manager = McpStatusManager::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        manager.set_status(id1, McpStatus::Running);
        manager.set_status(id2, McpStatus::Error("failed".to_string()));
        manager.set_status(id3, McpStatus::Starting);

        assert_eq!(manager.count_running(), 1);
        assert_eq!(manager.count_errors(), 1);

        assert_eq!(manager.get_status(&id1), McpStatus::Running);
        assert_eq!(
            manager.get_status(&id2),
            McpStatus::Error("failed".to_string())
        );
        assert_eq!(manager.get_status(&id3), McpStatus::Starting);
    }

    #[tokio::test]
    async fn test_status_for_disabled_mcp() {
        use crate::mcp::McpConfig;
        use uuid::Uuid;

        let mut config = McpConfig {
            id: Uuid::new_v4(),
            name: "test".to_string(),
            enabled: false,
            source: crate::mcp::McpSource::Official {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
            },
            package: crate::mcp::McpPackage {
                package_type: crate::mcp::McpPackageType::Npm,
                identifier: "test".to_string(),
                runtime_hint: None,
            },
            transport: crate::mcp::McpTransport::Stdio,
            auth_type: crate::mcp::McpAuthType::None,
            env_vars: vec![],
            keyfile_path: None,
            config: serde_json::json!({}),
            oauth_token: None,
        };

        let status = get_config_status(&config);
        assert_eq!(status, McpStatus::Disabled);

        // When enabled, should be Starting
        config.enabled = true;
        let status = get_config_status(&config);
        assert_eq!(status, McpStatus::Starting);
    }

    #[tokio::test]
    async fn test_aggregate_status_all_running() {
        let statuses = vec![McpStatus::Running, McpStatus::Running];
        let aggregate = aggregate_mcp_status(&statuses);
        assert_eq!(aggregate, AggregateStatus::AllHealthy);
    }

    #[tokio::test]
    async fn test_aggregate_status_some_errors() {
        let statuses = vec![McpStatus::Running, McpStatus::Error("test".to_string())];
        let aggregate = aggregate_mcp_status(&statuses);
        assert_eq!(aggregate, AggregateStatus::PartialFailure);
    }

    #[tokio::test]
    async fn test_aggregate_status_all_failed() {
        let statuses = vec![
            McpStatus::Error("err1".to_string()),
            McpStatus::Error("err2".to_string()),
        ];
        let aggregate = aggregate_mcp_status(&statuses);
        assert_eq!(aggregate, AggregateStatus::AllFailed);
    }

    #[tokio::test]
    async fn test_aggregate_status_no_mcps() {
        let statuses: Vec<McpStatus> = vec![];
        let aggregate = aggregate_mcp_status(&statuses);
        assert_eq!(aggregate, AggregateStatus::NoMcps);
    }

    #[tokio::test]
    async fn test_aggregate_status_mixed_states() {
        // Test with various non-running, non-error states
        let statuses = vec![McpStatus::Running, McpStatus::Starting, McpStatus::Stopped];
        let aggregate = aggregate_mcp_status(&statuses);
        assert_eq!(aggregate, AggregateStatus::PartialFailure);
    }

    #[tokio::test]
    async fn test_aggregate_status_only_disabled() {
        let statuses = vec![McpStatus::Disabled, McpStatus::Disabled];
        let aggregate = aggregate_mcp_status(&statuses);
        assert_eq!(aggregate, AggregateStatus::PartialFailure);
    }
}
