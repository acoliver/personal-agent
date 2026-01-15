//! MCP (Model Context Protocol) support module
pub mod types;
pub mod secrets;
pub mod manager;
pub mod runtime;
pub mod registry;

pub use types::*;
pub use secrets::SecretsManager;
pub use manager::{McpManager, McpError, McpResult};
pub use runtime::{McpRuntime, McpTool, McpConnection};
pub use registry::{McpRegistry, McpRegistryServerWrapper, McpSearchResult, McpRegistrySource};
