//! View components for the GPUI-based UI
//!
//! @plan PLAN-20250130-GPUIREDUX.P10
//! @requirement REQ-GPUI-003

pub mod api_key_manager_view;
pub mod chat_view;
pub mod error_log_view;
pub mod history_view;
pub mod main_panel;
pub mod mcp_add_view;
pub mod mcp_configure_view;
pub mod model_selector_view;
pub mod profile_editor_view;
pub mod settings_view;

pub use api_key_manager_view::ApiKeyManagerView;
pub use chat_view::{ChatState, ChatView};
pub use error_log_view::ErrorLogView;
pub use history_view::{ConversationItem, HistoryState, HistoryView};
pub use main_panel::MainPanel;
pub use mcp_add_view::{McpAddState, McpAddView, McpRegistry, McpSearchResult, SearchState};
pub use mcp_configure_view::{
    ConfigField, McpAuthMethod, McpConfigureData, McpConfigureState, McpConfigureView, OAuthStatus,
};
pub use model_selector_view::{ModelInfo, ModelSelectorState, ModelSelectorView, ProviderInfo};
pub use profile_editor_view::{
    ApiType, AuthMethod, ProfileEditorData, ProfileEditorState, ProfileEditorView,
};
pub use settings_view::{
    McpItem, McpStatus, ProfileItem, SettingsCategory, SettingsState, SettingsView, ThemeOption,
};
