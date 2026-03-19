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
    ShowThinking { conversation_id: Uuid },

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

    /// Show notification message
    ShowNotification { message: String },

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
    McpServerStarted { id: Uuid, tool_count: usize },

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
