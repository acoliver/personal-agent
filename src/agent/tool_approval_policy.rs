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
        if identifier.is_empty() || self.persistent_allowlist.contains(&identifier) {
            return Ok(());
        }

        self.persistent_allowlist.push(identifier);
        self.save_to_settings(app_settings).await
    }

    /// Add identifier to persistent denylist and save to settings.
    ///
    /// # Errors
    ///
    /// Returns an error when persisting updated policy fails.
    pub async fn deny_persistently(
        &mut self,
        identifier: impl Into<String>,
        app_settings: &dyn AppSettingsService,
    ) -> ServiceResult<()> {
        let identifier = identifier.into();
        if identifier.is_empty() || self.persistent_denylist.contains(&identifier) {
            return Ok(());
        }

        self.persistent_denylist.push(identifier);
        self.save_to_settings(app_settings).await
    }

    /// Remove identifier from persistent allowlist and save to settings.
    ///
    /// # Errors
    ///
    /// Returns an error when persisting updated policy fails.
    pub async fn remove_persistent_allow_prefix(
        &mut self,
        identifier: &str,
        app_settings: &dyn AppSettingsService,
    ) -> ServiceResult<()> {
        if identifier.is_empty() {
            return Ok(());
        }

        let original_len = self.persistent_allowlist.len();
        self.persistent_allowlist
            .retain(|entry| entry != identifier);
        if self.persistent_allowlist.len() == original_len {
            return Ok(());
        }

        self.save_to_settings(app_settings).await
    }

    /// Remove identifier from persistent denylist and save to settings.
    ///
    /// # Errors
    ///
    /// Returns an error when persisting updated policy fails.
    pub async fn remove_persistent_deny_prefix(
        &mut self,
        identifier: &str,
        app_settings: &dyn AppSettingsService,
    ) -> ServiceResult<()> {
        if identifier.is_empty() {
            return Ok(());
        }

        let original_len = self.persistent_denylist.len();
        self.persistent_denylist.retain(|entry| entry != identifier);
        if self.persistent_denylist.len() == original_len {
            return Ok(());
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

    /// Split compound shell commands into independent command segments.
    ///
    /// Splits on unquoted shell operators (`&&`, `||`, `;`, and `|`) while
    /// preserving quoted content.
    #[must_use]
    pub fn split_compound_command(command: &str) -> Vec<String> {
        let mut segments = Vec::new();
        let mut current = String::new();
        let mut quote: Option<char> = None;
        let mut escaped = false;
        let mut chars = command.chars().peekable();

        while let Some(ch) = chars.next() {
            if escaped {
                current.push(ch);
                escaped = false;
                continue;
            }

            if ch == '\\' {
                current.push(ch);
                escaped = true;
                continue;
            }

            if let Some(active_quote) = quote {
                current.push(ch);
                if ch == active_quote {
                    quote = None;
                }
                continue;
            }

            match ch {
                '"' | '\'' => {
                    quote = Some(ch);
                    current.push(ch);
                }
                ';' => {
                    Self::push_shell_segment(&mut segments, &mut current);
                }
                '|' => {
                    if chars.peek() == Some(&'|') {
                        let _ = chars.next();
                    }
                    Self::push_shell_segment(&mut segments, &mut current);
                }
                '&' => {
                    if chars.peek() == Some(&'&') {
                        let _ = chars.next();
                        Self::push_shell_segment(&mut segments, &mut current);
                    } else {
                        current.push(ch);
                    }
                }
                _ => current.push(ch),
            }
        }

        Self::push_shell_segment(&mut segments, &mut current);
        segments
    }

    /// Extract shell identifiers for each command in a compound command string.
    #[must_use]
    pub fn extract_shell_identifiers(command: &str) -> Vec<String> {
        Self::split_compound_command(command)
            .into_iter()
            .filter_map(|segment| {
                let identifier = Self::extract_shell_identifier(&segment);
                if identifier.is_empty() {
                    None
                } else {
                    Some(identifier)
                }
            })
            .collect()
    }

    #[must_use]
    fn evaluate_shell_identifier(&self, tool_identifier: &str) -> ToolApprovalDecision {
        if Self::matches_prefix_in_slice(&self.persistent_denylist, tool_identifier) {
            return ToolApprovalDecision::Deny;
        }

        if Self::matches_prefix_in_slice(&self.persistent_allowlist, tool_identifier) {
            return ToolApprovalDecision::Allow;
        }

        if self.yolo_mode {
            return ToolApprovalDecision::Allow;
        }

        if Self::matches_prefix_in_set(&self.session_allowlist, tool_identifier) {
            return ToolApprovalDecision::Allow;
        }

        ToolApprovalDecision::AskUser
    }

    /// Evaluate shell command approval by combining decisions for each segment.
    ///
    /// Most restrictive decision wins: `Deny > AskUser > Allow`.
    #[must_use]
    pub fn evaluate_compound_command(&self, command: &str) -> ToolApprovalDecision {
        let identifiers = Self::extract_shell_identifiers(command);
        if identifiers.is_empty() {
            return self.evaluate_shell_identifier(command.trim());
        }

        let mut has_ask_user = false;

        for identifier in identifiers {
            match self.evaluate_shell_identifier(&identifier) {
                ToolApprovalDecision::Deny => return ToolApprovalDecision::Deny,
                ToolApprovalDecision::AskUser => {
                    has_ask_user = true;
                }
                ToolApprovalDecision::Allow => {}
            }
        }

        if has_ask_user {
            ToolApprovalDecision::AskUser
        } else {
            ToolApprovalDecision::Allow
        }
    }

    fn push_shell_segment(segments: &mut Vec<String>, current: &mut String) {
        let segment = current.trim();
        if !segment.is_empty() {
            segments.push(segment.to_string());
        }
        current.clear();
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
            "Search",
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
mod tests;

