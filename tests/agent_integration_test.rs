//! Integration tests for Agent Mode

use personal_agent::agent::{runtime::run_in_agent_runtime, PersonalAgent};
use personal_agent::models::ModelProfile;

#[test]
fn test_full_agent_creation_path() {
    run_in_agent_runtime(async {
        // Create a test profile (no MCPs for this basic test)
        let profile = ModelProfile::default();

        // Create agent in global runtime
        let agent = PersonalAgent::new(&profile, &[]).await.unwrap();

        // Verify agent was created with no tools
        assert_eq!(agent.tool_count(), 0);
    });
}

#[test]
fn test_global_runtime_persists_across_tests() {
    // First operation
    let result1 = run_in_agent_runtime(async {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        1
    });
    assert_eq!(result1, 1);

    // Second operation - runtime should still be alive
    let result2 = run_in_agent_runtime(async {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        2
    });
    assert_eq!(result2, 2);
}

#[test]
fn test_agent_with_disabled_mcps() {
    use personal_agent::mcp::{
        McpAuthType, McpConfig, McpPackage, McpPackageType, McpSource, McpTransport,
    };

    run_in_agent_runtime(async {
        let profile = ModelProfile::default();
        let configs = vec![McpConfig {
            enabled: false,
            id: uuid::Uuid::new_v4(),
            name: "test_disabled".to_string(),
            source: McpSource::Official {
                name: "test".to_string(),
                version: "1.0.0".to_string(),
            },
            package: McpPackage {
                package_type: McpPackageType::Npm,
                identifier: "@test/mcp".to_string(),
                runtime_hint: Some("node".to_string()),
            },
            transport: McpTransport::Stdio,
            auth_type: McpAuthType::None,
            env_vars: vec![],
            keyfile_path: None,
            config: serde_json::json!({}),
            oauth_token: None,
        }];

        let agent = PersonalAgent::new(&profile, &configs).await.unwrap();

        // Disabled MCPs should not count
        assert_eq!(agent.tool_count(), 0);
    });
}
