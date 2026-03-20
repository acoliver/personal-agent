//! MCP Add View - Result Projection Regression Tests
//!
//! Validates that projected registry results retain source/transport metadata in the
//! local `McpAddView` state, preventing source-context loss between search and selection.

use personal_agent::ui_gpui::views::mcp_add_view::McpRegistry;

#[test]
fn test_projected_search_results_preserve_source_args_and_env_metadata() {
    let raw = personal_agent::presentation::view_command::McpRegistryResult {
        id: "filesystem".to_string(),
        name: "Filesystem".to_string(),
        description: "Filesystem tools".to_string(),
        source: "smithery".to_string(),
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-filesystem".to_string(),
        ],
        env: Some(vec![("FILESYSTEM_ROOT".to_string(), "/tmp".to_string())]),
        package_type: Some(personal_agent::mcp::McpPackageType::Npm),
        runtime_hint: Some("npx".to_string()),
        url: None,
    };

    let mapped = [
        personal_agent::ui_gpui::views::mcp_add_view::McpSearchResult::new(
            raw.id.clone(),
            raw.name.clone(),
            raw.description.clone(),
        )
        .with_registry(match raw.source.as_str() {
            "smithery" => McpRegistry::Smithery,
            "both" => McpRegistry::Both,
            _ => McpRegistry::Official,
        })
        .with_command(raw.command.clone())
        .with_args(raw.args.clone())
        .with_env(raw.env.clone())
        .with_source(raw.source.clone())
        .with_package_metadata(raw.package_type.clone(), raw.runtime_hint.clone()),
    ];

    assert_eq!(mapped.len(), 1);
    let projected = &mapped[0];
    assert_eq!(projected.source, "smithery");
    assert_eq!(projected.registry, McpRegistry::Smithery);
    assert_eq!(projected.command, "npx");
    assert_eq!(
        projected.args,
        vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-filesystem".to_string()
        ]
    );
    assert_eq!(
        projected.env,
        Some(vec![("FILESYSTEM_ROOT".to_string(), "/tmp".to_string())])
    );
}

#[test]
fn test_draft_loaded_projection_uses_source_prefixed_id_to_set_registry_and_source() {
    let draft_id = "smithery::fetch".to_string();

    let (source_hint, normalized_id) = draft_id.split_once("::").map_or_else(
        || (None, draft_id.clone()),
        |(source, raw_id)| (Some(source.to_string()), raw_id.to_string()),
    );

    let registry = match source_hint.as_deref() {
        Some("smithery") => McpRegistry::Smithery,
        Some("official") => McpRegistry::Official,
        _ => McpRegistry::Both,
    };

    let inferred_source = source_hint.unwrap_or_else(|| match registry {
        McpRegistry::Official => "official".to_string(),
        McpRegistry::Smithery => "smithery".to_string(),
        McpRegistry::Both => "both".to_string(),
    });

    let projected = personal_agent::ui_gpui::views::mcp_add_view::McpSearchResult::new(
        normalized_id,
        "Fetch",
        "Selected MCP",
    )
    .with_registry(registry)
    .with_command("fetch")
    .with_args(vec![
        "-y".to_string(),
        "@modelcontextprotocol/server-fetch".to_string(),
    ])
    .with_env(Some(vec![("FETCH_API_KEY".to_string(), String::new())]))
    .with_source(inferred_source);

    assert_eq!(projected.id, "fetch");
    assert_eq!(projected.registry, McpRegistry::Smithery);
    assert_eq!(projected.source, "smithery");
    assert_eq!(projected.command, "fetch");
    assert_eq!(
        projected.args,
        vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-fetch".to_string()
        ]
    );
}
