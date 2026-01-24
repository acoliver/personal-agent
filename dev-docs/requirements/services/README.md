# Services Layer Overview

This directory contains requirements for PersonalAgent's service layer. Services encapsulate business logic and coordinate between the UI layer and infrastructure (storage, external APIs, SerdesAI).

---

## Architecture Position

```
┌─────────────────────────────────────────────────────────────────┐
│                         UI LAYER                                 │
│  Views are purely presentational - render data, forward actions  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      SERVICES LAYER                              │
│  Business logic, orchestration, state management                 │
│                                                                  │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐                │
│  │ ChatService │ │ ProfileSvc  │ │ McpService  │ ...            │
│  └─────────────┘ └─────────────┘ └─────────────┘                │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   INFRASTRUCTURE LAYER                           │
│  Storage, external APIs, SerdesAI Agent                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## Service Catalog

### Core Services

| Service | File | Primary Responsibility |
|---------|------|----------------------|
| **AppSettingsService** | [app-settings.md](app-settings.md) | Global app settings (default profile, current conversation, hotkey) |
| **ChatService** | [chat.md](chat.md) | Orchestrate LLM interactions via SerdesAI Agent |
| **ConversationService** | [conversation.md](conversation.md) | Persist and retrieve conversations |
| **ProfileService** | [profile.md](profile.md) | Manage model profiles (CRUD only, not default selection) |

### Registry Services

| Service | File | Primary Responsibility |
|---------|------|----------------------|
| **ModelsRegistryService** | [models-registry.md](models-registry.md) | Fetch and cache model info from models.dev |
| **McpRegistryService** | [mcp-registry.md](mcp-registry.md) | Search Official/Smithery MCP registries |

### MCP Services

| Service | File | Primary Responsibility |
|---------|------|----------------------|
| **McpService** | [mcp.md](mcp.md) | Manage running MCP server instances |

### Security Services

| Service | File | Primary Responsibility |
|---------|------|----------------------|
| **SecretsService** | [secrets.md](secrets.md) | Secure credential storage |

### Context Management

| Service | File | Status |
|---------|------|--------|
| **Context Strategy** | [context.md](context.md) | **Superseded** - SerdesAI HistoryProcessor |

---

## Service Dependencies

```
ChatService
    ├── ProfileService (model config WITH resolved API key)
    ├── ConversationService (message history, persistence)
    └── McpService (toolsets for Agent)

ProfileService
    └── SecretsService (API key storage, resolution)

McpService
    └── SecretsService (env var secrets)

McpRegistryService
    └── (HTTP client for Smithery/Official MCP registries)

ModelsRegistryService
    └── (HTTP client for models.dev, local cache)

ConversationService
    └── (file system storage only - no other service deps)

SecretsService
    └── (encrypted file storage, future: Keychain)

AppSettingsService
    └── (file system storage only - settings.json)
```

**Key Design:** 
- ChatService does NOT call SecretsService directly. API keys are resolved through ProfileService.get_model_config().
- ProfileService does NOT manage which profile is "default". That's in AppSettingsService.
- Current conversation selection is in AppSettingsService, not ConversationService.

---

## Key Design Decisions

### 1. ChatService Uses SerdesAI Agent Mode

Instead of manual tool execution loops in the UI, ChatService builds and runs a SerdesAI Agent that handles:
- LLM communication
- Tool execution with retry logic
- Streaming events
- Context compression via HistoryProcessor

See [chat.md](chat.md) and [../architecture/chat-flow.md](../architecture/chat-flow.md).

### 2. Profile is Global, Not Per-Conversation

The "selected profile" is a global app setting:
- ProfileService manages which profile is default (selected)
- Conversations do NOT store `profile_id`
- Changing profile affects ALL conversations (existing and new)
- Each assistant MESSAGE stores its `model_id` for historical record
- Loading an old conversation uses the currently selected profile for new messages

This simplifies the model and matches user mental model: "I'm talking to Claude" not "this conversation is locked to Claude".

### 2. McpService Owns Toolsets

MCPs are managed centrally by McpService. ChatService requests toolsets via `get_toolsets()` and attaches them to the Agent. This allows:
- Shared MCPs across conversations
- Central lifecycle management
- Status visibility in Settings

See [mcp.md](mcp.md).

### 3. Context Compression via HistoryProcessor

Context management is handled by SerdesAI's built-in `TruncateByTokens` processor, configured with the profile's context limit. No custom ContextService needed.

See [context.md](context.md).

### 4. Secrets Separated from Profiles/MCPs

Sensitive data (API keys, OAuth tokens) is stored by SecretsService, not embedded in profile/MCP config files. This allows:
- Secure storage (encrypted files, future Keychain)
- Easy credential rotation
- Clean deletion when resources are removed

**Important:** Only ProfileService and McpService call SecretsService. ChatService gets API keys through `ProfileService.get_model_config()`.

See [secrets.md](secrets.md).

### 5. McpRegistryService Separate from McpService

Discovery/search (McpRegistryService) is separate from runtime management (McpService):
- McpRegistryService: HTTP calls to registry APIs
- McpService: Process spawning, tool execution

See [mcp-registry.md](mcp-registry.md) and [mcp.md](mcp.md).

---

## UI → Service Mapping

### Chat View

| UI Action | Service | Method |
|-----------|---------|--------|
| Send message | ChatService | `send_message(conv_id, text)` |
| Cancel streaming | ChatService | `cancel(handle)` |
| Load conversation | ConversationService | `load(id)` |
| Create conversation | ConversationService | `create()` |
| Rename | ConversationService | `rename(id, title)` |
| List conversations | ConversationService | `list()` |
| Get default profile | AppSettingsService | `get_default_profile_id()` then ProfileService.`get(id)` |
| Get current conversation | AppSettingsService | `get_current_conversation_id()` |
| Switch conversation | AppSettingsService | `set_current_conversation_id(id)` |

**Note:** The [T] toggle for show_thinking is runtime-only view state. It does not persist.

### Settings View

| UI Action | Service | Method |
|-----------|---------|--------|
| List profiles | ProfileService | `list()` |
| Set default profile | AppSettingsService | `set_default_profile_id(id)` |
| Delete profile | ProfileService | `delete(id)` + AppSettingsService.`clear_default_profile()` if needed |
| List MCPs | McpService | `list()` + `all_status()` |
| Start/Stop MCP | McpService | `start(id)` / `stop(id)` |
| Delete MCP | McpService | `delete(id)` |

### Profile Editor

| UI Action | Service | Method |
|-----------|---------|--------|
| Load profile | ProfileService | `get(id)` |
| Save profile | ProfileService | `create()` / `update(id, ...)` |
| Test connection | ProfileService | `test_connection(id)` |

### MCP Add View

| UI Action | Service | Method |
|-----------|---------|--------|
| Search MCPs | McpRegistryService | `search(query, source)` |
| Get MCP details | McpRegistryService | `get_details(source)` |

### MCP Configure View

| UI Action | Service | Method |
|-----------|---------|--------|
| Save MCP | McpService | `add(config)` |
| Store secret | SecretsService | `store(key, value)` |
| Start OAuth | SecretsService | `exchange_oauth_code(...)` |

---

## Stream Events

ChatService emits clean, UI-ready events:

| Event | Purpose |
|-------|---------|
| `Started { model_id }` | Stream began, show model label |
| `TextDelta { content }` | Append to assistant bubble |
| `ThinkingDelta { content }` | Append to thinking section (always stored) |
| `ToolStart { id, name }` | Show tool indicator |
| `ToolComplete { id, name, success, summary }` | Update tool indicator |
| `Complete { text, thinking, tool_calls, model_id }` | Finalize message |
| `Cancelled { partial_text, partial_thinking, model_id }` | Show partial + marker |
| `Error { message, retryable }` | Show error |

**Note:** Thinking content is always stored in messages even if `show_thinking` is disabled. The UI toggle only controls display, not storage.

---

## Storage Locations

```
~/Library/Application Support/PersonalAgent/
├── settings.json                      # Global app settings (default profile, current conversation, hotkey)
├── conversations/
│   ├── {timestamp}{random}.jsonl      # Messages (append-only, includes model_id per assistant msg)
│   └── {timestamp}{random}.meta.json  # Metadata (no profile_id - profile is global)
├── profiles/
│   └── {uuid}.json                    # Profile config (no is_default - that's in settings.json)
├── mcps/
│   └── {uuid}.json                    # MCP config
├── secrets/
│   └── {type}_{uuid}_{name}.enc       # Encrypted secrets
└── cache/
    ├── models-registry.json           # models.dev cache (24h TTL)
    └── mcp-registry-*.json            # MCP registry cache
```

---

## Test Strategy

Each service has its own test requirements (see individual files). Key testing themes:

1. **Unit tests**: Service methods in isolation with mocked dependencies
2. **Integration tests**: Service interactions (e.g., ChatService → ConversationService)
3. **Contract tests**: SerdesAI Agent integration
4. **Storage tests**: File format compatibility, atomic writes, corruption handling
