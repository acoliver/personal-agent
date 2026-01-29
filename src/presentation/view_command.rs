//! ViewCommand enum - commands from presenters to UI layer
//!
//! ViewCommands are emitted by presenters to instruct the UI layer to update.
//! This decouples presenters from any specific UI framework.
//!
//! @plan PLAN-20250125-REFACTOR.P10
//! @requirement REQ-025.6
//! @pseudocode presenters.md lines 510-541

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Command from presenter to UI layer
///
/// @plan PLAN-20250125-REFACTOR.P10
/// @requirement REQ-025.6
/// @pseudocode presenters.md lines 510-541
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ViewCommand {
    // ===== Chat Commands =====

    /// A new conversation was created
    ConversationCreated {
        id: Uuid,
        profile_id: Uuid,
    },

    /// A message was appended to the conversation
    MessageAppended {
        conversation_id: Uuid,
        role: MessageRole,
        content: String,
    },

    /// Show thinking indicator
    ShowThinking {
        conversation_id: Uuid,
    },

    /// Hide thinking indicator
    HideThinking {
        conversation_id: Uuid,
    },

    /// Append streaming text chunk
    AppendStream {
        conversation_id: Uuid,
        chunk: String,
    },

    /// Finalize streaming response
    FinalizeStream {
        conversation_id: Uuid,
        tokens: u64,
    },

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
    MessageSaved {
        conversation_id: Uuid,
    },

    /// Toggle thinking visibility
    ToggleThinkingVisibility,

    /// Conversation was renamed
    ConversationRenamed {
        id: Uuid,
        new_title: String,
    },

    /// Conversation was cleared
    ConversationCleared,

    /// History was updated
    HistoryUpdated {
        count: Option<usize>,
    },

    // ===== History Commands =====

    /// Conversation list was refreshed
    ConversationListRefreshed {
        conversations: Vec<ConversationSummary>,
    },

    /// Conversation was activated
    ConversationActivated {
        id: Uuid,
    },

    /// Conversation was deleted
    ConversationDeleted {
        id: Uuid,
    },

    /// Conversation title was updated
    ConversationTitleUpdated {
        id: Uuid,
        title: String,
    },

    // ===== Settings Commands =====

    /// Show settings view
    ShowSettings {
        profiles: Vec<ProfileSummary>,
    },

    /// Show notification message
    ShowNotification {
        message: String,
    },

    /// Profile was created
    ProfileCreated {
        id: Uuid,
        name: String,
    },

    /// Profile was updated
    ProfileUpdated {
        id: Uuid,
        name: String,
    },

    /// Profile was deleted
    ProfileDeleted {
        id: Uuid,
    },

    /// Default profile was changed
    DefaultProfileChanged {
        profile_id: Option<Uuid>,
    },

    /// Connection test started
    ProfileTestStarted {
        id: Uuid,
    },

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
        tool_count: usize,
    },

    /// MCP server failed to start
    McpServerFailed {
        id: Uuid,
        error: String,
    },

    /// MCP tools were updated
    McpToolsUpdated {
        tools: Vec<ToolInfo>,
    },

    /// MCP server status changed
    McpStatusChanged {
        id: Uuid,
        status: McpStatus,
    },

    /// MCP configuration was saved
    McpConfigSaved {
        id: Uuid,
    },

    /// MCP was deleted
    McpDeleted {
        id: Uuid,
    },

    // ===== Model Selector Commands =====

    /// Model search results updated
    ModelSearchResults {
        models: Vec<ModelInfo>,
    },

    /// Model selection changed
    ModelSelected {
        provider_id: String,
        model_id: String,
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
    NavigateTo {
        view: ViewId,
    },

    /// Navigate back
    NavigateBack,

    /// Show modal
    ShowModal {
        modal: ModalId,
    },

    /// Dismiss modal
    DismissModal,
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

/// Conversation summary for list display
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: Uuid,
    pub title: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub message_count: usize,
}

/// Profile summary for settings display
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub id: Uuid,
    pub name: String,
    pub provider_id: String,
    pub is_default: bool,
}

/// MCP tool information
///
/// @plan PLAN-20250125-REFACTOR.P10
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub mcp_id: Uuid,
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    McpAdd,
    McpConfigure,
    ModelSelector,
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
