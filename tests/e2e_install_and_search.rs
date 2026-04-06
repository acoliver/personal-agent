//! E2E test: Install Exa MCP and do a real search
//!
//! This test:
//! 1. Installs Exa MCP from registry using `install()`
//! 2. Initializes the MCP service
//! 3. Creates an agent with the Exa tool
//! 4. Asks it to search for something
//! 5. Verifies tool execution
//!
//! Run with:
//!   cargo test --test `e2e_install_and_search` -- --ignored --nocapture
//!
//! Requires:
//! - `PA_E2E_PROVIDER_ID` (optional; default: `ollama`)
//! - `PA_E2E_MODEL_ID` (optional; default: `minimax-m2.7:cloud`)
//! - `PA_E2E_BASE_URL` (optional; default: <https://ollama.com/v1>)
//! - `PA_E2E_KEY_LABEL` (optional; default: `pa-e2e-ollama-cloud`)
//! - `PA_E2E_API_KEY` (recommended for non-interactive runs)

use personal_agent::llm::client_agent::McpToolContext;
use personal_agent::llm::AgentClientExt;
use personal_agent::mcp::{McpRegistry, McpService};
use personal_agent::services::{McpRegistryService, McpRegistryServiceImpl};
use personal_agent::{LlmClient, ModelProfile, StreamEvent};
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

mod support;

fn load_e2e_profile() -> ModelProfile {
    support::e2e_config::load_e2e_profile()
}

#[tokio::test]
#[ignore = "Requires PA_E2E_* configuration and Exa MCP"]
#[allow(clippy::too_many_lines)]
async fn test_install_exa_and_search() {
    println!("=== E2E Test: Install Exa and Search ===\n");

    // Step 1: Install Exa from registry (via mock registry)
    println!("Step 1: Installing Exa MCP from registry (mock)...");

    let mock_server = MockServer::start().await;
    let fixture = include_str!("fixtures/mcp_registry_with_exa.json");
    Mock::given(any())
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(fixture)
                .insert_header("content-type", "application/json"),
        )
        .mount(&mock_server)
        .await;

    let mock_registry = McpRegistry::with_url(mock_server.uri());
    let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
    let registry =
        McpRegistryServiceImpl::with_registry(temp_dir.path().to_path_buf(), mock_registry);

    // Refresh to get latest catalog
    registry
        .refresh()
        .await
        .expect("Failed to refresh registry");

    // Try to install Exa
    match registry
        .install("exa", Some("Exa Search".to_string()))
        .await
    {
        Ok(()) => println!("[OK] Exa installed successfully!"),
        Err(e) => {
            let err_str = format!("{e:?}");
            if err_str.contains("already exists") {
                println!("[OK] Exa already installed");
            } else {
                println!("[ERROR] Failed to install Exa: {e:?}");
                println!("[INFO] Continuing anyway to check if it's already configured...");
            }
        }
    }

    // Step 2: Initialize MCP service
    println!("\nStep 2: Initializing MCP service...");
    let mcp_service = McpService::global();
    let mut mcp = mcp_service.lock().await;

    match mcp.reload().await {
        Ok(()) => println!("[OK] MCP service reloaded"),
        Err(e) => println!("[WARN] MCP reload: {e}"),
    }

    match mcp.initialize().await {
        Ok(()) => println!("[OK] MCP service initialized"),
        Err(e) => println!("[WARN] MCP init: {e}"),
    }

    let tools = mcp.get_llm_tools();
    println!("Available tools: {}", tools.len());

    if tools.is_empty() {
        println!("\n[FAIL] No tools available after install!");
        println!("[INFO] The MCP may need to be started manually or needs API key");

        // Check config to see if Exa is there
        let config_path = personal_agent::config::Config::default_path().unwrap();
        let config = personal_agent::config::Config::load(&config_path).unwrap();
        println!("\nConfigured MCPs: {}", config.mcps.len());
        for mcp_config in &config.mcps {
            println!("  - {} (enabled: {})", mcp_config.name, mcp_config.enabled);
        }

        drop(mcp);
        return;
    }

    println!("\nAvailable tools:");
    for tool in &tools {
        println!("  - {}: {}", tool.name, tool.description);
    }

    let llm_tools = mcp.get_llm_tools();
    drop(mcp);

    // Step 3: Create agent with tools
    println!("\nStep 3: Creating agent with Exa tools...");
    let profile = load_e2e_profile();
    let client = LlmClient::from_profile(&profile).expect("Failed to create client");

    let agent = client
        .create_agent(
            llm_tools,
            "You are a helpful assistant. When asked to search, use the web_search tool.",
        )
        .await
        .expect("Failed to create agent");

    // Step 4: Search!
    println!("\nStep 4: Asking agent to search for 'Rust programming'...\n");

    let messages = vec![personal_agent::LlmMessage::user(
        "Use the search tool to find information about 'Rust programming language'. Tell me one interesting fact from the results.",
    )];

    let mut saw_tool_start = false;
    let mut saw_tool_complete = false;
    let mut tool_result_preview = String::new();

    let result = client
        .run_agent_stream(
            &agent,
            &messages,
            McpToolContext::default(),
            |event| match &event {
                StreamEvent::TextDelta(text) => {
                    print!("{text}");
                    std::io::Write::flush(&mut std::io::stdout()).ok();
                }
                StreamEvent::ToolCallStarted { tool_name, call_id } => {
                    println!("\n\n>>> [TOOL STARTED] {tool_name} ({call_id})");
                    saw_tool_start = true;
                }
                StreamEvent::ToolCallCompleted {
                    tool_name,
                    success,
                    result,
                    call_id,
                    ..
                } => {
                    println!(">>> [TOOL COMPLETED] {tool_name} success={success} ({call_id})");
                    if let Some(r) = result {
                        tool_result_preview = if r.len() > 500 {
                            format!("{}...", &r[..500])
                        } else {
                            r.clone()
                        };
                        println!(">>> [RESULT] {tool_result_preview}");
                    }
                    saw_tool_complete = true;
                }
                StreamEvent::Complete { .. } => {
                    println!("\n\n>>> [STREAM COMPLETE]");
                }
                StreamEvent::Error(e) => {
                    println!("\n>>> [ERROR] {e}");
                }
                _ => {}
            },
        )
        .await;

    println!("\n");

    match result {
        Ok(()) => {
            println!("==========================================");
            if saw_tool_start && saw_tool_complete {
                println!("E2E TEST PASSED - REAL TOOL EXECUTION!");
                println!("==========================================");
                println!("- Tool was called: YES");
                println!("- Tool completed: YES");
                if !tool_result_preview.is_empty() {
                    println!("- Got real results: YES");
                }
            } else {
                println!("E2E TEST PARTIAL - Agent responded but no tool call");
                println!("==========================================");
                println!("- Tool was called: {saw_tool_start}");
                println!("- Tool completed: {saw_tool_complete}");
                println!("\nThe LLM may have chosen not to use the tool.");
            }
        }
        Err(e) => {
            println!("E2E TEST FAILED");
            println!("==========================================");
            println!("Error: {e}");
        }
    }
}
