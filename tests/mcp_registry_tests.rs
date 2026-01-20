use personal_agent::mcp::registry::{
    McpRegistryEnvVar, McpRegistryPackage, McpRegistryPackageArgument, McpRegistryServer,
    McpRegistryTransport,
};
use personal_agent::mcp::{
    McpPackageType, McpRegistry, McpRegistryServerWrapper, McpSource, McpTransport,
};

#[test]
fn mcp_registry_entry_to_config_maps_package_args() {
    let wrapper = McpRegistryServerWrapper {
        server: McpRegistryServer {
            name: "test/fs-server".to_string(),
            description: "Filesystem MCP".to_string(),
            repository: Default::default(),
            version: "1.0.0".to_string(),
            packages: vec![McpRegistryPackage {
                registry_type: "npm".to_string(),
                identifier: "@agent-infra/mcp-server-filesystem".to_string(),
                version: Some("1.0.0".to_string()),
                transport: McpRegistryTransport {
                    transport_type: "stdio".to_string(),
                },
                environment_variables: vec![McpRegistryEnvVar {
                    name: "API_KEY".to_string(),
                    description: None,
                    is_secret: true,
                    is_required: true,
                }],
                package_arguments: vec![McpRegistryPackageArgument {
                    argument_type: "named".to_string(),
                    name: "allowed-directories".to_string(),
                    description: Some("Allowed directories".to_string()),
                    is_required: true,
                    default: None,
                }],
            }],
            remotes: vec![],
        },
        meta: serde_json::json!({}),
    };

    let config = McpRegistry::entry_to_config(&wrapper).unwrap();
    assert_eq!(
        config.source,
        McpSource::Official {
            name: "test/fs-server".to_string(),
            version: "1.0.0".to_string(),
        }
    );
    assert_eq!(config.package.package_type, McpPackageType::Npm);
    assert_eq!(config.transport, McpTransport::Stdio);
    assert_eq!(config.package_args.len(), 1);
    assert_eq!(config.package_args[0].name, "allowed-directories");
    assert!(config.package_args[0].required);
}
