//! Event Type Definitions
//!
//! Defines all event type enums used throughout the application.
//!
//! @plan PLAN-20250125-REFACTOR.P04
//! @requirement REQ-019.2
//! @pseudocode event-bus.md lines 80-123

use serde::Serialize;
use uuid::Uuid;

/// Top-level event enum - all events in the system
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.2
/// @pseudocode event-bus.md lines 80-84
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum AppEvent {
    /// User-initiated actions from UI
    User(UserEvent),

    /// Chat and streaming events
    Chat(ChatEvent),

    /// MCP server lifecycle events
    Mcp(McpEvent),

    /// Profile and settings events
    Profile(ProfileEvent),

    /// Conversation events
    Conversation(ConversationEvent),

    /// Navigation events
    Navigation(NavigationEvent),

    /// System-level events
    System(SystemEvent),
}

/// User-initiated actions from UI
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.2
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum UserEvent {
    // ===== Chat Actions =====
    /// User clicked send or pressed Enter
    SendMessage { text: String },

    /// User clicked stop during streaming
    StopStreaming,

    /// User clicked new conversation
    NewConversation,

    /// User selected a conversation from history
    SelectConversation { id: Uuid },

    /// User toggled thinking display
    ToggleThinking,

    /// User started renaming conversation
    StartRenameConversation { id: Uuid },

    /// User confirmed rename
    ConfirmRenameConversation { id: Uuid, title: String },

    /// User cancelled rename
    CancelRenameConversation,

    // ===== Profile Actions =====
    /// User selected a profile as default
    SelectProfile { id: Uuid },

    /// User clicked create new profile
    CreateProfile,

    /// User clicked edit profile
    EditProfile { id: Uuid },

    /// User clicked save on profile editor
    SaveProfile { profile: ModelProfile },

    /// User clicked delete profile
    DeleteProfile { id: Uuid },

    /// User confirmed delete in dialog
    ConfirmDeleteProfile { id: Uuid },

    /// User clicked test connection
    TestProfileConnection { id: Uuid },

    // ===== MCP Actions =====
    /// User toggled MCP enabled/disabled
    ToggleMcp { id: Uuid, enabled: bool },

    /// User clicked add MCP
    AddMcp,

    /// User searched MCP registry
    SearchMcpRegistry { query: String, source: McpRegistrySource },

    /// User selected MCP from search results
    SelectMcpFromRegistry { source: McpRegistrySource },

    /// User clicked configure MCP
    ConfigureMcp { id: Uuid },

    /// User saved MCP configuration
    SaveMcpConfig { id: Uuid, config: McpConfig },

    /// User clicked delete MCP
    DeleteMcp { id: Uuid },

    /// User confirmed delete in dialog
    ConfirmDeleteMcp { id: Uuid },

    /// User initiated OAuth flow
    StartMcpOAuth { id: Uuid, provider: String },

    // ===== Model Selector Actions =====
    /// User opened model selector
    OpenModelSelector,

    /// User searched models
    SearchModels { query: String },

    /// User selected provider filter
    FilterModelsByProvider { provider_id: Option<String> },

    /// User selected a model
    SelectModel { provider_id: String, model_id: String },

    // ===== Navigation =====
    /// User clicked to navigate to a view
    Navigate { to: ViewId },

    /// User clicked back
    NavigateBack,
}

/// View identifiers for navigation
///
/// @plan PLAN-20250125-REFACTOR.P04
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ViewId {
    Chat,
    History,
    Settings,
    ProfileEditor { id: Option<Uuid> },
    McpAdd,
    McpConfigure { id: Uuid },
    ModelSelector,
}

/// Chat and streaming events
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.2
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ChatEvent {
    /// Stream has started
    StreamStarted {
        conversation_id: Uuid,
        message_id: Uuid,
        model_id: String,
    },

    /// Text content delta received
    TextDelta { text: String },

    /// Thinking content delta received
    ThinkingDelta { text: String },

    /// Tool call started
    ToolCallStarted {
        tool_call_id: String,
        tool_name: String,
    },

    /// Tool call completed
    ToolCallCompleted {
        tool_call_id: String,
        tool_name: String,
        success: bool,
        result: String,
        duration_ms: u64,
    },

    /// Stream completed successfully
    StreamCompleted {
        conversation_id: Uuid,
        message_id: Uuid,
        total_tokens: Option<u32>,
    },

    /// Stream was cancelled by user
    StreamCancelled {
        conversation_id: Uuid,
        message_id: Uuid,
        partial_content: String,
    },

    /// Stream failed with error
    StreamError {
        conversation_id: Uuid,
        error: String,
        recoverable: bool,
    },

    /// Message was saved to conversation
    MessageSaved {
        conversation_id: Uuid,
        message_id: Uuid,
    },
}

/// MCP server lifecycle events
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.2
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum McpEvent {
    /// MCP server is starting
    Starting { id: Uuid, name: String },

    /// MCP server started successfully
    Started {
        id: Uuid,
        name: String,
        tools: Vec<String>,
        tool_count: usize,
    },

    /// MCP server failed to start
    StartFailed {
        id: Uuid,
        name: String,
        error: String,
    },

    /// MCP server stopped
    Stopped { id: Uuid, name: String },

    /// MCP server health check failed
    Unhealthy {
        id: Uuid,
        name: String,
        error: String,
    },

    /// MCP server recovered from unhealthy state
    Recovered { id: Uuid, name: String },

    /// MCP server is restarting
    Restarting { id: Uuid, name: String },

    /// MCP tool was called
    ToolCalled {
        mcp_id: Uuid,
        tool_name: String,
        tool_call_id: String,
    },

    /// MCP tool call completed
    ToolCompleted {
        mcp_id: Uuid,
        tool_name: String,
        tool_call_id: String,
        success: bool,
        duration_ms: u64,
    },

    /// MCP configuration was saved
    ConfigSaved { id: Uuid },

    /// MCP was deleted
    Deleted { id: Uuid, name: String },
}

/// Profile and settings events
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.2
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ProfileEvent {
    /// Profile was created
    Created { id: Uuid, name: String },

    /// Profile was updated
    Updated { id: Uuid, name: String },

    /// Profile was deleted
    Deleted { id: Uuid, name: String },

    /// Default profile was changed
    DefaultChanged { profile_id: Option<Uuid> },

    /// Profile connection test started
    TestStarted { id: Uuid },

    /// Profile connection test completed
    TestCompleted {
        id: Uuid,
        success: bool,
        response_time_ms: Option<u64>,
        error: Option<String>,
    },

    /// Profile validation failed
    ValidationFailed {
        id: Uuid,
        errors: Vec<String>,
    },
}

/// Conversation lifecycle events
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.2
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ConversationEvent {
    /// Conversation was created
    Created { id: Uuid, title: String },

    /// Conversation was loaded
    Loaded { id: Uuid },

    /// Conversation title was updated
    TitleUpdated { id: Uuid, title: String },

    /// Conversation was deleted
    Deleted { id: Uuid },

    /// Conversation was set as active
    Activated { id: Uuid },

    /// Active conversation was cleared
    Deactivated,

    /// Conversation list was refreshed
    ListRefreshed { count: usize },
}

/// Navigation events
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.2
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum NavigationEvent {
    /// Navigation to view started
    Navigating { from: ViewId, to: ViewId },

    /// Navigation completed
    Navigated { view: ViewId },

    /// Navigation was cancelled
    Cancelled { reason: String },

    /// Modal was presented
    ModalPresented { view: ViewId },

    /// Modal was dismissed
    ModalDismissed { view: ViewId },
}

/// System-level events
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.2
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum SystemEvent {
    /// Application launched
    AppLaunched,

    /// Application will terminate
    AppWillTerminate,

    /// Application became active (foreground)
    AppBecameActive,

    /// Application resigned active (background)
    AppResignedActive,

    /// Global hotkey was pressed
    HotkeyPressed,

    /// Hotkey configuration was changed
    HotkeyChanged { hotkey: HotkeyConfig },

    /// Popover was shown
    PopoverShown,

    /// Popover was hidden
    PopoverHidden,

    /// Unhandled error occurred
    Error {
        source: String,
        error: String,
        context: Option<String>,
    },

    /// Config was loaded
    ConfigLoaded,

    /// Config was saved
    ConfigSaved,

    /// Models registry was refreshed
    ModelsRegistryRefreshed {
        provider_count: usize,
        model_count: usize,
    },

    /// Models registry refresh failed
    ModelsRegistryRefreshFailed { error: String },
}

// Placeholder types for event variants
// These will be replaced with actual types in later phases

/// Placeholder for ModelProfile
///
/// @plan PLAN-20250125-REFACTOR.P04
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelProfile {
    pub id: Uuid,
    pub name: String,
}

/// Placeholder for McpConfig
///
/// @plan PLAN-20250125-REFACTOR.P04
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct McpConfig {
    pub id: Uuid,
    pub name: String,
}

/// Placeholder for McpRegistrySource
///
/// @plan PLAN-20250125-REFACTOR.P04
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct McpRegistrySource {
    pub name: String,
}

/// Placeholder for HotkeyConfig
///
/// @plan PLAN-20250125-REFACTOR.P04
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HotkeyConfig {
    pub key: String,
}
