# Architecture Diagrams: Agent Mode Implementation

## Diagram 1: What We Have Now (WRONG)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              CURRENT (BROKEN)                                 │
└──────────────────────────────────────────────────────────────────────────────┘

User: "Search for rust async patterns"
                │
                ▼
┌──────────────────────────────────────┐
│          ChatServiceImpl             │
│                                      │
│  let client = LlmClient::from_profile()
│                                      │
│  client.request_stream_with_tools()  │◄── Uses RAW MODEL, not Agent
│         │                            │
│         ▼                            │
│  ┌────────────────────────────────┐  │
│  │      LlmClient                 │  │
│  │                                │  │
│  │  model.request_stream()        │◄─┼── Direct model call
│  │         │                      │  │
│  │         ▼                      │  │
│  │  LLM says: "tool_use: exa"     │  │
│  │         │                      │  │
│  │         ▼                      │  │
│  │  pending_tool_calls.push()     │◄─┼── COLLECTS BUT NEVER EXECUTES!
│  │         │                      │  │
│  │         ▼                      │  │
│  │  (tool call ignored)           │  │
│  │         │                      │  │
│  │         ▼                      │  │
│  │  Stream ends                   │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
                │
                ▼
        Response: "I'll search..." 
        (But search NEVER HAPPENED)
```

**THE PROBLEM:** 
- LlmClient uses `model.request_stream()` - raw model, no agent
- Tool calls received but NEVER EXECUTED
- User gets "I'll search for that" but NO SEARCH HAPPENS

---

## Diagram 2: What We Need (CORRECT)

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                              TARGET (CORRECT)                                 │
└──────────────────────────────────────────────────────────────────────────────┘

User: "Search for rust async patterns"
                │
                ▼
┌──────────────────────────────────────┐
│          ChatServiceImpl             │
│                                      │
│  // 1. Build Agent with tools        │
│  let agent = AgentBuilder::from_config(model_config)
│      .system_prompt(...)             │
│      .toolset(mcp_toolset)           │◄── MCP tools attached
│      .build()                        │
│                                      │
│  // 2. Run agent stream              │
│  agent.run_stream(user_message)      │◄── Agent handles everything
│         │                            │
└─────────┼────────────────────────────┘
          │
          ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│                           SERDES-AI AGENT                                     │
│                                                                               │
│  Step 1: Send to LLM                                                          │
│          │                                                                    │
│          ▼                                                                    │
│  LLM Response: "I'll search using exa_search tool"                            │
│                tool_use: { name: "exa_search", args: {query: "..."} }         │
│          │                                                                    │
│          ▼                                                                    │
│  Step 2: Agent sees tool_use, EXECUTES IT                                     │
│          │                                                                    │
│          ├──────────────────────────────────┐                                 │
│          │                                  ▼                                 │
│          │              ┌─────────────────────────────────┐                   │
│          │              │       MCP TOOLSET               │                   │
│          │              │                                 │                   │
│          │              │  toolset.execute("exa_search")  │                   │
│          │              │          │                      │                   │
│          │              │          ▼                      │                   │
│          │              │  ┌─────────────────────────┐    │                   │
│          │              │  │    MCP SERVER           │    │                   │
│          │              │  │    (exa-mcp-server)     │    │                   │
│          │              │  │                         │    │                   │
│          │              │  │  Actually calls Exa API │    │                   │
│          │              │  │  Returns search results │    │                   │
│          │              │  └─────────────────────────┘    │                   │
│          │              │          │                      │                   │
│          │              │          ▼                      │                   │
│          │              │  tool_result: [search results]  │                   │
│          │              └─────────────────────────────────┘                   │
│          │                                  │                                 │
│          ◄──────────────────────────────────┘                                 │
│          │                                                                    │
│          ▼                                                                    │
│  Step 3: Agent adds tool_result to conversation                               │
│          │                                                                    │
│          ▼                                                                    │
│  Step 4: Agent sends updated conversation to LLM                              │
│          │                                                                    │
│          ▼                                                                    │
│  LLM Response: "Based on my search, here are patterns for rust async..."      │
│          │                                                                    │
│          ▼                                                                    │
│  Step 5: Stream final response to ChatService                                 │
│                                                                               │
└──────────────────────────────────────────────────────────────────────────────┘
                │
                ▼
        Response: "Based on my search, here are patterns..."
        (ACTUAL SEARCH RESULTS INCLUDED)
```

---

## Diagram 3: Code Change Overview

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                         src/services/chat_impl.rs                             │
└──────────────────────────────────────────────────────────────────────────────┘

REMOVE:
─────────────────────────────────────────────────────────────────────────────────
use crate::llm::{LlmClient, ...};

let client = LlmClient::from_profile(&profile)?;

let result = client.request_stream_with_tools(&messages, &mcp_tools, |event| {
    match event {
        LlmStreamEvent::TextDelta(text) => { ... }
        LlmStreamEvent::ToolUse(tool_use) => {
            pending_tool_calls.push(tool_use);  // NEVER EXECUTED!
        }
    }
});
─────────────────────────────────────────────────────────────────────────────────

ADD:
─────────────────────────────────────────────────────────────────────────────────
use serdes_ai::agent::{AgentBuilder, AgentStreamEvent, ModelConfig, RunOptions};

// Build model config
let model_config = ModelConfig::new(&format!("{}:{}", provider, model))
    .with_api_key(&api_key)
    .with_base_url(&base_url);

// Build agent with MCP tools
let agent = AgentBuilder::from_config(model_config)?
    .system_prompt(&system_prompt)
    .toolset(mcp_toolset)  // <-- Tools attached here
    .history_processor(TruncateByTokens::new(context_limit))
    .build()?;

// Run agent stream - IT HANDLES TOOL EXECUTION INTERNALLY
let mut stream = agent.run_stream(user_message, RunOptions::new()).await?;

while let Some(event) = stream.next().await {
    match event? {
        AgentStreamEvent::TextDelta { text } => {
            emit(ChatEvent::TextDelta { text });
        }
        AgentStreamEvent::ToolCallStart { name, .. } => {
            emit(ChatEvent::ToolCallStarted { tool_name: name });
        }
        AgentStreamEvent::ToolCallComplete { name, result, .. } => {
            // Tool was ACTUALLY EXECUTED by the agent!
            emit(ChatEvent::ToolCallCompleted { tool_name: name, result });
        }
        AgentStreamEvent::Complete { .. } => {
            emit(ChatEvent::StreamCompleted { ... });
        }
    }
}
─────────────────────────────────────────────────────────────────────────────────
```

---

## Diagram 4: MCP Tool Integration

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                          MCP TOOL FLOW                                        │
└──────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────┐     ┌─────────────────────┐     ┌─────────────────────┐
│   ChatService       │     │     McpService      │     │   MCP Server        │
│                     │     │                     │     │   (e.g., exa-mcp)   │
└─────────────────────┘     └─────────────────────┘     └─────────────────────┘
         │                           │                           │
         │  get_toolsets()           │                           │
         │ ─────────────────────────►│                           │
         │                           │                           │
         │                           │  (toolsets wrap MCP       │
         │                           │   server connections)     │
         │                           │                           │
         │  Vec<McpToolset>          │                           │
         │ ◄─────────────────────────│                           │
         │                           │                           │
         │                           │                           │
         │  Build Agent with         │                           │
         │  .toolset(mcp_toolset)    │                           │
         │                           │                           │
         │                           │                           │
         │         AGENT RUNS...     │                           │
         │                           │                           │
         │  (Agent needs tool)       │                           │
         │         │                 │                           │
         │         ▼                 │                           │
         │  ┌─────────────────────────────────────────────────┐  │
         │  │              INSIDE AGENT                       │  │
         │  │                                                 │  │
         │  │  toolset.execute("exa_search", args)            │  │
         │  │         │                                       │  │
         │  │         ▼                                       │  │
         │  │  McpToolset sends JSON-RPC to MCP server ──────────►│
         │  │         │                                       │  │ tools/call
         │  │         │                               ◄──────────│
         │  │         │                               (result)│  │
         │  │         ▼                                       │  │
         │  │  Return tool result to agent loop               │  │
         │  │                                                 │  │
         │  └─────────────────────────────────────────────────┘  │
         │                           │                           │
         │                           │                           │
```

---

## Diagram 5: Event Flow

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                          EVENT FLOW (CORRECT)                                 │
└──────────────────────────────────────────────────────────────────────────────┘

Agent                    ChatService                EventBus                 UI
  │                           │                        │                      │
  │ AgentStreamEvent::        │                        │                      │
  │ RunStarted                │                        │                      │
  │ ──────────────────────────►                        │                      │
  │                           │ ChatEvent::            │                      │
  │                           │ StreamStarted          │                      │
  │                           │ ───────────────────────►                      │
  │                           │                        │ Update UI            │
  │                           │                        │ ─────────────────────►
  │                           │                        │                      │
  │ AgentStreamEvent::        │                        │                      │
  │ TextDelta { text }        │                        │                      │
  │ ──────────────────────────►                        │                      │
  │                           │ ChatEvent::            │                      │
  │                           │ TextDelta { text }     │                      │
  │                           │ ───────────────────────►                      │
  │                           │                        │ Append text          │
  │                           │                        │ ─────────────────────►
  │                           │                        │                      │
  │ AgentStreamEvent::        │                        │                      │
  │ ToolCallStart { name }    │                        │                      │
  │ ──────────────────────────►                        │                      │
  │                           │ ChatEvent::            │                      │
  │                           │ ToolCallStarted        │                      │
  │                           │ ───────────────────────►                      │
  │                           │                        │ Show tool indicator  │
  │                           │                        │ ─────────────────────►
  │                           │                        │                      │
  │ (Agent executes tool      │                        │                      │
  │  via MCP toolset)         │                        │                      │
  │                           │                        │                      │
  │ AgentStreamEvent::        │                        │                      │
  │ ToolCallComplete          │                        │                      │
  │ ──────────────────────────►                        │                      │
  │                           │ ChatEvent::            │                      │
  │                           │ ToolCallCompleted      │                      │
  │                           │ ───────────────────────►                      │
  │                           │                        │ Update indicator     │
  │                           │                        │ ─────────────────────►
  │                           │                        │                      │
  │ AgentStreamEvent::        │                        │                      │
  │ TextDelta (more text)     │                        │                      │
  │ ──────────────────────────►                        │                      │
  │                           │                        │                      │
  │ AgentStreamEvent::        │                        │                      │
  │ Complete                  │                        │                      │
  │ ──────────────────────────►                        │                      │
  │                           │ ChatEvent::            │                      │
  │                           │ StreamCompleted        │                      │
  │                           │ ───────────────────────►                      │
  │                           │                        │ Finalize message     │
  │                           │                        │ ─────────────────────►
```

---

## Diagram 6: File Changes

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                          FILES TO CHANGE                                      │
└──────────────────────────────────────────────────────────────────────────────┘

src/services/chat_impl.rs
├── REMOVE: use crate::llm::LlmClient
├── ADD: use serdes_ai::agent::{AgentBuilder, AgentStreamEvent, ModelConfig}
├── REMOVE: LlmClient::from_profile()
├── ADD: AgentBuilder::from_config()
├── REMOVE: client.request_stream_with_tools()
├── ADD: agent.run_stream()
├── CHANGE: Event mapping from LlmStreamEvent to AgentStreamEvent
└── KEEP: EventBus integration, ConversationService integration

src/services/mcp.rs (maybe)
├── Wire trait methods to actual impl
└── Or create toolset conversion helper

tests/e2e_agent_mode.rs (NEW)
├── Test agent mode with real API
├── Verify AgentStreamEvent types received
└── Verify tool_use results in tool execution (if tools available)

tests/e2e_mcp_catalog.rs (NEW)
├── Test fetching real MCP catalog
├── Verify can search for tools
└── Verify can get tool details
```

---

## Diagram 7: Verification Checkpoints

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                       VERIFICATION CHECKPOINTS                                │
└──────────────────────────────────────────────────────────────────────────────┘

PHASE 1: Agent Mode in ChatService
────────────────────────────────────────────────────────────────────────────────
$ grep -n "AgentBuilder" src/services/chat_impl.rs
src/services/chat_impl.rs:XX:use serdes_ai::agent::AgentBuilder;
src/services/chat_impl.rs:YY:    let agent = AgentBuilder::from_config(...)
                            ▲
                            │
                    MUST HAVE MATCHES

$ grep -n "model.request_stream" src/services/chat_impl.rs
(no output)
            ▲
            │
    MUST BE EMPTY (no raw model usage)


PHASE 2: MCP Toolset Attachment
────────────────────────────────────────────────────────────────────────────────
$ grep -n "\.toolset(" src/services/chat_impl.rs
src/services/chat_impl.rs:XX:    .toolset(mcp_toolset)
                            ▲
                            │
                    MUST HAVE MATCHES


PHASE 3: E2E Tests
────────────────────────────────────────────────────────────────────────────────
$ cargo test --test e2e_agent_mode -- --ignored --nocapture
running 1 test
=== E2E Test: Agent Mode ===
Agent built with ModelConfig
Agent has 3 tools attached       <── If MCPs configured
AgentStreamEvent::TextDelta received
Response: "Hello from agent mode test"
test test_agent_mode_real ... ok
                            ▲
                            │
                    MUST PASS WITH REAL API CALL


FINAL: All Checks Pass
────────────────────────────────────────────────────────────────────────────────
[x] grep "AgentBuilder" has matches
[x] grep "model.request_stream" is empty
[x] grep ".toolset(" has matches
[x] grep "AgentStreamEvent" has matches
[x] cargo build passes
[x] cargo test --lib passes
[x] E2E test with real API passes
[x] Code review confirms agent handles tools
```
