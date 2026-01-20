use personal_agent::mcp::{McpRegistry, McpRegistryServerWrapper};

#[test]
fn entry_to_config_rejects_missing_packages() {
    let json = r#"{
        "server": {
            "name": "Empty",
            "description": "",
            "version": "1.0.0",
            "packages": [],
            "remotes": []
        },
        "_meta": {}
    }"#;

    let wrapper: McpRegistryServerWrapper = serde_json::from_str(json).unwrap();
    let result = McpRegistry::entry_to_config(&wrapper);
    assert!(result.is_err());
}

#[test]
fn entry_to_config_uses_package_when_present() {
    let json = r#"{
        "server": {
            "name": "Has packages",
            "description": "",
            "version": "1.0.0",
            "packages": [
                {
                    "registryType": "npm",
                    "identifier": "@agent-infra/mcp-server",
                    "transport": { "type": "stdio" },
                    "environmentVariables": [],
                    "packageArguments": []
                }
            ],
            "remotes": []
        },
        "_meta": {}
    }"#;

    let wrapper: McpRegistryServerWrapper = serde_json::from_str(json).unwrap();
    let result = McpRegistry::entry_to_config(&wrapper);
    assert!(result.is_ok());
}
