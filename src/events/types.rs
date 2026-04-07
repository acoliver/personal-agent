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
    /// @plan PLAN-20260304-GPUIREMEDIATE.P05
    /// @requirement REQ-ARCH-003.6
    /// @pseudocode analysis/pseudocode/02-selection-loading-protocol.md:001-087
    SelectConversation { id: Uuid, selection_generation: u64 },

    /// User toggled thinking display
    ToggleThinking,

    /// User refreshed conversation list (e.g., after restore)
    RefreshConversations,

    /// User toggled emoji filter mode
    ToggleEmojiFilter,

    /// User requested conversation export with the currently selected format.
    SaveConversation,

    /// User requested error log export as plain text.
    SaveErrorLog,

    /// User selected an export format for conversation save.
    SelectConversationExportFormat {
        format: crate::models::ConversationExportFormat,
    },

    /// User changed the export directory path in settings.
    SetExportDirectory { path: String },

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
    SearchMcpRegistry {
        query: String,
        source: McpRegistrySource,
    },

    /// User selected MCP from search results
    SelectMcpFromRegistry { source: McpRegistrySource },

    /// User clicked configure MCP
    ConfigureMcp { id: Uuid },

    /// User saved MCP configuration (boxed to keep `UserEvent` size small).
    SaveMcpConfig { id: Uuid, config: Box<McpConfig> },

    /// User clicked delete MCP
    DeleteMcp { id: Uuid },

    /// User confirmed delete in dialog
    ConfirmDeleteMcp { id: Uuid },

    // ===== Conversation Actions =====
    /// User clicked delete conversation in history
    /// @plan PLAN-20250130-GPUIREDUX.P05
    DeleteConversation { id: Uuid },

    /// User confirmed delete conversation
    /// @plan PLAN-20250130-GPUIREDUX.P05
    ConfirmDeleteConversation { id: Uuid },

    /// User requested history refresh
    /// @plan PLAN-20250130-GPUIREDUX.P05
    RefreshHistory,

    /// User toggled between popup and popout window mode.
    ToggleWindowMode,

    /// User toggled the sidebar visibility in popout mode.
    ToggleSidebar,

    /// User typed a search query in the sidebar search box.
    SearchConversations { query: String },

    /// User requested a fresh profile snapshot for chat/settings dropdowns.
    RefreshProfiles,

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
    SelectModel {
        provider_id: String,
        model_id: String,
    },

    /// User clicked refresh models registry
    /// @plan PLAN-20250130-GPUIREDUX.P06
    RefreshModelsRegistry,

    // ===== Profile Editor Actions =====
    /// User clicked save in profile editor (without full profile data)
    /// @plan PLAN-20250130-GPUIREDUX.P08
    SaveProfileEditor,

    /// Store a new API key in the OS keychain.
    StoreApiKey { label: String, value: String },

    /// Delete an API key from the OS keychain.
    DeleteApiKey { label: String },

    /// Request the full list of stored API key labels (triggers `ApiKeysListed` command).
    RefreshApiKeys,

    // ===== MCP Add Actions =====
    /// User clicked Next in MCP Add view
    /// @plan PLAN-20250130-GPUIREDUX.P09
    McpAddNext { manual_entry: Option<String> },

    /// User selected a theme from the settings dropdown
    SelectTheme { slug: String },

    /// User changed the UI font size.
    SetFontSize { size: f32 },

    /// User changed the proportional UI font family (`None` = system default).
    SetUiFontFamily { name: Option<String> },

    /// User changed the monospace font family.
    SetMonoFontFamily { name: String },

    /// User toggled monospace ligatures.
    SetMonoLigatures { enabled: bool },

    // ===== Tool Approval Actions =====
    /// User requested a fresh tool approval policy snapshot.
    RefreshToolApprovalPolicy,

    /// User requested a fresh skills snapshot.
    RefreshSkills,

    /// User toggled an individual skill's enabled state.
    SetSkillEnabled { name: String, enabled: bool },

    /// User requested adding a watched skills directory.
    AddSkillsDirectory { path: String },

    /// User requested removing a watched skills directory.
    RemoveSkillsDirectory { path: String },

    /// User requested installing a skill from a direct SKILL.md URL.
    InstallSkillFromUrl { url: String },

    /// User responded to a tool approval request.
    ToolApprovalResponse {
        request_id: String,
        decision: ToolApprovalResponseAction,
    },

    /// User toggled YOLO mode for tool approvals.
    SetToolApprovalYoloMode { enabled: bool },

    /// User toggled automatic approval for read-only tools.
    SetToolApprovalAutoApproveReads { enabled: bool },

    /// User toggled automatic approval for skill activation.
    SetToolApprovalSkillsAutoApprove { enabled: bool },

    /// User selected MCP approval granularity.
    SetToolApprovalMcpApprovalMode { mode: crate::agent::McpApprovalMode },

    /// User added an allowlist prefix for persistent tool approvals.
    AddToolApprovalAllowlistPrefix { prefix: String },

    /// User removed an allowlist prefix for persistent tool approvals.
    RemoveToolApprovalAllowlistPrefix { prefix: String },

    /// User added a denylist prefix for persistent tool approvals.
    AddToolApprovalDenylistPrefix { prefix: String },

    /// User removed a denylist prefix for persistent tool approvals.
    RemoveToolApprovalDenylistPrefix { prefix: String },

    // ===== Database Backup Actions =====
    /// User requested a manual backup now
    TriggerBackupNow,

    /// User changed the backup directory path
    SetBackupDirectory { path: Option<String> },

    /// User requested to restore from a backup
    RestoreBackup { path: String },

    /// User requested to refresh the backup list
    RefreshBackupList,

    /// User toggled automatic backups
    SetBackupEnabled { enabled: bool },

    /// User changed backup interval (hours)
    SetBackupIntervalHours { hours: u32 },

    /// User changed max backup copies to retain
    SetBackupMaxCopies { copies: u32 },

    /// User requested to restore a database from a backup file (recovery flow)
    RestoreDatabaseBackup { backup_path: std::path::PathBuf },

    /// User requested to quit the application
    QuitApplication,

    // ===== Navigation =====
    /// User clicked to navigate to a view
    Navigate { to: ViewId },

    /// User selected profile from chat title bar
    SelectChatProfile { id: Uuid },

    /// User clicked back
    NavigateBack,
}

/// Actions a user can take on a tool approval request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ToolApprovalResponseAction {
    /// Approve this single invocation.
    ProceedOnce,
    /// Approve for the remainder of this session.
    ProceedSession,
    /// Permanently add to the allowlist.
    ProceedAlways,
    /// Deny this invocation.
    Denied,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ChatEvent {
    /// Stream has started
    StreamStarted {
        conversation_id: Uuid,
        message_id: Uuid,
        model_id: String,
    },

    /// Text content delta received
    TextDelta { conversation_id: Uuid, text: String },

    /// Thinking content delta received
    ThinkingDelta { conversation_id: Uuid, text: String },

    /// Tool call started
    ToolCallStarted {
        conversation_id: Uuid,
        tool_call_id: String,
        tool_name: String,
    },

    /// Tool call completed
    ToolCallCompleted {
        conversation_id: Uuid,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
    ValidationFailed { id: Uuid, errors: Vec<String> },
}

/// Conversation lifecycle events
///
/// @plan PLAN-20250125-REFACTOR.P04
/// @requirement REQ-019.2
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

/// Lightweight profile auth payload for GPUI save flow
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ModelProfileAuth {
    /// API key stored in OS keychain, referenced by label.
    Keychain { label: String },
    /// No authentication required (for local/offline models).
    None,
}

/// Lightweight profile parameters payload for GPUI save flow
///
/// @plan PLAN-20250125-REFACTOR.P04
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
pub struct ModelProfileParameters {
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub show_thinking: Option<bool>,
    pub enable_thinking: Option<bool>,
    pub thinking_budget: Option<u32>,
}

/// Placeholder for `ModelProfile`
///
/// @plan PLAN-20250125-REFACTOR.P04
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelProfile {
    pub id: Uuid,
    pub name: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub base_url: Option<String>,
    pub auth: Option<ModelProfileAuth>,
    pub parameters: Option<ModelProfileParameters>,
    pub system_prompt: Option<String>,
}

/// Rich MCP config re-export for save flow (replaces earlier lossy placeholder).
pub use crate::mcp::McpConfig;

/// Placeholder for `McpRegistrySource`
///
/// @plan PLAN-20250125-REFACTOR.P04
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct McpRegistrySource {
    pub name: String,
}

/// Placeholder for `HotkeyConfig`
///
/// @plan PLAN-20250125-REFACTOR.P04
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HotkeyConfig {
    pub key: String,
}
