use personal_agent::mcp::{
    McpPackageType, McpRegistry, McpRegistryRemote, McpRegistryServer, McpRegistryServerWrapper,
    McpSource, McpTransport,
};

#[test]
fn entry_to_config_maps_remote_http() {
    let wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "Remote".to_string(),
            description: "".to_string(),
            repository: Default::default(),
            version: "1.0.0".to_string(),
            packages: vec![],
            remotes: vec![McpRegistryRemote {
                remote_type: "http".to_string(),
                url: "https://mcp.example.com".to_string(),
            }],
        },
        meta: serde_json::json!({}),
    };

    let config = McpRegistry::entry_to_config(&wrapper).unwrap();
    assert_eq!(config.transport, McpTransport::Http);
    assert_eq!(config.package.package_type, McpPackageType::Http);
    assert_eq!(config.package.identifier, "https://mcp.example.com");
    assert_eq!(
        config.source,
        McpSource::Manual {
            url: "https://mcp.example.com".to_string()
        }
    );
}

#[test]
fn entry_to_config_errors_on_unknown_package_type() {
    let json = r#"{
        "server": {
            "name": "Unknown",
            "description": "",
            "version": "1.0.0",
            "packages": [
                {
                    "registryType": "unknown",
                    "identifier": "@example/mcp",
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
    assert!(result.is_err());
}

#[test]
fn entry_to_config_errors_on_unknown_transport_type() {
    let json = r#"{
        "server": {
            "name": "Unknown",
            "description": "",
            "version": "1.0.0",
            "packages": [
                {
                    "registryType": "npm",
                    "identifier": "@example/mcp",
                    "transport": { "type": "udp" },
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
    assert!(result.is_err());
}

#[test]
fn entry_to_config_errors_on_unknown_remote_type() {
    let wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "Remote".to_string(),
            description: "".to_string(),
            repository: Default::default(),
            version: "1.0.0".to_string(),
            packages: vec![],
            remotes: vec![McpRegistryRemote {
                remote_type: "udp".to_string(),
                url: "https://mcp.example.com".to_string(),
            }],
        },
        meta: serde_json::json!({}),
    };

    let result = McpRegistry::entry_to_config(&wrapper);
    assert!(result.is_err());
}
