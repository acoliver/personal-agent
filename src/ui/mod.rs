mod chat_view;
pub mod history_view;
mod mcp_add_view;
mod mcp_configure_view;
pub mod model_selector;
mod profile_editor;
mod profile_editor_demo;
pub mod settings_view;
mod simple_test;
mod theme;

pub use chat_view::ChatViewController;
pub use history_view::HistoryViewController;
pub use mcp_add_view::{McpAddViewController, SELECTED_MCP_CONFIG};
pub use mcp_configure_view::McpConfigureViewController;
pub use model_selector::ModelSelectorViewController;
pub use profile_editor_demo::ProfileEditorDemoViewController;
pub use settings_view::SettingsViewController;
pub use theme::{FlippedStackView, Theme};
