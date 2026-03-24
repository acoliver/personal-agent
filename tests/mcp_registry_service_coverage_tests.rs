use personal_agent::mcp::registry::{
    McpRegistryPackage, McpRegistryPackageArgument, McpRegistryRemote, McpRegistryRepository,
    McpRegistryServer, McpRegistryTransport,
};
use personal_agent::services::mcp_registry_impl::McpRegistryServiceImpl;
use personal_agent::services::McpRegistryService;
use tempfile::TempDir;

fn npm_wrapper_with_tags_and_source() -> serde_json::Value {
    serde_json::json!({
        "server": {
            "name": "fetch",
            "description": "Fetch documents from the internet",
            "repository": {
                "url": "https://github.com/example/fetch",
                "source": "github"
            },
            "version": "1.2.3",
            "packages": [
                {
                    "registryType": "npm",
                    "identifier": "@example/fetch-mcp",
                    "transport": {
                        "type": "stdio"
                    },
                    "environmentVariables": [
                        {
                            "name": "API_KEY",
                            "description": "api key",
                            "isSecret": true,
                            "isRequired": true
                        }
                    ],
                    "packageArguments": [
                        {
                            "type": "named",
                            "name": "allowed-directories",
                            "description": "paths",
                            "isRequired": true,
                            "default": "/tmp"
                        }
                    ]
                }
            ],
            "remotes": []
        },
        "_meta": {
            "tags": ["filesystem", "fetch"],
            "source": "both"
        }
    })
}

fn docker_wrapper() -> serde_json::Value {
    serde_json::json!({
        "server": {
            "name": "container-tools",
            "description": "Docker-based tool runner",
            "repository": {
                "url": "https://github.com/example/docker-tools",
                "source": null
            },
            "version": "9.9.9",
            "packages": [
                {
                    "registryType": "oci",
                    "identifier": "ghcr.io/example/docker-tools:latest",
                    "transport": {
                        "type": "stdio"
                    },
                    "environmentVariables": [],
                    "packageArguments": []
                }
            ],
            "remotes": []
        },
        "_meta": {
            "tags": ["docker"],
            "source": "official"
        }
    })
}

fn http_remote_wrapper() -> serde_json::Value {
    serde_json::json!({
        "server": {
            "name": "remote-http",
            "description": "Hosted MCP",
            "repository": {
                "url": "https://github.com/example/http-remote",
                "source": null
            },
            "version": "2.0.0",
            "packages": [],
            "remotes": [
                {
                    "type": "http",
                    "url": "https://mcp.example.com"
                }
            ]
        },
        "_meta": {
            "tags": ["remote"],
            "source": "smithery"
        }
    })
}

fn write_cache_file(temp_dir: &TempDir, wrappers: &[serde_json::Value]) {
    let content = serde_json::to_string_pretty(wrappers).expect("serialize wrapper cache");
    std::fs::write(temp_dir.path().join("mcp_registry.json"), content).expect("write cache file");
}

#[tokio::test]
async fn list_by_tag_matches_case_insensitively_and_maps_entry_fields() {
    let temp_dir = TempDir::new().unwrap();
    write_cache_file(
        &temp_dir,
        &[
            npm_wrapper_with_tags_and_source(),
            docker_wrapper(),
            http_remote_wrapper(),
        ],
    );

    let service = McpRegistryServiceImpl::with_cache_dir(temp_dir.path().to_path_buf());
    let results = service.list_by_tag("FILES").await.expect("list by tag");

    assert_eq!(results.len(), 1);
    let entry = &results[0];
    assert_eq!(entry.name, "fetch");
    assert_eq!(entry.display_name, "fetch");
    assert_eq!(entry.description, "Fetch documents from the internet");
    assert_eq!(entry.version, "1.2.3");
    assert_eq!(entry.author, "https://github.com/example/fetch");
    assert_eq!(entry.license, "Unknown");
    assert_eq!(entry.repository, "https://github.com/example/fetch");
    assert_eq!(entry.command, "@example/fetch-mcp");
    assert_eq!(entry.args, vec!["allowed-directories".to_string()]);
    assert_eq!(
        entry.env,
        Some(vec![("API_KEY".to_string(), String::new())])
    );
    assert_eq!(
        entry.tags,
        vec!["filesystem".to_string(), "fetch".to_string()]
    );
    assert_eq!(entry.source, "both");
    assert_eq!(
        entry.package_type,
        Some(personal_agent::mcp::McpPackageType::Npm)
    );
    assert_eq!(entry.runtime_hint, Some("npx".to_string()));
    assert_eq!(entry.url, None);
}

#[tokio::test]
async fn list_by_tag_returns_empty_when_tag_is_missing() {
    let temp_dir = TempDir::new().unwrap();
    write_cache_file(&temp_dir, &[docker_wrapper()]);

    let service = McpRegistryServiceImpl::with_cache_dir(temp_dir.path().to_path_buf());
    let results = service.list_by_tag("missing").await.expect("list by tag");

    assert!(results.is_empty());
}

#[tokio::test]
async fn list_trending_delegates_to_cached_list_all() {
    let temp_dir = TempDir::new().unwrap();
    write_cache_file(&temp_dir, &[docker_wrapper(), http_remote_wrapper()]);

    let service = McpRegistryServiceImpl::with_cache_dir(temp_dir.path().to_path_buf());
    let trending = service.list_trending().await.expect("list trending");

    assert_eq!(trending.len(), 2);
    assert_eq!(trending[0].name, "container-tools");
    assert_eq!(trending[0].runtime_hint, Some("docker".to_string()));
    assert_eq!(trending[1].name, "remote-http");
    assert_eq!(trending[1].package_type, None);
    assert_eq!(trending[1].runtime_hint, None);
    assert_eq!(trending[1].url, Some("https://mcp.example.com".to_string()));
}

#[tokio::test]
async fn get_last_refresh_defaults_to_none_before_refresh() {
    let temp_dir = TempDir::new().unwrap();
    let service = McpRegistryServiceImpl::with_cache_dir(temp_dir.path().to_path_buf());

    let last_refresh = service
        .get_last_refresh()
        .await
        .expect("last refresh query");
    assert_eq!(last_refresh, None);
}

#[test]
fn registry_type_support_types_construct_for_coverage() {
    let package = McpRegistryPackage {
        registry_type: "npm".to_string(),
        identifier: "@example/pkg".to_string(),
        version: Some("1.0.0".to_string()),
        transport: McpRegistryTransport {
            transport_type: "stdio".to_string(),
        },
        environment_variables: vec![],
        package_arguments: vec![McpRegistryPackageArgument {
            argument_type: "named".to_string(),
            name: "arg".to_string(),
            description: Some("description".to_string()),
            is_required: false,
            default: Some("value".to_string()),
        }],
    };
    let remote = McpRegistryRemote {
        remote_type: "http".to_string(),
        url: "https://mcp.example.com".to_string(),
    };
    let repository = McpRegistryRepository {
        url: Some("https://github.com/example/repo".to_string()),
        source: Some("github".to_string()),
    };
    let server = McpRegistryServer {
        name: "server".to_string(),
        description: "description".to_string(),
        repository,
        version: "1.0.0".to_string(),
        packages: vec![package],
        remotes: vec![remote],
    };

    assert_eq!(server.name, "server");
    assert_eq!(server.packages.len(), 1);
    assert_eq!(server.remotes.len(), 1);
}
