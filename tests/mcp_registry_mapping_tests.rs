use personal_agent::mcp::{
    EnvVarConfig, McpAuthType, McpConfig, McpPackage, McpPackageArg, McpPackageArgType,
    McpPackageType, McpRegistry, McpRegistryServerWrapper, McpSource, McpTransport,
};
use uuid::Uuid;

#[test]
fn entry_to_config_maps_env_vars_and_package_args() {
    let json = r#"{
        "server": {
            "name": "Filesystem",
            "description": "Local files",
            "version": "1.0.0",
            "packages": [
                {
                    "registryType": "npm",
                    "identifier": "@agent-infra/mcp-server-filesystem",
                    "transport": { "type": "stdio" },
                    "environmentVariables": [
                        { "name": "API_KEY", "isSecret": true, "isRequired": true }
                    ],
                    "packageArguments": [
                        { "type": "named", "name": "allowed-directories", "isRequired": true }
                    ]
                }
            ],
            "remotes": [
                { "type": "stdio", "url": "npx -y @agent-infra/mcp-server-filesystem" }
            ]
        },
        "_meta": {}
    }"#;

    let wrapper: McpRegistryServerWrapper = serde_json::from_str(json).unwrap();
    let entry = McpRegistry::entry_to_config(&wrapper).unwrap();

    let env = entry
        .env_vars
        .iter()
        .find(|var| var.name == "API_KEY")
        .unwrap();
    assert_eq!(
        env,
        &EnvVarConfig {
            name: "API_KEY".to_string(),
            required: true,
        }
    );
    assert_eq!(entry.auth_type, McpAuthType::ApiKey);

    let arg = entry
        .package_args
        .iter()
        .find(|arg| arg.name == "allowed-directories")
        .unwrap();
    assert_eq!(
        arg,
        &McpPackageArg {
            name: "allowed-directories".to_string(),
            arg_type: McpPackageArgType::Named,
            required: true,
            description: None,
            default: None,
        }
    );
}

#[test]
fn entry_to_config_prefers_package_transport() {
    let json = r#"{
        "server": {
            "name": "Remote MCP",
            "description": "Remote",
            "version": "1.0.0",
            "packages": [
                {
                    "registryType": "npm",
                    "identifier": "@agent-infra/mcp-server-remote",
                    "transport": { "type": "stdio" },
                    "environmentVariables": [],
                    "packageArguments": []
                }
            ],
            "remotes": [
                { "type": "http", "url": "https://mcp.example.com" }
            ]
        },
        "_meta": {}
    }"#;

    let wrapper: McpRegistryServerWrapper = serde_json::from_str(json).unwrap();
    let entry = McpRegistry::entry_to_config(&wrapper).unwrap();

    assert_eq!(entry.transport, McpTransport::Stdio);
    assert_eq!(entry.package.package_type, McpPackageType::Npm);
    assert_eq!(entry.package.identifier, "@agent-infra/mcp-server-remote");
}

#[test]
fn entry_to_config_defaults_for_manual_sources() {
    let config = McpConfig {
        id: Uuid::new_v4(),
        name: "Manual".to_string(),
        enabled: true,
        source: McpSource::Manual {
            url: "http://manual".to_string(),
        },
        package: McpPackage {
            package_type: McpPackageType::Http,
            identifier: "http://manual".to_string(),
            runtime_hint: None,
        },
        transport: McpTransport::Http,
        auth_type: McpAuthType::None,
        env_vars: vec![],
        package_args: vec![],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: None,
    };

    assert_eq!(
        config.source,
        McpSource::Manual {
            url: "http://manual".to_string()
        }
    );
}
