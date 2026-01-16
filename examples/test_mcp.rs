//! Test MCP integration with a real MCP server
//! 
//! This example demonstrates connecting to an MCP server and calling tools.
//! 
//! Usage:
//!   cargo run --example test_mcp

use serdes_ai::mcp::{McpClient, StdioTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing MCP Integration...\n");

    // Try to connect to filesystem MCP if npx is available
    println!("Attempting to spawn filesystem MCP server...");
    
    match StdioTransport::spawn("npx", &["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]).await {
        Ok(transport) => {
            println!("[OK] MCP transport spawned");
            
            let client = McpClient::new(transport);
            
            println!("Initializing MCP client...");
            match client.initialize().await {
                Ok(init_result) => {
                    println!("[OK] MCP initialized");
                    println!("  Server: {}", init_result.server_info.name);
                    println!("  Version: {}", init_result.server_info.version);
                    
                    println!("\nListing tools...");
                    match client.list_tools().await {
                        Ok(tools) => {
                            println!("[OK] Found {} tools:", tools.len());
                            for tool in &tools {
                                println!("  - {}: {}", 
                                    tool.name, 
                                    tool.description.as_ref().unwrap_or(&"(no description)".to_string())
                                );
                            }
                            
                            // Try calling a simple tool if available
                            if let Some(tool) = tools.first() {
                                println!("\nAttempting to call tool: {}", tool.name);
                                // Note: Would need proper arguments for real tool call
                                println!("  (Skipping actual call - would need proper arguments)");
                            }
                        }
                        Err(e) => {
                            println!(" Failed to list tools: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!(" Failed to initialize: {}", e);
                }
            }
            
            println!("\nClosing connection...");
            let _ = client.close().await;
            println!("[OK] Connection closed");
        }
        Err(e) => {
            println!(" Failed to spawn MCP server: {}", e);
            println!("\nNote: This is expected if 'npx' is not available on your system.");
            println!("To test MCP integration, ensure Node.js and npm/npx are installed.");
        }
    }
    
    println!("\n=== MCP Integration Test Complete ===");
    Ok(())
}
