//! View components for the GPUI-based UI
//!
//! @plan PLAN-20250130-GPUIREDUX.P10
//! @requirement REQ-GPUI-003

pub mod chat_view;
pub mod main_panel;
pub mod history_view;
pub mod settings_view;
pub mod model_selector_view;
pub mod profile_editor_view;
pub mod mcp_add_view;
pub mod mcp_configure_view;

pub use chat_view::{ChatView, ChatState};
pub use main_panel::MainPanel;
pub use history_view::{HistoryView, HistoryState, ConversationItem};
pub use settings_view::{SettingsView, SettingsState, ProfileItem, McpItem, McpStatus};
pub use model_selector_view::{ModelSelectorView, ModelSelectorState, ModelInfo, ProviderInfo};
pub use profile_editor_view::{ProfileEditorView, ProfileEditorState, ProfileEditorData, AuthMethod, ApiType};
pub use mcp_add_view::{McpAddView, McpAddState, McpSearchResult, McpRegistry, SearchState};
pub use mcp_configure_view::{McpConfigureView, McpConfigureState, McpConfigureData, McpAuthMethod, OAuthStatus, ConfigField};
