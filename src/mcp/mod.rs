//! MCP (Model Context Protocol) support module
pub mod types;
pub mod secrets;
pub mod manager;

pub use types::*;
pub use secrets::SecretsManager;
pub use manager::{McpManager, McpError, McpResult};
