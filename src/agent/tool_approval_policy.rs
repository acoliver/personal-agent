//! Tool approval policy engine.
//!
//! This module provides deterministic policy evaluation for tool execution.
//! Decisions are evaluated in strict order using prefix matching semantics.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::services::{AppSettingsService, ServiceError, ServiceResult};

/// Settings key for persisted approval policy configuration.
pub const TOOL_APPROVAL_POLICY_SETTINGS_KEY: &str = "tool_approval.policy";

/// How MCP tools are represented for approval checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum McpApprovalMode {
    /// Match individual MCP tools using `server/tool`.
    #[default]
    PerTool,
    /// Match all tools on a server using `server`.
    PerServer,
}

/// Result of evaluating a tool invocation against approval policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolApprovalDecision {
    Allow,
    Deny,
    AskUser,
}

/// Approval policy state.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolApprovalPolicy {
    pub yolo_mode: bool,
    pub auto_approve_reads: bool,
    pub mcp_approval_mode: McpApprovalMode,
    pub persistent_allowlist: Vec<String>,
    pub persistent_denylist: Vec<String>,
    pub session_allowlist: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct PersistedToolApprovalPolicy {
    yolo_mode: bool,
    auto_approve_reads: bool,
    mcp_approval_mode: McpApprovalMode,
    persistent_allowlist: Vec<String>,
    persistent_denylist: Vec<String>,
}

impl Default for ToolApprovalPolicy {
    fn default() -> Self {
        Self {
            yolo_mode: false,
            auto_approve_reads: false,
            mcp_approval_mode: McpApprovalMode::PerTool,
            persistent_allowlist: Vec::new(),
            persistent_denylist: Vec::new(),
            session_allowlist: HashSet::new(),
        }
    }
}

impl From<PersistedToolApprovalPolicy> for ToolApprovalPolicy {
    fn from(value: PersistedToolApprovalPolicy) -> Self {
        Self {
            yolo_mode: value.yolo_mode,
            auto_approve_reads: value.auto_approve_reads,
            mcp_approval_mode: value.mcp_approval_mode,
            persistent_allowlist: value.persistent_allowlist,
            persistent_denylist: value.persistent_denylist,
            session_allowlist: HashSet::new(),
        }
    }
}

impl From<&ToolApprovalPolicy> for PersistedToolApprovalPolicy {
    fn from(value: &ToolApprovalPolicy) -> Self {
        Self {
            yolo_mode: value.yolo_mode,
            auto_approve_reads: value.auto_approve_reads,
            mcp_approval_mode: value.mcp_approval_mode,
            persistent_allowlist: value.persistent_allowlist.clone(),
            persistent_denylist: value.persistent_denylist.clone(),
        }
    }
}

impl ToolApprovalPolicy {
    /// Evaluate a tool identifier using issue-defined precedence.
    #[must_use]
    pub fn evaluate(&self, tool_identifier: &str) -> ToolApprovalDecision {
        if Self::matches_prefix_in_slice(&self.persistent_denylist, tool_identifier) {
            return ToolApprovalDecision::Deny;
        }

        if Self::matches_prefix_in_slice(&self.persistent_allowlist, tool_identifier) {
            return ToolApprovalDecision::Allow;
        }

        if self.yolo_mode {
            return ToolApprovalDecision::Allow;
        }

        if self.auto_approve_reads && Self::is_read_tool_identifier(tool_identifier) {
            return ToolApprovalDecision::Allow;
        }

        if Self::matches_prefix_in_set(&self.session_allowlist, tool_identifier) {
            return ToolApprovalDecision::Allow;
        }

        ToolApprovalDecision::AskUser
    }

    /// Load policy from app settings, defaulting safely on missing/malformed data.
    ///
    /// # Errors
    ///
    /// Returns an error when reading from the settings service fails.
    pub async fn load_from_settings(app_settings: &dyn AppSettingsService) -> ServiceResult<Self> {
        let stored = app_settings
            .get_setting(TOOL_APPROVAL_POLICY_SETTINGS_KEY)
            .await?;

        let Some(raw) = stored else {
            return Ok(Self::default());
        };

        serde_json::from_str::<PersistedToolApprovalPolicy>(&raw)
            .map_or_else(|_| Ok(Self::default()), |parsed| Ok(parsed.into()))
    }

    /// Persist policy settings (excluding session allowlist).
    ///
    /// # Errors
    ///
    /// Returns an error when serialization or settings writes fail.
    pub async fn save_to_settings(
        &self,
        app_settings: &dyn AppSettingsService,
    ) -> ServiceResult<()> {
        let persisted = PersistedToolApprovalPolicy::from(self);
        let serialized = serde_json::to_string(&persisted)
            .map_err(|error| ServiceError::Serialization(error.to_string()))?;

        app_settings
            .set_setting(TOOL_APPROVAL_POLICY_SETTINGS_KEY, serialized)
            .await
    }

    /// Add identifier to persistent allowlist and save to settings.
    ///
    /// # Errors
    ///
    /// Returns an error when persisting updated policy fails.
    pub async fn allow_persistently(
        &mut self,
        identifier: impl Into<String>,
        app_settings: &dyn AppSettingsService,
    ) -> ServiceResult<()> {
        let identifier = identifier.into();
        if !identifier.is_empty() && !self.persistent_allowlist.contains(&identifier) {
            self.persistent_allowlist.push(identifier);
        }

        self.save_to_settings(app_settings).await
    }

    /// Add identifier to session-scoped allowlist (in-memory only).
    pub fn allow_for_session(&mut self, identifier: impl Into<String>) {
        let identifier = identifier.into();
        if identifier.is_empty() {
            return;
        }

        self.session_allowlist.insert(identifier);
    }

    /// Clear session-scoped allowlist.
    pub fn clear_session_allowlist(&mut self) {
        self.session_allowlist.clear();
    }

    /// Build MCP tool identifier according to current approval mode.
    #[must_use]
    pub fn mcp_tool_identifier(&self, server: &str, tool_name: &str) -> String {
        match self.mcp_approval_mode {
            McpApprovalMode::PerTool => format!("{server}/{tool_name}"),
            McpApprovalMode::PerServer => server.to_string(),
        }
    }

    /// Extract a shell identifier root from command text.
    ///
    /// Examples:
    /// - `git status --short` -> `git status`
    /// - `ls -la /tmp` -> `ls`
    /// - `cargo test --lib` -> `cargo test`
    #[must_use]
    pub fn extract_shell_identifier(command: &str) -> String {
        let tokens = Self::parse_shell_tokens(command);
        if tokens.is_empty() {
            return String::new();
        }

        let mut index = 0usize;
        while index < tokens.len() && Self::is_env_assignment_token(&tokens[index]) {
            index += 1;
        }

        let Some(command_token) = tokens.get(index) else {
            return String::new();
        };

        if matches!(command_token.as_str(), "git" | "cargo") {
            let maybe_subcommand = tokens
                .get(index + 1)
                .filter(|token| !token.starts_with('-'));
            if let Some(subcommand) = maybe_subcommand {
                return format!("{command_token} {subcommand}");
            }
        }

        command_token.clone()
    }

    #[must_use]
    fn matches_prefix_in_slice(entries: &[String], tool_identifier: &str) -> bool {
        entries
            .iter()
            .filter(|entry| !entry.is_empty())
            .any(|entry| tool_identifier.starts_with(entry))
    }

    #[must_use]
    fn matches_prefix_in_set(entries: &HashSet<String>, tool_identifier: &str) -> bool {
        entries
            .iter()
            .filter(|entry| !entry.is_empty())
            .any(|entry| tool_identifier.starts_with(entry))
    }

    #[must_use]
    fn is_read_tool_identifier(tool_identifier: &str) -> bool {
        const READ_TOOL_NAMES: &[&str] = &[
            "ReadFile",
            "ReadLineRange",
            "ReadManyFiles",
            "ListDirectory",
            "Glob",
            "SearchFileContent",
            "AstReadFile",
            "AstGrep",
            "StructuralAnalysis",
        ];
        const READ_TOKENS: &[&str] = &["read", "get", "list", "search", "fetch", "query"];
        const MUTATING_TOKENS: &[&str] = &[
            "create", "write", "update", "delete", "remove", "set", "apply", "run", "exec",
            "insert", "patch", "post", "put", "send", "start", "stop", "restart", "install",
            "save", "edit", "rename", "move", "copy", "clear",
        ];

        if READ_TOOL_NAMES
            .iter()
            .any(|name| tool_identifier.eq_ignore_ascii_case(name))
        {
            return true;
        }

        let candidate = tool_identifier
            .split_once('/')
            .map_or(tool_identifier, |(_, tool)| tool)
            .to_ascii_lowercase();

        let segments: Vec<&str> = candidate
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|segment| !segment.is_empty())
            .collect();

        if segments
            .iter()
            .any(|segment| MUTATING_TOKENS.contains(segment))
        {
            return false;
        }

        if segments.iter().any(|segment| READ_TOKENS.contains(segment)) {
            return true;
        }

        ["read", "list", "search", "fetch", "query"]
            .iter()
            .any(|verb| candidate.starts_with(verb))
            || candidate == "get"
            || candidate.starts_with("get_")
            || candidate.starts_with("get-")
            || candidate.starts_with("get/")
            || candidate.starts_with("get.")
    }

    #[must_use]
    fn parse_shell_tokens(command: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut current = String::new();
        let mut quote: Option<char> = None;
        let mut escaped = false;

        for ch in command.chars() {
            if escaped {
                current.push(ch);
                escaped = false;
                continue;
            }

            if ch == '\\' {
                escaped = true;
                continue;
            }

            if let Some(active_quote) = quote {
                if ch == active_quote {
                    quote = None;
                } else {
                    current.push(ch);
                }
                continue;
            }

            match ch {
                '"' | '\'' => {
                    quote = Some(ch);
                }
                c if c.is_whitespace() => {
                    if !current.is_empty() {
                        tokens.push(std::mem::take(&mut current));
                    }
                }
                _ => current.push(ch),
            }
        }

        if escaped {
            current.push('\\');
        }

        if !current.is_empty() {
            tokens.push(current);
        }

        tokens
    }

    #[must_use]
    fn is_env_assignment_token(token: &str) -> bool {
        let Some((name, _value)) = token.split_once('=') else {
            return false;
        };

        !name.is_empty()
            && name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{
        McpApprovalMode, ToolApprovalDecision, ToolApprovalPolicy,
        TOOL_APPROVAL_POLICY_SETTINGS_KEY,
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
}
