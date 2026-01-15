# MCP Implementation - Executive Summary

## What We're Building

Adding **MCP (Model Context Protocol)** support to PersonalAgent, enabling the AI agent to use external tools like GitHub, file systems, web search, and more.

## Timeline

**Total: 10-13 weeks (48-65 days)**

| Milestone | Duration | Deliverable |
|-----------|----------|-------------|
| **M1: Foundation** | Week 1-2 | SerdesAI PR (draft) + Data models |
| **M2: Core Spawning** | Week 3-4 | MCP process management |
| **M3: Basic UI** | Week 5-6 | Add/Configure MCP screens |
| **M4: Agent Integration** | Week 7-8 | Tool routing + Chat display |
| **M5: Registry Search** | Week 9-10 | Official + Smithery search |
| **M6: Production Ready** | Week 11-13 | Error recovery, status indicators |

## Phase 0 (Immediate): SerdesAI PR

**BLOCKER** - Must create before we can inject credentials into MCP servers.

**Deliverable**: Draft PR to SerdesAI adding `spawn_with_env()` to `StdioTransport`

```rust
// What we need to add
pub async fn spawn_with_env(
    command: &str,
    args: &[&str],
    env: HashMap<String, String>,  // <-- NEW: inject API keys here
) -> McpResult<Self>
```

**Action Items**:
1. Fork SerdesAI locally (research/serdesAI/)
2. Add `spawn_with_env()` to `serdes-ai-mcp/src/transport.rs`
3. Add 4 unit tests
4. Create draft PR on upstream repo
5. Keep as draft until we use it in PersonalAgent

## Testable Milestones

### M1: Foundation (Week 1-2)
**Success Criteria**:
- [ ] `cargo test` passes for new MCP types
- [ ] Config loads/saves with `mcps` array
- [ ] Secrets stored with `chmod 600` permissions
- [ ] Auth type detected from registry metadata

### M2: Core Spawning (Week 3-4)
**Success Criteria**:
- [ ] `McpManager::start()` spawns real MCP server
- [ ] Environment variables injected correctly
- [ ] Multiple env vars per MCP works (AWS example)
- [ ] Spawn failures handled gracefully

### M3: Basic UI (Week 5-6)
**Success Criteria**:
- [ ] Click "+" in Settings → Add MCP screen appears
- [ ] Enter `npx -y @github/mcp-server` → parses correctly
- [ ] Configure screen shows API key input
- [ ] Save → MCP appears in Settings list

### M4: Agent Integration (Week 7-8)
**Success Criteria**:
- [ ] Send message → Agent can call MCP tool
- [ ] Tool call shows spinner in chat
- [ ] Tool result displayed in chat
- [ ] System prompt includes available tools

### M5: Registry Search (Week 9-10)
**Success Criteria**:
- [ ] Select "Official" registry → Search works
- [ ] Select "Smithery" registry → Search works
- [ ] Select "Both" → Results merged, deduplicated
- [ ] Click search result → Populates configure screen

### M6: Production Ready (Week 11-13)
**Success Criteria**:
- [ ] MCP crash → Auto-restart (max 3 times)
- [ ] 30 min idle → MCP cleaned up
- [ ] Status badges show Connected/Idle/Error
- [ ] Delete MCP → Credentials cleaned up

## Deferred (Phase 7+)

- **OAuth authentication** - API key first
- **HTTP transport MCPs** - stdio first
- **configSchema dynamic UI** - JSON editor fallback
- **Tool filtering** - All tools enabled by default

## Key Files

| File | Purpose |
|------|---------|
| `project-plans/mcps/SPEC.md` | Full specification |
| `project-plans/mcps/UI_FLOW.md` | Wireframes |
| `project-plans/mcps/IMPLEMENTATION_PLAN.md` | Detailed test-first plan |
| `research/serdesAI/` | SerdesAI fork for PR |

## Next Actions

1. **Today**: Create draft PR for SerdesAI env var support
2. **This week**: Start Phase 1 data models (parallel with PR)
3. **Week 2**: Complete Phase 1, start Phase 2 if PR ready
