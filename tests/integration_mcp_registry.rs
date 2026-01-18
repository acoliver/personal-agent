//! Integration tests for the MCP registry module

use personal_agent::mcp::{McpRegistry, McpRegistrySource};

#[tokio::test]
#[ignore] // Requires network access
async fn test_fetch_official_registry() {
    let registry = McpRegistry::new();

    let servers = registry.fetch_official().await;
    assert!(
        servers.is_ok(),
        "Should fetch official registry successfully"
    );

    let servers = servers.unwrap();
    assert!(
        !servers.is_empty(),
        "Registry should have at least one server"
    );

    // Check first server has expected fields
    if let Some(first) = servers.first() {
        assert!(!first.server.name.is_empty(), "Server should have a name");
        assert!(
            !first.server.description.is_empty(),
            "Server should have a description"
        );
        assert!(
            !first.server.version.is_empty(),
            "Server should have a version"
        );

        println!(
            "Sample server: {} v{}",
            first.server.name, first.server.version
        );
        println!("  Description: {}", first.server.description);
    }
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_search_registry() {
    let registry = McpRegistry::new();

    // Search for common terms
    let result = registry.search("github").await;
    assert!(result.is_ok(), "Search should succeed");

    let search_result = result.unwrap();
    assert_eq!(search_result.source, McpRegistrySource::Official);

    println!(
        "Found {} servers matching 'github'",
        search_result.entries.len()
    );

    // Should find at least some results
    if !search_result.entries.is_empty() {
        let first = &search_result.entries[0];
        println!(
            "First result: {} - {}",
            first.server.name, first.server.description
        );
    }
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_entry_to_config_conversion() {
    let registry = McpRegistry::new();

    let servers = registry.fetch_official().await.unwrap();

    // Try to convert the first server with packages
    for server_wrapper in servers.iter().take(10) {
        if !server_wrapper.server.packages.is_empty() {
            let config_result = McpRegistry::entry_to_config(server_wrapper);

            if let Ok(config) = config_result {
                println!("Converted server: {}", config.name);
                println!("  Package: {}", config.package.identifier);
                println!("  Transport: {:?}", config.transport);
                println!("  Auth type: {:?}", config.auth_type);
                println!("  Env vars: {}", config.env_vars.len());

                // Config should have valid values
                assert!(!config.name.is_empty());
                assert!(!config.package.identifier.is_empty());

                break;
            }
        }
    }
}

#[tokio::test]
async fn test_registry_default() {
    let _registry1 = McpRegistry::new();
    let _registry2 = McpRegistry::default();

    // Both should be equivalent (we can't directly compare them, so just ensure both construct)
    assert!(true, "Both constructors should work");
}

#[tokio::test]
#[ignore] // Requires network access
async fn test_empty_search() {
    let registry = McpRegistry::new();

    // Search for something unlikely to match
    let result = registry.search("xyznonexistent123456").await;
    assert!(result.is_ok(), "Search should succeed even with no results");

    let search_result = result.unwrap();
    println!(
        "Found {} servers for non-existent query",
        search_result.entries.len()
    );
}
