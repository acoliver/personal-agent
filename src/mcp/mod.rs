//! MCP (Model Context Protocol) support module
pub mod manager;
pub mod oauth;
pub mod registry;
pub mod runtime;
pub mod secrets;
pub mod service;
pub mod status;
pub mod toolset;
pub mod types;

pub use manager::{McpError, McpManager, McpResult};
pub use oauth::{
    generate_smithery_oauth_url, start_oauth_callback_server, OAuthCallbackResult, OAuthConfig,
    OAuthFlowState, OAuthManager, OAuthToken, SmitheryOAuthConfig,
};
pub use registry::{
    McpRegistry, McpRegistryRemote, McpRegistryServer, McpRegistryServerWrapper,
    McpRegistrySource, McpSearchResult,
};
pub use runtime::{McpConnection, McpRuntime, McpTool};
pub use secrets::SecretsManager;
pub use service::{McpService, ToolDefinition};
pub use status::{
    aggregate_mcp_status, get_config_status, AggregateStatus, McpStatus, McpStatusManager,
};
pub use toolset::{
    build_command, build_env_for_config, build_headers_for_config, create_toolset_from_config,
};
pub use types::*;
