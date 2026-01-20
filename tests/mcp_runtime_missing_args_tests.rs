use personal_agent::mcp::{
    McpAuthType, McpConfig, McpPackage, McpPackageArg, McpPackageArgType, McpPackageType,
    McpRuntime, McpSource, McpTransport, SecretsManager,
};
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn start_mcp_requires_package_args() {
    let temp_dir = TempDir::new().unwrap();
    let secrets = SecretsManager::new(temp_dir.path().to_path_buf());
    let mut runtime = McpRuntime::new(secrets);

    let config = McpConfig {
        id: Uuid::new_v4(),
        name: "Args".to_string(),
        enabled: true,
        source: McpSource::Manual {
            url: "".to_string(),
        },
        package: McpPackage {
            package_type: McpPackageType::Http,
            identifier: "".to_string(),
            runtime_hint: None,
        },
        transport: McpTransport::Http,
        auth_type: McpAuthType::None,
        env_vars: vec![],
        package_args: vec![McpPackageArg {
            name: "allowed".to_string(),
            arg_type: McpPackageArgType::Named,
            required: true,
            description: None,
            default: None,
        }],
        keyfile_path: None,
        config: serde_json::json!({}),
        oauth_token: None,
    };

    let result = runtime.start_mcp(&config).await;
    assert!(result.is_err());
    assert_eq!(runtime.status_manager().count_errors(), 1);
}
