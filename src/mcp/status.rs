//! MCP Status tracking and display helpers

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpStatus {
    Disabled,
    Stopped,
    Starting,
    Running,
    Error(String),
    Restarting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AggregateStatus {
    AllHealthy,
    PartialFailure,
    AllFailed,
    NoMcps,
}

impl McpStatus {
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    #[must_use]
    pub const fn display_name(&self) -> &str {
        match self {
            Self::Disabled => "Disabled",
            Self::Stopped => "Stopped",
            Self::Starting => "Starting...",
            Self::Running => "Running",
            Self::Error(_) => "Error",
            Self::Restarting => "Restarting...",
        }
    }

    #[must_use]
    pub const fn status_color(&self) -> (f64, f64, f64) {
        match self {
            Self::Disabled => (0.3, 0.3, 0.3),                    // Dark Gray
            Self::Stopped => (0.5, 0.5, 0.5),                     // Gray
            Self::Starting | Self::Restarting => (1.0, 0.8, 0.0), // Yellow
            Self::Running => (0.0, 0.8, 0.0),                     // Green
            Self::Error(_) => (0.8, 0.0, 0.0),                    // Red
        }
    }
}

/// MCP Status Manager - Thread-safe status tracking
///
/// This manager can be safely shared across threads using `Arc<McpStatusManager>`.
/// All operations use interior mutability via `RwLock`.
#[derive(Clone)]
pub struct McpStatusManager {
    statuses: Arc<RwLock<HashMap<Uuid, McpStatus>>>,
}

impl McpStatusManager {
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn get_all_statuses(&self) -> HashMap<Uuid, McpStatus> {
        self.statuses.read().map(|s| s.clone()).unwrap_or_default()
    }

    /// Count running MCPs (thread-safe)
    #[must_use]
    pub fn count_running(&self) -> usize {
        self.statuses
            .read()
            .map(|s| s.values().filter(|v| v.is_running()).count())
            .unwrap_or(0)
    }

    /// Count MCPs in error state (thread-safe)
    #[must_use]
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
#[must_use]
pub const fn get_config_status(config: &crate::mcp::McpConfig) -> McpStatus {
    if config.enabled {
        McpStatus::Starting
    } else {
        McpStatus::Disabled
    }
}

/// Aggregate status from multiple MCPs
#[must_use]
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
