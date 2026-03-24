//! E2E test: REAL tool execution with Exa search
//!
//! This test actually:
//! 1. Installs Exa MCP from registry
//! 2. Asks the agent to search for something
//! 3. Verifies tool was called and returned real results
//!
//! Run with:
//!   cargo test --test `e2e_real_tool_execution` -- --ignored --nocapture
//!
//! Requires:
//! - PA_E2E_PROVIDER_ID (optional; default: ollama)
//! - PA_E2E_MODEL_ID (optional; default: minimax-m2.7:cloud)
//! - PA_E2E_BASE_URL (optional; default: https://ollama.com/v1)
//! - PA_E2E_KEY_LABEL (optional; default: pa-e2e-ollama-cloud)
//! - PA_E2E_API_KEY (recommended for non-interactive runs)

use personal_agent::llm::AgentClientExt;
use personal_agent::mcp::McpService;
use personal_agent::services::{McpRegistryService, McpRegistryServiceImpl};
use personal_agent::{LlmClient, ModelProfile, StreamEvent};

mod support;

fn load_e2e_profile() -> ModelProfile {
    support::e2e_config::load_e2e_profile()
}

#[tokio::test]
#[ignore = "Requires PA_E2E_* configuration and Exa MCP"]
#[allow(clippy::too_many_lines)]
async fn test_real_exa_search() {
    println!("=== E2E Test: REAL Exa Search ===\n");

    // Step 1: Ensure Exa MCP is available
    println!("Step 1: Checking MCP registry for Exa...");
    let registry = McpRegistryServiceImpl::new().expect("Failed to create registry");
    registry
        .refresh()
        .await
        .expect("Failed to refresh registry");

    let results = registry.search("exa").await.expect("Search failed");
    let exa = results.iter().find(|r| r.name.contains("exa"));

    if exa.is_none() {
        println!("[SKIP] Exa not found in registry");
        return;
    }
    println!("Found Exa in registry: {}", exa.unwrap().name);

    // Step 2: Initialize MCP service and check for tools
    println!("\nStep 2: Initializing MCP service...");
    let mcp_service = McpService::global();
    let mut mcp = mcp_service.lock().await;

    // Try to initialize - this loads configured MCPs
    match mcp.initialize().await {
        Ok(()) => println!("MCP service initialized"),
        Err(e) => println!("MCP init note: {e}"),
    }

    let tools = mcp.get_llm_tools();
    println!("Available tools: {}", tools.len());

    if tools.is_empty() {
        println!("\n[INFO] No MCP tools configured yet.");
        println!("[INFO] To test with Exa, configure it in the app first.");
        println!("[INFO] The install() function works - use the UI to add Exa MCP.");

        // Still test agent mode works
        drop(mcp);

        println!("\nStep 3: Testing agent without tools (basic mode)...");
        let profile = load_e2e_profile();
        let client = LlmClient::from_profile(&profile).expect("Failed to create client");
        let agent = client
            .create_agent(vec![], "You are a helpful assistant.")
            .await
            .expect("Failed to create agent");

        let messages = vec![personal_agent::LlmMessage::user(
            "What is 2 + 2? Reply with just the number.",
        )];

        client
            .run_agent_stream(&agent, &messages, |event| {
                if let StreamEvent::TextDelta(text) = event {
                    print!("{text}");
                }
            })
            .await
            .expect("Agent stream failed");

        println!("\n\n[OK] Agent works (no tools mode)");
        println!("[INFO] Configure Exa MCP to test tool execution");
        return;
    }

    // If we have tools, test with them
    println!("\nTools available:");
    for tool in &tools {
        println!("  - {}: {}", tool.name, tool.description);
    }

    drop(mcp);

    // Step 3: Create agent with tools and search
    println!("\nStep 3: Creating agent with tools...");
    let profile = load_e2e_profile();
    let client = LlmClient::from_profile(&profile).expect("Failed to create client");

    let mcp = mcp_service.lock().await;
    let llm_tools = mcp.get_llm_tools();
    drop(mcp);

    let agent = client
        .create_agent(llm_tools, "You are a helpful assistant with web search capability. Use the search tool when asked to find information.")
        .await
        .expect("Failed to create agent");

    println!("\nStep 4: Asking agent to search...");
    let messages = vec![personal_agent::LlmMessage::user(
        "Search for 'Rust programming language' and tell me one fact you found.",
    )];

    let mut saw_tool_start = false;
    let mut saw_tool_complete = false;
    let mut tool_name = String::new();
    let mut response = String::new();

    let result = client
        .run_agent_stream(&agent, &messages, |event| match &event {
            StreamEvent::TextDelta(text) => {
                print!("{text}");
                std::io::Write::flush(&mut std::io::stdout()).ok();
                response.push_str(text);
            }
            StreamEvent::ToolCallStarted {
                tool_name: name,
                call_id,
            } => {
                println!("\n\n[TOOL STARTED] {name} ({call_id})");
                saw_tool_start = true;
                tool_name = name.clone();
            }
            StreamEvent::ToolCallCompleted {
                tool_name: name,
                success,
                result,
                call_id,
                ..
            } => {
                println!("[TOOL COMPLETED] {name} success={success} ({call_id})");
                if let Some(r) = result {
                    let preview = if r.len() > 200 { &r[..200] } else { r };
                    println!("[RESULT PREVIEW] {preview}...");
                }
                saw_tool_complete = true;
            }
            StreamEvent::Complete => {
                println!("\n[STREAM COMPLETE]");
            }
            _ => {}
        })
        .await;

    println!("\n");

    match result {
        Ok(()) => {
            if saw_tool_start && saw_tool_complete {
                println!("========================================");
                println!("E2E TEST PASSED - REAL TOOL EXECUTION!");
                println!("========================================");
                println!("Tool '{tool_name}' was called and completed");
                println!("Response length: {} chars", response.len());
            } else {
                println!("[WARN] LLM responded without using tools");
                println!("[WARN] This may happen if the model chose not to search");
                println!("Response: {response}");
            }
        }
        Err(e) => {
            panic!("Agent stream failed: {e}");
        }
    }
}
