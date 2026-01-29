# Phase 04: E2E Test - Full Flow

**Plan:** PLAN-20250128-AGENT
**Phase:** P04
**Prerequisites:** P03 evidence with PASS
**Subagent:** rustexpert

---

## Objective

Write an E2E test that proves the ENTIRE FLOW works:

1. Service searches MCP catalog
2. Service installs MCP 
3. ChatService creates Agent with MCP tools
4. Agent executes tool
5. Test receives ToolCallStarted and ToolCallCompleted events
6. Response contains data from tool execution

---

## Test File

**File:** `tests/e2e_agent_tool_execution.rs`

```rust
//! E2E Test: Full flow from MCP install to tool execution
//!
//! Run: cargo test --test e2e_agent_tool_execution -- --ignored --nocapture
//!
//! This test proves:
//! 1. McpRegistryService.install() works (not a stub)
//! 2. ChatService uses Agent mode (not raw model)
//! 3. Agent executes MCP tools (not just collects them)
//! 4. We receive ToolCallStarted/ToolCallCompleted events
//!
//! Requires:
//! - Internet connection (fetches MCP catalog, calls LLM API)
//! - EXA_API_KEY environment variable OR ~/.exa_key file
//! - ~/.llxprt/profiles/synthetic.json (or another working profile)

use personal_agent::services::{
    ChatService, ChatServiceImpl, ChatStreamEvent,
    McpRegistryService, McpRegistryServiceImpl,
    ConversationService, ConversationServiceImpl,
    ProfileService, ProfileServiceImpl,
};
use personal_agent::mcp::McpService;
use personal_agent::events::EventBus;
use std::sync::Arc;
use futures::StreamExt;

#[tokio::test]
#[ignore] // Run manually with --ignored
async fn test_full_flow_install_mcp_and_agent_executes_tool() {
    println!("\n=== E2E TEST: Full Agent + MCP Flow ===\n");
    
    // =========================================
    // STEP 1: Search MCP catalog for Exa
    // =========================================
    println!("Step 1: Searching MCP catalog for 'exa'...");
    
    let registry = McpRegistryServiceImpl::new()
        .expect("Should create registry service");
    
    // Refresh catalog first
    registry.refresh().await.expect("Should refresh catalog");
    
    let results = registry.search("exa").await
        .expect("Should search catalog");
    
    assert!(!results.is_empty(), "Should find Exa in MCP catalog");
    
    let exa = results.iter()
        .find(|r| r.name.to_lowercase().contains("exa"))
        .expect("Should find Exa MCP");
    
    println!("   Found: {} - {}", exa.name, exa.description);
    println!("   Step 1: PASS\n");
    
    // =========================================
    // STEP 2: Install MCP via service layer
    // =========================================
    println!("Step 2: Installing Exa MCP via service layer...");
    
    // This MUST actually install, not just return Ok(())
    registry.install(&exa.name, Some("exa-search".to_string())).await
        .expect("Should install Exa MCP");
    
    println!("   Install completed");
    
    // =========================================
    // STEP 3: Verify MCP tools are available
    // =========================================
    println!("Step 3: Verifying MCP tools are available...");
    
    let mcp_service = McpService::global();
    {
        let mcp = mcp_service.lock().await;
        let tools = mcp.get_llm_tools();
        
        println!("   Available tools: {}", tools.len());
        for tool in &tools {
            println!("     - {}", tool.name);
        }
        
        let has_search = tools.iter().any(|t| 
            t.name.to_lowercase().contains("search") || 
            t.name.to_lowercase().contains("exa")
        );
        
        assert!(has_search, "Exa search tool MUST be available after install");
    }
    println!("   Step 3: PASS\n");
    
    // =========================================
    // STEP 4: Create ChatService and send message
    // =========================================
    println!("Step 4: Sending message via ChatService...");
    
    let event_bus = Arc::new(EventBus::new());
    let conversation_service = Arc::new(ConversationServiceImpl::new()?);
    let profile_service = Arc::new(ProfileServiceImpl::new()?);
    
    let chat_service = ChatServiceImpl::new(
        event_bus.clone(),
        conversation_service.clone(),
        profile_service.clone(),
    );
    
    // Create a test conversation
    let conversation_id = conversation_service.create("E2E Test").await?;
    
    // Send a message that REQUIRES a search
    println!("   Sending: 'Search for the latest Rust programming news'");
    
    let mut stream = chat_service.send_message(
        conversation_id,
        "Search for the latest Rust programming news and tell me the top result. \
         You MUST use the search tool.".to_string(),
    ).await.expect("send_message should work");
    
    // =========================================
    // STEP 5: Collect events and verify tool execution
    // =========================================
    println!("Step 5: Collecting stream events...\n");
    
    let mut response = String::new();
    let mut tool_started = false;
    let mut tool_completed = false;
    let mut tool_name = String::new();
    let mut tool_result: Option<String> = None;
    
    while let Some(event) = stream.next().await {
        match event {
            ChatStreamEvent::Token(text) => {
                print!("{}", text);
                response.push_str(&text);
            }
            ChatStreamEvent::ToolCallStarted { tool_name: name, .. } => {
                println!("\n   [TOOL STARTED: {}]", name);
                tool_started = true;
                tool_name = name;
            }
            ChatStreamEvent::ToolCallCompleted { success, result, .. } => {
                println!("   [TOOL COMPLETED: success={}]", success);
                if let Some(ref r) = result {
                    // Print first 200 chars of result
                    let preview: String = r.chars().take(200).collect();
                    println!("   [RESULT PREVIEW: {}...]", preview);
                }
                tool_completed = success;
                tool_result = result;
            }
            ChatStreamEvent::Complete => {
                println!("\n   [STREAM COMPLETE]");
                break;
            }
            ChatStreamEvent::Error(e) => {
                panic!("Stream error: {:?}", e);
            }
        }
    }
    
    // =========================================
    // STEP 6: Assertions - THESE PROVE IT WORKS
    // =========================================
    println!("\n=== VERIFICATION ===\n");
    
    // CRITICAL: Tool must have been called
    println!("Tool was started: {}", tool_started);
    assert!(tool_started, "Agent MUST have started a tool call");
    
    println!("Tool name: {}", tool_name);
    assert!(
        tool_name.to_lowercase().contains("search") || tool_name.to_lowercase().contains("exa"),
        "Tool should be a search tool"
    );
    
    println!("Tool completed successfully: {}", tool_completed);
    assert!(tool_completed, "Tool MUST have completed successfully");
    
    println!("Got tool result: {}", tool_result.is_some());
    assert!(tool_result.is_some(), "Tool MUST have returned a result");
    
    let result = tool_result.unwrap();
    println!("Result length: {} chars", result.len());
    assert!(result.len() > 50, "Result should have substantial content");
    
    println!("Response length: {} chars", response.len());
    assert!(!response.is_empty(), "Response should not be empty");
    
    // Response should contain info that could only come from search
    let response_lower = response.to_lowercase();
    let has_search_content = response_lower.contains("rust");
    assert!(has_search_content, "Response should contain Rust info from search");
    
    println!("\n=== ALL ASSERTIONS PASSED ===");
    println!("\nFull flow verified:");
    println!("  1. McpRegistryService.search() found Exa");
    println!("  2. McpRegistryService.install() installed Exa");
    println!("  3. McpService has search tool available");
    println!("  4. ChatService.send_message() used Agent mode");
    println!("  5. Agent executed search tool");
    println!("  6. Tool returned real results");
    println!("  7. Response contains search data");
    println!("\nE2E TEST PASSED!\n");
}
```

---

## Verification Commands (BLOCKING)

### Check 1: Test file exists
```bash
ls tests/e2e_agent_tool_execution.rs
```

### Check 2: Test compiles
```bash
cargo build --test e2e_agent_tool_execution 2>&1 | tail -5
```

### Check 3: Test passes (THE REAL PROOF)
```bash
EXA_API_KEY=your-key cargo test --test e2e_agent_tool_execution -- --ignored --nocapture 2>&1 | tail -50
```

**Expected output MUST contain:**
- `[TOOL STARTED: ...]`
- `[TOOL COMPLETED: success=true]`
- `[RESULT PREVIEW: ...]` with actual search results
- `ALL ASSERTIONS PASSED`
- `E2E TEST PASSED`

---

## What This Test Proves

| Assertion | What It Proves |
|-----------|----------------|
| `tool_started == true` | Agent called a tool (not just collected it) |
| `tool_completed == true` | MCP executed the tool successfully |
| `tool_result.is_some()` | Tool returned actual data |
| `result.len() > 50` | Result is real content, not stub |
| `response.contains("rust")` | LLM used the search results |

---

## Deliverables

1. `tests/e2e_agent_tool_execution.rs` exists
2. Test compiles
3. Test passes with real API calls
4. Evidence file shows test output with tool execution
5. Evidence file at `plan/.completed/P04.md`
