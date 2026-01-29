# PersonalAgent - Application Requirements


## Service Boundaries

- UI requirements live under `dev-docs/requirements/ui/` and define rendering and view-state behavior.
- Service requirements live under `dev-docs/requirements/services/` and define contracts, validation, persistence, and error shapes.
- This document captures application-level capabilities and non-UI behaviors only.

## Overview

PersonalAgent is a macOS menu bar application that provides a conversational AI interface. It lives in the system tray (status bar), displays a popover when clicked, and allows users to chat with various LLM providers through a unified interface with tool/MCP support.

## Canonical Data Model Glossary

See `dev-docs/requirements/data-models.md` for canonical definitions and ownership. All services and UI requirements must use these terms without drift.

---

## Core Application Features

### 1. Menu Bar Integration

| ID | Requirement | Status |
|----|-------------|--------|
| MB-1 | Display persistent icon in macOS menu bar (status area) | [OK] |
| MB-2 | Show NSPopover when tray icon is clicked | [OK] |
| MB-3 | Popover should dismiss when clicking outside | [OK] |
| MB-4 | Popover should auto-position relative to menu bar | [OK] |
| MB-5 | Support global hotkey to summon/dismiss popover | WARNING: Partial |
| MB-6 | App should launch quickly (<1 second to show icon) | [OK] |

### 2. View Navigation

| ID | Requirement | Status |
|----|-------------|--------|
| VN-1 | Chat View is the default/main view | [OK] |
| VN-2 | Navigate to Settings via gear button | [OK] |
| VN-3 | Navigate to History via "H" button | [OK] |
| VN-4 | Navigate to Model Selector via + Add Profile | [OK] |
| VN-5 | Navigate to Profile Editor via Edit button | [OK] |
| VN-6 | Navigate to MCP Add View via + Add MCP | [OK] |
| VN-7 | Navigate to MCP Configure View via Edit MCP | [OK] |
| VN-8 | Back navigation from all sub-views | [OK] |
| VN-9 | Consistent top bar across all views | [OK] |

### 3. Theme & Appearance

| ID | Requirement | Status |
|----|-------------|--------|
| TH-1 | Dark mode as default theme | [OK] |
| TH-2 | Consistent color palette across views | [OK] |
| TH-3 | Native macOS look and feel | WARNING: Partial |
| TH-4 | User message bubbles right-aligned with green tint | [OK] |
| TH-5 | Assistant message bubbles left-aligned with dark tint | [OK] |
| TH-6 | Thinking content displayed in collapsible section | WARNING: Partial |

---

## Conversation Management

### 4. Conversation Lifecycle

| ID | Requirement | Status |
|----|-------------|--------|
| CL-1 | Create new conversation via + button | [OK] |
| CL-2 | New conversation gets default title "New YYYY-MM-DD HH:MM" | [OK] |
| CL-3 | User messages persisted immediately after sending | [OK] |
| CL-4 | Assistant responses persisted after streaming completes | [OK] |
| CL-5 | Conversation title editable via double-click or rename | WARNING: Partial |
| CL-6 | Conversation deletable from History view | [OK] |
| CL-7 | Conversations stored in ~/Library/Application Support/PersonalAgent/conversations/ | [OK] |
| CL-8 | Messages stored in `.jsonl` file (append-only) | [PLANNED] |
| CL-9 | Metadata stored in `.meta.json` file (small, rewritable) | [PLANNED] |
| CL-10 | Filename format: YYYYMMDDHHMMSS###.jsonl + .meta.json | [PLANNED] |

### 5. Conversation Switching

| ID | Requirement | Status |
|----|-------------|--------|
| CS-1 | Title dropdown shows all saved conversations | [OK] |
| CS-2 | Selecting conversation from dropdown loads it | [OK] |
| CS-3 | Current conversation highlighted in dropdown | [OK] |
| CS-4 | Conversation messages restored from storage on load | [OK] |
| CS-5 | Message history displayed in chat area on load | [OK] |

### 6. Message Handling

| ID | Requirement | Status |
|----|-------------|--------|
| MH-1 | User can type message in input field | [OK] |
| MH-2 | Send message via Enter key | [OK] |
| MH-3 | Send message via Send button | [OK] |
| MH-4 | Input field clears after sending | [OK] |
| MH-5 | User message appears immediately in chat | [OK] |
| MH-6 | Assistant response streams in real-time | [OK] |
| MH-7 | Streaming indicator (▌) shows during response | [OK] |
| MH-8 | Thinking content streams if enabled | WARNING: Broken |
| MH-9 | Cancel streaming via button or Escape | [ERROR] Not impl |

---

## Model Profile Management

### 7. Profile CRUD

| ID | Requirement | Status |
|----|-------------|--------|
| PR-1 | Create new profile via Model Selector flow | [OK] |
| PR-2 | Edit existing profile via Profile Editor | [OK] |
| PR-3 | Delete profile with confirmation | [OK] |
| PR-4 | Set profile as default/active | [OK] |
| PR-5 | Active profile visually indicated | WARNING: Partial |
| PR-6 | Profile includes: name, provider, model, API key | [OK] |
| PR-7 | Profile includes: temperature, max_tokens, top_p | [OK] |
| PR-8 | Profile includes: enable_thinking, show_thinking | [OK] |
| PR-9 | Profile includes: thinking_budget | [OK] |
| PR-10 | Profile includes: custom system_prompt | [OK] |
| PR-11 | System prompt sent to LLM with each request | [OK] |
| PR-12 | Default system prompt: "You are a helpful assistant." | [OK] |

### 8. Model Selection

| ID | Requirement | Status |
|----|-------------|--------|
| MS-1 | Fetch models from models.dev API (https://models.dev/api.json) | [OK] |
| MS-2 | Parse provider structure: id, name, api URL, models list | [OK] |
| MS-3 | Parse model fields: id, name, tool_call, reasoning, cost, etc. | [OK] |
| MS-4 | Display models grouped by provider | [OK] |
| MS-5 | Filter/search models by name or ID | WARNING: Partial |
| MS-6 | Show model capabilities (tools, reasoning, vision, structured_output) | [OK] |
| MS-7 | Show model cost info (input/output per 1M tokens) | [OK] |
| MS-8 | Manual provider/model/base_url entry for custom endpoints | [OK] |
| MS-9 | Cache models.dev data locally (~/.../cache/models.json) | [OK] |
| MS-10 | Refresh models from API on demand | [OK] |
| MS-11 | Auto-populate base_url from provider data | [OK] |

### 9. Thinking Mode

| ID | Requirement | Status |
|----|-------------|--------|
| TM-1 | Toggle thinking on/off via "T" button | [OK] |
| TM-2 | Visual indicator when thinking is enabled | [OK] |
| TM-3 | Thinking content displayed if show_thinking=true | WARNING: Broken |
| TM-4 | Thinking displayed in collapsible section | WARNING: Broken |
| TM-5 | Extended thinking budget passed to API | [OK] |
| TM-6 | Thinking content persisted with message | [OK] |

---

## MCP (Model Context Protocol) Integration

### 10. MCP Configuration

| ID | Requirement | Status |
|----|-------------|--------|
| MC-1 | Add MCP via search or URL | [OK] |
| MC-2 | Search Official MCP registry (registry.modelcontextprotocol.io) | [OK] |
| MC-3 | Search Smithery registry (registry.smithery.ai) | [OK] |
| MC-4 | Manual MCP entry via URL (npx command, docker image, or HTTP URL) | [OK] |
| MC-5 | Configure MCP credentials (API key, keyfile, OAuth) | [OK] |
| MC-6 | Enable/disable MCPs via toggle | [OK] |
| MC-7 | Edit MCP configuration | [OK] |
| MC-8 | Delete MCP with confirmation | [OK] |
| MC-9 | Store MCP credentials securely | [OK] |
| MC-10 | Detect auth requirements from registry metadata (env vars) | [OK] |

### 11. MCP Runtime

| ID | Requirement | Status |
|----|-------------|--------|
| MR-1 | Spawn MCP processes on demand | [OK] |
| MR-2 | Pass credentials via environment variables | [OK] |
| MR-3 | Support stdio transport (npx, docker) | [OK] |
| MR-4 | Support HTTP transport (hosted MCP servers) | [OK] |
| MR-5 | Display MCP status (running, error, disabled) | WARNING: Partial |
| MR-6 | Tools from enabled MCPs available to agent | [OK] |
| MR-7 | Tool execution results displayed in chat | [OK] |
| MR-8 | Graceful MCP shutdown on app quit | [OK] |
| MR-9 | Agent can invoke MCP tools during conversation | [OK] |
| MR-10 | Tool results fed back to LLM for continued response | [OK] |
| MR-11 | Multiple MCPs can be enabled simultaneously | [OK] |
| MR-12 | Individual MCP failure does not block other MCPs | [OK] |
| MR-13 | Failed MCP shows error status in Settings UI | WARNING: Partial |
| MR-14 | Working MCPs continue to provide tools when one fails | [OK] |

---

## Data Persistence

### 12. Configuration Storage

| ID | Requirement | Status |
|----|-------------|--------|
| DS-1 | Config stored in ~/Library/Application Support/PersonalAgent/config.json | [OK] |
| DS-2 | Config includes: profiles, mcps, default_profile, hotkey | [OK] |
| DS-3 | Config auto-saved on changes | [OK] |
| DS-4 | Config loaded on app startup | [OK] |
| DS-5 | Missing config creates defaults | [OK] |
| DS-6 | Config version for migrations | WARNING: Partial |

### 13. Secrets Storage

| ID | Requirement | Status |
|----|-------------|--------|
| SS-1 | API keys stored securely | WARNING: File-based |
| SS-2 | OAuth tokens stored and refreshed | [OK] |
| SS-3 | Keyfile paths supported | [OK] |
| SS-4 | Secrets not logged | [OK] |
| SS-5 | macOS Keychain integration | [ERROR] Future |


## End-to-End Flows (Cross-Service)

### Send Message with Persistence and Context Compression

1. UI sends user message via ChatService.send_message(conversation_id, content, profile_id).
2. ChatService validates input and calls ConversationService.append_message(user message).
3. ChatService loads conversation + ContextStrategy.build_context(...).
4. ChatService builds model request and starts provider stream.
5. On stream completion, ChatService calls ConversationService.append_message(assistant message).
6. If ContextStrategy produced new ContextState, ChatService calls ConversationService.update_context_state(...).
7. Persistence is complete only after both message append calls succeed.
8. Retry: if append of assistant message fails, retry up to 2 times with backoff (200ms, 500ms).
9. Rollback: if user message append succeeded but streaming fails before any assistant output, append a system message noting failure and keep conversation consistent.

### Create Conversation and First Message

1. UI triggers ConversationService.create().
2. ConversationService persists empty .jsonl and .meta.json.
3. UI selects conversation and calls ChatService.send_message(...).
4. On failure to create conversation, UI shows error in #error-banner and keeps previous conversation selected.

### Delete Conversation

1. UI confirms deletion.
2. ConversationService.delete(id) removes .jsonl and .meta.json.
3. UI removes row and clears selection.
4. If delete fails, UI shows error in #error-banner and keeps row visible.

---

## Error Handling & UX

### Standard Error Contract

All services return errors using a consistent shape so UI can map to stable states.

```json
{ "code": "string", "message": "string", "field": "string" }
```

- `code` is required and stable for test assertions.
- `message` is user-presentable.
- `field` is optional and used for validation errors tied to a specific field.

### Error Code → UI State Mapping

| Error Code | UI State | UI Behavior |
|------------|----------|-------------|
| VALIDATION_ERROR | Inline field error | Highlight field, show message in field error label |
| NOT_FOUND | Empty state | Show empty state with message in #empty-state-label |
| CONFLICT | Banner error | Show non-blocking banner in #error-banner |
| UNAUTHORIZED | Inline auth error | Show "Invalid credentials" in #error-banner |
| NETWORK_ERROR | Inline error | Show "Network error" in #error-banner, allow retry |
| RATE_LIMITED | Inline error | Show "Rate limited" in #error-banner, disable send 30s |
| SERVICE_UNAVAILABLE | Inline error | Show "Service unavailable" in #error-banner |
| STREAM_CANCELLED | Inline info | Show "Streaming stopped" in #info-banner |

### 14. Error States

| ID | Requirement | Status |
|----|-------------|--------|
| ER-1 | Display error when no profile configured | [OK] |
| ER-2 | Display API errors in chat | [OK] |
| ER-3 | Display MCP connection errors | WARNING: Partial |
| ER-4 | Display network errors gracefully | [OK] |
| ER-5 | Confirmation dialogs for destructive actions | [OK] |

### 15. Accessibility

| ID | Requirement | Status |
|----|-------------|--------|
| AC-1 | Keyboard navigation support | WARNING: Partial |
| AC-2 | Screen reader compatibility | [ERROR] Not tested |
| AC-3 | Keyboard shortcuts for common actions | WARNING: Partial |
| AC-4 | Focus management between views | WARNING: Partial |

---

## Context Management

### 16. Context Window Strategy

| ID | Requirement | Status |
|----|-------------|--------|
| CM-1 | Pluggable context management strategy interface | [PLANNED] |
| CM-2 | Default: Sandwich strategy (preserve top 5, bottom 5, compress middle) | [PLANNED] |
| CM-3 | Trigger compression at 70% of model's context limit | [PLANNED] |
| CM-4 | Store full message history in `.jsonl` (user sees complete conversation) | [PLANNED] |
| CM-5 | Store compressed summary in `.meta.json` | [PLANNED] |
| CM-6 | Cache summary with range metadata (which messages it covers) | [PLANNED] |
| CM-7 | Reuse cached summary on reload if still valid | [PLANNED] |
| CM-8 | Re-compress when middle section grows beyond cached range | [PLANNED] |
| CM-9 | Summary invisible to user, only affects what model sees | [PLANNED] |

---

## Performance Requirements

| ID | Requirement | Target | Status |
|----|-------------|--------|--------|
| PF-1 | App launch time | <1s | [OK] |
| PF-2 | Memory when idle | <100MB | [OK] |
| PF-3 | UI responsive during streaming | Yes | [OK] |
| PF-4 | MCP cold start | <5s | [OK] |
| PF-5 | Tool call timeout | 30s default | [OK] |

---

## Legend

- [OK] Implemented and working
- WARNING: Partial or has known issues
- [ERROR] Not implemented
- Future - Planned for future release
