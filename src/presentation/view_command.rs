//! `ViewCommand` enum - commands from presenters to UI layer
//!
//! `ViewCommands` are emitted by presenters to instruct the UI layer to update.
//! This decouples presenters from any specific UI framework.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.6
//! @pseudocode presenters.md lines 510-541

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::McpApprovalMode;
use crate::models::ConversationExportFormat;

/// Application window mode — popup (tray-anchored) or popout (free-floating).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AppMode {
    /// Tray-anchored popup (default at startup).
    #[default]
    Popup,
    /// Free-floating resizable window.
    Popout,
}

/// Command from presenter to UI layer
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.6
/// @pseudocode presenters.md lines 510-541
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ViewCommand {
    // ===== Chat Commands =====
    /// A new conversation was created
    ConversationCreated { id: Uuid, profile_id: Uuid },

    /// A message was appended to the conversation
    MessageAppended {
        conversation_id: Uuid,
        role: MessageRole,
        content: String,
        model_id: Option<String>,
    },

    /// Replace the visible transcript for a conversation with a full replay payload.
    ConversationMessagesLoaded {
        conversation_id: Uuid,
        selection_generation: u64,
        messages: Vec<ConversationMessagePayload>,
    },

    /// Explicit load failure for the currently selected conversation generation.
    ConversationLoadFailed {
        conversation_id: Uuid,
        selection_generation: u64,
        message: String,
    },

    /// Show thinking indicator
    ShowThinking {
        conversation_id: Uuid,
        model_id: String,
    },

    /// Hide thinking indicator
    HideThinking { conversation_id: Uuid },

    /// Append streaming text chunk
    AppendStream {
        conversation_id: Uuid,
        chunk: String,
    },

    /// Finalize streaming response
    FinalizeStream { conversation_id: Uuid, tokens: u64 },

    /// Streaming was cancelled
    StreamCancelled {
        conversation_id: Uuid,
        partial_content: String,
    },

    /// Streaming failed with error
    StreamError {
        conversation_id: Uuid,
        error: String,
        recoverable: bool,
    },

    /// Append thinking content
    AppendThinking {
        conversation_id: Uuid,
        content: String,
    },

    /// Show tool call UI
    ShowToolCall {
        conversation_id: Uuid,
        tool_name: String,
        status: String,
    },

    /// Update tool call status
    UpdateToolCall {
        conversation_id: Uuid,
        tool_name: String,
        status: String,
        result: Option<String>,
        duration: Option<u64>,
    },

    /// Message was saved
    MessageSaved { conversation_id: Uuid },

    /// Toggle thinking visibility
    ToggleThinkingVisibility,

    /// Toggle between popup and popout window modes.
    ToggleWindowMode,

    /// Conversation search results returned from backend.
    ConversationSearchResults {
        results: Vec<ConversationSearchResult>,
    },

    /// Conversation was renamed
    ConversationRenamed { id: Uuid, new_title: String },

    /// Conversation was cleared
    ConversationCleared,

    /// History was updated
    HistoryUpdated { count: Option<usize> },

    // ===== History Commands =====
    /// Conversation list was refreshed
    ConversationListRefreshed {
        conversations: Vec<ConversationSummary>,
    },

    /// Conversation was activated
    ConversationActivated { id: Uuid, selection_generation: u64 },

    /// Conversation was deleted
    ConversationDeleted { id: Uuid },

    /// Conversation title was updated
    ConversationTitleUpdated { id: Uuid, title: String },

    // ===== Settings Commands =====
    /// Show settings view
    ShowSettings {
        profiles: Vec<ProfileSummary>,
        selected_profile_id: Option<Uuid>,
    },

    /// Provide theme options and the currently-selected slug to the settings view.
    ShowSettingsTheme {
        options: Vec<ThemeSummary>,
        selected_slug: String,
    },

    /// Provide the current font settings snapshot to the settings view.
    ShowFontSettings {
        size: f32,
        ui_family: Option<String>,
        mono_family: String,
        ligatures: bool,
    },

    /// Show notification message
    ShowNotification { message: String },

    /// Update chat export format controls in the chat view.
    ShowConversationExportFormat { format: ConversationExportFormat },

    /// Provide the persisted export directory to the settings view.
    ExportDirectoryLoaded { path: String },

    /// Conversation was successfully exported to disk.
    ExportCompleted { path: String, format_label: String },

    /// Error log was successfully exported to disk.
    ErrorLogExportCompleted { path: String },

    /// Profile was created
    ProfileCreated { id: Uuid, name: String },

    /// Profile was updated
    ProfileUpdated { id: Uuid, name: String },

    /// Profile was deleted
    ProfileDeleted { id: Uuid },

    /// Full list of stored API key entries (response to `RefreshApiKeys`).
    ApiKeysListed { keys: Vec<ApiKeyInfo> },

    /// An API key was stored successfully.
    ApiKeyStored { label: String },

    /// An API key was deleted successfully.
    ApiKeyDeleted { label: String },

    /// Default profile was changed
    DefaultProfileChanged { profile_id: Option<Uuid> },

    /// Chat profile list updated
    ChatProfilesUpdated {
        profiles: Vec<ProfileSummary>,
        selected_profile_id: Option<Uuid>,
    },

    /// Connection test started
    ProfileTestStarted { id: Uuid },

    /// Connection test completed
    ProfileTestCompleted {
        id: Uuid,
        success: bool,
        response_time_ms: Option<u64>,
        error: Option<String>,
    },

    // ===== MCP Commands =====
    /// MCP server started successfully
    McpServerStarted {
        id: Uuid,
        name: Option<String>,
        tool_count: usize,
        enabled: Option<bool>,
    },

    /// MCP server failed to start
    McpServerFailed { id: Uuid, error: String },

    /// MCP tools were updated
    McpToolsUpdated { tools: Vec<ToolInfo> },

    /// MCP server status changed
    McpStatusChanged { id: Uuid, status: McpStatus },

    /// MCP configuration was saved
    McpConfigSaved { id: Uuid, name: Option<String> },

    /// MCP was deleted
    McpDeleted { id: Uuid },

    /// MCP registry search results were updated
    McpRegistrySearchResults { results: Vec<McpRegistryResult> },

    /// MCP configure draft payload from MCP add flow
    McpConfigureDraftLoaded {
        id: String,
        name: String,
        package: String,
        package_type: crate::mcp::McpPackageType,
        runtime_hint: Option<String>,
        env_var_name: String,
        command: String,
        args: Vec<String>,
        env: Option<Vec<(String, String)>>,
        /// Remote URL for HTTP/SSE transport MCPs (None for stdio-only).
        url: Option<String>,
    },

    // ===== Model Selector Commands =====
    /// Model search results updated
    ModelSearchResults { models: Vec<ModelInfo> },

    /// Model selection changed
    ModelSelected {
        provider_id: String,
        model_id: String,
        provider_api_url: Option<String>,
        context_length: Option<u32>,
    },

    /// Prefill profile editor with existing profile data for edit flow
    ProfileEditorLoad {
        id: Uuid,
        name: String,
        provider_id: String,
        model_id: String,
        base_url: String,
        /// Keychain label for the API key (empty string = none set).
        api_key_label: String,
        temperature: f64,
        max_tokens: u32,
        context_limit: Option<u32>,
        show_thinking: bool,
        enable_thinking: bool,
        thinking_budget: Option<u32>,
        system_prompt: String,
    },

    // ===== Tool Approval Commands =====
    /// Display an inline approval bubble for a tool call.
    ToolApprovalRequest {
        request_id: String,
        context: ToolApprovalContext,
    },

    /// Update an existing approval bubble to reflect the user's decision.
    ToolApprovalResolved { request_id: String, approved: bool },

    /// Inform the UI whether YOLO mode is currently active.
    YoloModeChanged { active: bool },

    /// Update settings surfaces with the current persisted tool approval policy.
    ToolApprovalPolicyUpdated {
        yolo_mode: bool,
        auto_approve_reads: bool,
        mcp_approval_mode: McpApprovalMode,
        persistent_allowlist: Vec<String>,
        persistent_denylist: Vec<String>,
    },

    /// Request presenters/views to refresh tool approval settings from persistence.
    RefreshToolApprovalSettings,

    // ===== Database Backup Commands =====
    /// Backup settings and list loaded for settings view
    BackupSettingsLoaded {
        settings: crate::backup::DatabaseBackupSettings,
        backups: Vec<crate::backup::BackupInfo>,
        last_backup_time: Option<chrono::DateTime<chrono::Utc>>,
    },

    /// Backup operation completed
    BackupCompleted { result: crate::backup::BackupResult },

    /// Backup list refreshed
    BackupListRefreshed {
        backups: Vec<crate::backup::BackupInfo>,
    },

    /// Restore operation completed
    RestoreCompleted {
        result: crate::backup::RestoreResult,
    },

    // ===== Error Commands =====
    /// Show error to user
    ShowError {
        title: String,
        message: String,
        severity: ErrorSeverity,
    },

    /// Clear error display
    ClearError,

    // ===== Navigation Commands =====
    /// Navigate to view
    NavigateTo { view: ViewId },

    /// Navigate back
    NavigateBack,

    /// Show modal
    ShowModal { modal: ModalId },

    /// Dismiss modal
    DismissModal,
}

/// Tool category for structured approval context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    /// File edit operation (`EditFile`)
    FileEdit,
    /// File write operation (`WriteFile`)
    FileWrite,
    /// File read operation (`ReadFile`)
    FileRead,
    /// Search operation
    Search,
    /// Shell command execution
    Shell,
    /// MCP tool execution
    Mcp,
}

/// Structured context for tool approval requests.
///
/// Replaces flat `tool_argument` strings with rich, tool-specific metadata
/// for consistent display across all tool types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolApprovalContext {
    /// The tool name (e.g., `EditFile`, `ShellExec`)
    pub tool_name: String,
    /// The category of tool for grouping and icon selection
    pub category: ToolCategory,
    /// The primary target (file path, command, search pattern)
    pub primary_target: String,
    /// Additional key-value details (truncated for display)
    pub details: Vec<(String, String)>,
    /// For MCP tools, the server name
    pub server_name: Option<String>,
}

impl ToolApprovalContext {
    /// Create a new tool approval context.
    #[must_use]
    pub fn new(
        tool_name: impl Into<String>,
        category: ToolCategory,
        primary_target: impl Into<String>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            category,
            primary_target: primary_target.into(),
            details: Vec::new(),
            server_name: None,
        }
    }

    /// Add a detail key-value pair.
    #[must_use]
    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.push((key.into(), value.into()));
        self
    }

    /// Set the server name (for MCP tools).
    #[must_use]
    pub fn with_server_name(mut self, server_name: impl Into<String>) -> Self {
        self.server_name = Some(server_name.into());
        self
    }
}

/// Message role for display
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationMessagePayload {
    pub role: MessageRole,
    pub content: String,
    pub thinking_content: Option<String>,
    pub timestamp: Option<u64>,
    pub model_id: Option<String>,
}

/// Conversation summary for list display
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub title: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
    pub preview: Option<String>,
}

/// A single search result for the sidebar conversation search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationSearchResult {
    pub id: Uuid,
    pub title: String,
    pub is_title_match: bool,
    pub match_context: String,
    pub message_count: usize,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Summary of a stored API key for the key manager UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyInfo {
    /// The keychain label (e.g. "anthropic").
    pub label: String,
    /// Masked display of the secret value (e.g. "sk-a••••••••b3Xz").
    pub masked_value: String,
    /// Names of profiles referencing this label.
    pub used_by: Vec<String>,
}

/// Profile summary for settings display
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub id: Uuid,
    pub name: String,
    pub provider_id: String,
    pub model_id: String,
    pub is_default: bool,
}

/// Theme summary for the settings theme dropdown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeSummary {
    /// Human-readable display name (e.g. "Green Screen").
    pub name: String,
    /// URL-safe slug used as the identifier (e.g. "green-screen").
    pub slug: String,
}

/// MCP tool information
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub mcp_id: Uuid,
}

/// MCP registry search result information for MCP add flow
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpRegistryResult {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<Vec<(String, String)>>,
    pub package_type: Option<crate::mcp::McpPackageType>,
    pub runtime_hint: Option<String>,
    /// Remote URL for HTTP/SSE transport MCPs (None for stdio-only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// MCP server status
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpStatus {
    Starting,
    Running,
    Stopped,
    Failed,
    Unhealthy,
}

/// Model information for selector
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub provider_id: String,
    pub model_id: String,
    pub name: String,
    pub context_length: Option<u32>,
}

/// View identifier
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewId {
    Chat,
    History,
    Settings,
    ProfileEditor,
    ApiKeyManager,
    McpAdd,
    McpConfigure,
    ModelSelector,
    ErrorLog,
}

/// Modal identifier
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModalId {
    ConfirmDeleteConversation,
    ConfirmDeleteProfile,
    ConfirmDeleteMcp,
}

/// Error severity level
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_category_equality() {
        assert_eq!(ToolCategory::FileEdit, ToolCategory::FileEdit);
        assert_ne!(ToolCategory::FileEdit, ToolCategory::FileWrite);
        assert_eq!(ToolCategory::Shell, ToolCategory::Shell);
        assert_eq!(ToolCategory::Mcp, ToolCategory::Mcp);
    }

    #[test]
    fn tool_approval_context_new_sets_fields() {
        let ctx = ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, "/tmp/test.rs");
        assert_eq!(ctx.tool_name, "EditFile");
        assert_eq!(ctx.category, ToolCategory::FileEdit);
        assert_eq!(ctx.primary_target, "/tmp/test.rs");
        assert!(ctx.details.is_empty());
        assert!(ctx.server_name.is_none());
    }

    #[test]
    fn tool_approval_context_with_detail_adds_pair() {
        let ctx = ToolApprovalContext::new("ShellExec", ToolCategory::Shell, "git status")
            .with_detail("working_dir", "/home/user");
        assert_eq!(ctx.details.len(), 1);
        assert_eq!(ctx.details[0].0, "working_dir");
        assert_eq!(ctx.details[0].1, "/home/user");
    }

    #[test]
    fn tool_approval_context_with_multiple_details() {
        let ctx = ToolApprovalContext::new("Search", ToolCategory::Search, "/src")
            .with_detail("pattern", "fn main")
            .with_detail("include", "*.rs");
        assert_eq!(ctx.details.len(), 2);
        assert_eq!(
            ctx.details[0],
            ("pattern".to_string(), "fn main".to_string())
        );
        assert_eq!(ctx.details[1], ("include".to_string(), "*.rs".to_string()));
    }

    #[test]
    fn tool_approval_context_with_server_name() {
        let ctx = ToolApprovalContext::new("mcp-tool", ToolCategory::Mcp, "query")
            .with_server_name("test-server");
        assert_eq!(ctx.server_name, Some("test-server".to_string()));
    }

    #[test]
    fn tool_approval_context_builder_chaining() {
        let ctx = ToolApprovalContext::new("EditFile", ToolCategory::FileEdit, "/tmp/file.txt")
            .with_detail("line_range", "10-20")
            .with_detail("encoding", "utf-8")
            .with_server_name("local");
        assert_eq!(ctx.tool_name, "EditFile");
        assert_eq!(ctx.category, ToolCategory::FileEdit);
        assert_eq!(ctx.primary_target, "/tmp/file.txt");
        assert_eq!(ctx.details.len(), 2);
        assert_eq!(ctx.server_name, Some("local".to_string()));
    }

    #[test]
    fn message_role_equality() {
        assert_eq!(MessageRole::User, MessageRole::User);
        assert_eq!(MessageRole::Assistant, MessageRole::Assistant);
        assert_ne!(MessageRole::User, MessageRole::Assistant);
    }

    #[test]
    fn error_severity_equality() {
        assert_eq!(ErrorSeverity::Info, ErrorSeverity::Info);
        assert_eq!(ErrorSeverity::Critical, ErrorSeverity::Critical);
        assert_ne!(ErrorSeverity::Warning, ErrorSeverity::Error);
    }

    #[test]
    fn view_id_equality() {
        assert_eq!(ViewId::Chat, ViewId::Chat);
        assert_eq!(ViewId::Settings, ViewId::Settings);
        assert_ne!(ViewId::Chat, ViewId::History);
    }

    #[test]
    fn modal_id_equality() {
        assert_eq!(
            ModalId::ConfirmDeleteConversation,
            ModalId::ConfirmDeleteConversation
        );
        assert_ne!(
            ModalId::ConfirmDeleteConversation,
            ModalId::ConfirmDeleteProfile
        );
    }
}
