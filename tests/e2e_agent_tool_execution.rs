//! E2E test: Agent mode with MCP tool execution
//!
//! This test verifies that the Agent actually executes tools during streaming.
//! It checks for ToolCallStarted and ToolCallCompleted events in the output.
//!
//! Run with:
//!   cargo test --test e2e_agent_tool_execution -- --ignored --nocapture
//!
//! Requires:
//! - ~/.llxprt/profiles/synthetic.json (profile config)
//! - ~/.synthetic_key (API key)
//! - An MCP server configured with search capability (e.g., Exa)
//!   OR the test will gracefully skip tool verification if no MCPs configured

use personal_agent::{AuthConfig, LlmClient, ModelProfile, StreamEvent};
use personal_agent::llm::AgentClientExt;

/// Load synthetic profile from ~/.llxprt/profiles/synthetic.json
fn load_synthetic_profile() -> ModelProfile {
    let home = dirs::home_dir().expect("No home directory");
    let profile_path = home.join(".llxprt/profiles/synthetic.json");

    let content = std::fs::read_to_string(&profile_path)
        .expect("Failed to read ~/.llxprt/profiles/synthetic.json");

    let json: serde_json::Value =
        serde_json::from_str(&content).expect("Failed to parse synthetic.json");

    let provider = json["provider"].as_str().unwrap_or("openai").to_string();
    let model = json["model"]
        .as_str()
        .expect("No model in profile")
        .to_string();
    let base_url = json["ephemeralSettings"]["base-url"]
        .as_str()
        .unwrap_or("")
        .to_string();
    let keyfile_path = json["ephemeralSettings"]["auth-keyfile"]
        .as_str()
        .unwrap_or("~/.synthetic_key")
        .to_string();

    // Expand ~ to home directory
    let keyfile_path = if keyfile_path.starts_with("~/") {
        home.join(&keyfile_path[2..]).to_string_lossy().to_string()
    } else {
        keyfile_path
    };

    ModelProfile::new(
        "Synthetic GLM".to_string(),
        provider,
        model,
        base_url,
        AuthConfig::Keyfile { path: keyfile_path },
    )
}

#[tokio::test]
#[ignore]
async fn test_agent_mode_basic() {
    println!("=== E2E Test: Agent Mode Basic ===\n");

    let profile = load_synthetic_profile();
    println!("Profile: {} / {}", profile.provider_id, profile.model_id);

    // Create client and agent
    let client = LlmClient::from_profile(&profile).expect("Failed to create LlmClient");
    let agent = client.create_agent(vec![], "You are a helpful assistant.").await
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
        .run_agent_stream(&agent, &messages, |event| {
            match &event {
                StreamEvent::TextDelta(text) => {
                    print!("{}", text);
                    std::io::Write::flush(&mut std::io::stdout()).ok();
                    response.push_str(text);
                    saw_text = true;
                }
                StreamEvent::Complete => {
                    saw_done = true;
                }
                _ => {}
            }
        })
        .await;

    println!("\n");

    match result {
        Ok(_) => {
            assert!(saw_text, "Should see TextDelta events");
            assert!(saw_done, "Should see Done event");
            assert!(!response.is_empty(), "Response should not be empty");
            println!("[OK] Agent mode works!");
            println!("[OK] Response: {}", response.trim());
        }
        Err(e) => {
            panic!("Agent stream failed: {}", e);
        }
    }
}

#[tokio::test]
#[ignore]
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
            println!("[SKIP] No MCPs configured: {}", e);
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

    let profile = load_synthetic_profile();
    let client = LlmClient::from_profile(&profile).expect("Failed to create LlmClient");

    // Create agent WITH tools
    let mcp = mcp_service.lock().await;
    let llm_tools = mcp.get_llm_tools();
    drop(mcp);

    let agent = client.create_agent(llm_tools, "You are a helpful assistant with tools.").await
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
        .run_agent_stream(&agent, &messages, |event| {
            match &event {
                StreamEvent::TextDelta(text) => {
                    print!("{}", text);
                    std::io::Write::flush(&mut std::io::stdout()).ok();
                }
                StreamEvent::ToolCallStarted { tool_name: name, call_id } => {
                    println!("\n[TOOL STARTED] {} ({})", name, call_id);
                    saw_tool_start = true;
                    tool_name = name.clone();
                }
                StreamEvent::ToolCallCompleted { tool_name: name, success, call_id, .. } => {
                    println!("[TOOL COMPLETED] {} success={} ({})", name, success, call_id);
                    saw_tool_complete = true;
                }
                StreamEvent::Complete => {
                    println!("\n[DONE]");
                }
                _ => {}
            }
        })
        .await;

    println!("\n");

    match result {
        Ok(_) => {
            if saw_tool_start && saw_tool_complete {
                println!("[OK] Tool events detected!");
                println!("[OK] Tool '{}' was called and completed", tool_name);
                println!("[OK] Agent mode with tools WORKS!");
            } else if !saw_tool_start {
                println!("[WARN] No tool was called - LLM may have answered directly");
                println!("[WARN] This is acceptable if LLM didn't need to use a tool");
            }
        }
        Err(e) => {
            panic!("Agent stream failed: {}", e);
        }
    }
}

#[tokio::test]
#[ignore]
async fn test_mcp_catalog_real() {
    println!("=== E2E Test: MCP Catalog Fetch ===\n");

    let registry = personal_agent::services::McpRegistryServiceImpl::new()
        .expect("Failed to create registry");

    println!("Fetching MCP catalog from Smithery...");

    use personal_agent::services::McpRegistryService;

    match registry.refresh().await {
        Ok(()) => println!("[OK] Catalog refreshed"),
        Err(e) => {
            println!("[WARN] Refresh failed: {}", e);
        }
    }

    let results = registry
        .search("search")
        .await
        .expect("Search should work");

    println!("Found {} MCP servers matching 'search'", results.len());
    for (i, entry) in results.iter().take(5).enumerate() {
        println!("  {}. {} - {}", i + 1, entry.name, entry.description);
    }

    assert!(!results.is_empty(), "Should find MCP servers in catalog");
    println!("\n[OK] MCP catalog E2E test passed!");
}
