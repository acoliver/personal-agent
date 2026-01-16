use personal_agent::config::Config;
use personal_agent::mcp::{McpRuntime, SecretsManager};

#[tokio::main]
async fn main() {
    // Load config
    let config_path = Config::default_path().expect("config path");
    let config = Config::load(config_path).expect("load config");
    
    println!("Found {} MCPs in config", config.mcps.len());
    for mcp in &config.mcps {
        println!("  - {} (enabled: {}, transport: {:?})", mcp.name, mcp.enabled, mcp.transport);
        println!("    identifier: {}", mcp.package.identifier);
    }
    
    let enabled = config.get_enabled_mcps();
    println!("\nEnabled MCPs: {}", enabled.len());
    
    // Try to start
    let secrets_path = dirs::data_local_dir()
        .expect("data dir")
        .join("PersonalAgent")
        .join("mcp_secrets");
    let secrets = SecretsManager::new(secrets_path);
    let mut runtime = McpRuntime::new(secrets);
    
    println!("\nStarting MCPs...");
    let results = runtime.start_all(&config).await;
    
    for (id, result) in results {
        match result {
            Ok(()) => println!("  Started: {}", id),
            Err(e) => println!("  Failed {}: {}", id, e),
        }
    }
    
    println!("\nActive MCPs: {}", runtime.active_count());
    println!("Tools available: {}", runtime.get_all_tools().len());
    
    for tool in runtime.get_all_tools() {
        println!("  - {}: {}", tool.name, tool.description);
    }
}
