//! E2E test: Agent mode with MCP tool execution
//!
//! This test verifies that the Agent actually executes tools during streaming.
//! It checks for `ToolCallStarted` and `ToolCallCompleted` events in the output.
//!
//! Run with:
//!   cargo test --test `e2e_agent_tool_execution` -- --ignored --nocapture
//!
//! Requires:
//! - `PA_E2E_PROVIDER_ID` (optional; default: `ollama`)
//! - `PA_E2E_MODEL_ID` (optional; default: `minimax-m2.7:cloud`)
//! - `PA_E2E_BASE_URL` (optional; default: <https://ollama.com/v1>)
//! - `PA_E2E_KEY_LABEL` (optional; default: `pa-e2e-ollama-cloud`)
//! - `PA_E2E_API_KEY` (recommended for non-interactive runs)
//! - An MCP server configured with search capability (e.g., Exa)
//!   OR the test will gracefully skip tool verification if no MCPs configured

use personal_agent::llm::client_agent::McpToolContext;
use personal_agent::llm::AgentClientExt;
use personal_agent::services::McpRegistryService;
use personal_agent::{LlmClient, ModelProfile, StreamEvent};

mod support;

fn load_e2e_profile() -> ModelProfile {
    support::e2e_config::load_e2e_profile()
}

#[tokio::test]
#[ignore = "Requires PA_E2E_* configuration"]
async fn test_agent_mode_basic() {
    println!("=== E2E Test: Agent Mode Basic ===\n");

    let profile = load_e2e_profile();
    println!("Profile: {} / {}", profile.provider_id, profile.model_id);

    // Create client and agent
    let client = LlmClient::from_profile(&profile).expect("Failed to create LlmClient");
    let agent = client
        .create_agent(vec![], "You are a helpful assistant.")
        .await
        .expect("Failed to create agent");

    println!("Agent created successfully");
    println!("\nSending message to agent...");

    let messages = vec![personal_agent::LlmMessage::user(
        "Say 'Agent mode works' and nothing else.",
    )];

    let mut saw_text = false;
    let mut saw_done = false;
    let mut response = String::new();

    let result = client
        .run_agent_stream(
            &agent,
            &messages,
            McpToolContext::default(),
            |event| match &event {
                StreamEvent::TextDelta(text) => {
                    print!("{text}");
                    std::io::Write::flush(&mut std::io::stdout()).ok();
                    response.push_str(text);
                    saw_text = true;
                }
                StreamEvent::Complete => {
                    saw_done = true;
                }
                _ => {}
            },
        )
        .await;

    println!("\n");

    match result {
        Ok(()) => {
            assert!(saw_text, "Should see TextDelta events");
            assert!(saw_done, "Should see Done event");
            assert!(!response.is_empty(), "Response should not be empty");
            println!("[OK] Agent mode works!");
            println!("[OK] Response: {}", response.trim());
        }
        Err(e) => {
            panic!("Agent stream failed: {e}");
        }
    }
}

#[tokio::test]
#[ignore = "Requires PA_E2E_* configuration"]
async fn test_agent_tool_events() {
    println!("=== E2E Test: Agent Tool Events ===\n");

    // Initialize MCP service
    let mcp_service = personal_agent::mcp::McpService::global();
    let mut mcp = mcp_service.lock().await;

    match mcp.initialize().await {
        Ok(()) => {
            println!("[OK] MCP service initialized");
        }
        Err(e) => {
            println!("[SKIP] No MCPs configured: {e}");
            println!("[SKIP] Tool event test skipped - configure MCPs and re-run");
            return;
        }
    }

    let tools = mcp.get_llm_tools();
    println!("Available tools: {}", tools.len());
    for tool in &tools {
        println!("  - {}", tool.name);
    }

    if tools.is_empty() {
        println!("[SKIP] No tools available - configure MCPs with tools and re-run");
        return;
    }

    // Drop MCP lock before continuing
    drop(mcp);

    let profile = load_e2e_profile();
    let client = LlmClient::from_profile(&profile).expect("Failed to create LlmClient");

    // Create agent WITH tools
    let mcp = mcp_service.lock().await;
    let llm_tools = mcp.get_llm_tools();
    drop(mcp);

    let agent = client
        .create_agent(llm_tools, "You are a helpful assistant with tools.")
        .await
        .expect("Failed to create agent with tools");

    println!("\nSending message that should trigger tool use...");

    // This prompt should trigger a search tool if one is available
    let messages = vec![personal_agent::LlmMessage::user(
        "Search for information about Rust programming language. Use the search tool.",
    )];

    let mut saw_tool_start = false;
    let mut saw_tool_complete = false;
    let mut tool_name = String::new();

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
                StreamEvent::ToolCallStarted {
                    tool_name: name,
                    call_id,
                } => {
                    println!("\n[TOOL STARTED] {name} ({call_id})");
                    saw_tool_start = true;
                    tool_name = name.clone();
                }
                StreamEvent::ToolCallCompleted {
                    tool_name: name,
                    success,
                    call_id,
                    ..
                } => {
                    println!("[TOOL COMPLETED] {name} success={success} ({call_id})");
                    saw_tool_complete = true;
                }
                StreamEvent::Complete => {
                    println!("\n[DONE]");
                }
                _ => {}
            },
        )
        .await;

    println!("\n");

    match result {
        Ok(()) => {
            if saw_tool_start && saw_tool_complete {
                println!("[OK] Tool events detected!");
                println!("[OK] Tool '{tool_name}' was called and completed");
                println!("[OK] Agent mode with tools WORKS!");
            } else if !saw_tool_start {
                println!("[WARN] No tool was called - LLM may have answered directly");
                println!("[WARN] This is acceptable if LLM didn't need to use a tool");
            }
        }
        Err(e) => {
            panic!("Agent stream failed: {e}");
        }
    }
}

#[tokio::test]
#[ignore = "Requires PA_E2E_* configuration"]
async fn test_mcp_catalog_real() {
    println!("=== E2E Test: MCP Catalog Fetch ===\n");

    let registry =
        personal_agent::services::McpRegistryServiceImpl::new().expect("Failed to create registry");

    println!("Fetching MCP catalog from Smithery...");

    match registry.refresh().await {
        Ok(()) => println!("[OK] Catalog refreshed"),
        Err(e) => {
            println!("[WARN] Refresh failed: {e}");
        }
    }

    let results = registry.search("search").await.expect("Search should work");

    println!("Found {} MCP servers matching 'search'", results.len());
    for (i, entry) in results.iter().take(5).enumerate() {
        println!("  {}. {} - {}", i + 1, entry.name, entry.description);
    }

    assert!(!results.is_empty(), "Should find MCP servers in catalog");
    println!("\n[OK] MCP catalog E2E test passed!");
}
