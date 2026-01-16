//! MCP (Model Context Protocol) support module
pub mod types;
pub mod secrets;
pub mod manager;
pub mod runtime;
pub mod registry;
pub mod status;
pub mod oauth;
pub mod service;

pub use types::*;
pub use secrets::SecretsManager;
pub use manager::{McpManager, McpError, McpResult};
pub use runtime::{McpRuntime, McpTool, McpConnection};
pub use registry::{McpRegistry, McpRegistryServerWrapper, McpSearchResult, McpRegistrySource};
pub use status::{McpStatus, McpStatusManager};
pub use oauth::{OAuthManager, OAuthToken, OAuthConfig, OAuthFlowState, OAuthCallbackResult, SmitheryOAuthConfig, start_oauth_callback_server, generate_smithery_oauth_url};
pub use service::{McpService, ToolDefinition};
