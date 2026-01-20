use personal_agent::mcp::{McpRegistry, McpRegistrySource};

#[tokio::test]
async fn search_registry_requires_smithery_key() {
    let registry = McpRegistry::new();
    let result = registry
        .search_registry("query", McpRegistrySource::Smithery, None)
        .await;

    assert!(result.is_err());
}
