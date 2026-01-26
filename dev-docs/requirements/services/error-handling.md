# Service Error Handling Requirements

This document defines the shared error contract, service-specific codes, validation rules, and UI mappings. All services must use this contract.

---

## Standard Error Contract

All service failures return `ServiceError`:

```json
{ "code": "string", "message": "string", "field": "string" }
```

- `code` is a stable, machine-readable value.
- `message` is user-facing and should be actionable.
- `field` is optional and used for form validation errors.

### Common Error Codes

| Code | Meaning | UI Default |
|------|---------|------------|
| VALIDATION_ERROR | Input invalid | Inline field error + banner if needed |
| NOT_FOUND | Resource missing | Show "not found" state |
| UNAUTHORIZED | Invalid credentials | Auth error banner + settings CTA |
| FORBIDDEN | Permission denied | Error banner, no retry |
| CONFLICT | Illegal state transition | Non-blocking warning |
| RATE_LIMITED | Provider throttled | Retry enabled, show wait hint |
| NETWORK_ERROR | Connection failed or timeout | Retry enabled, offline hint |
| SERVICE_UNAVAILABLE | Dependency or provider failure | Retry enabled, status notice |
| INTERNAL_ERROR | Unexpected failure | Generic error banner |

---

## Service-Specific Error Codes

### ConversationService

| Code | When | Notes |
|------|------|-------|
| VALIDATION_ERROR | Invalid UUID, empty message content, title too long | field set when applicable |
| NOT_FOUND | Conversation missing on delete | load missing returns None (not error) |
| SERVICE_UNAVAILABLE | Disk IO failure, parse error, disk full | Do not corrupt files |

### ChatService

| Code | When | Notes |
|------|------|-------|
| VALIDATION_ERROR | Empty user message, missing profile fields | field set for UI inputs |
| NOT_FOUND | Conversation or profile missing | No stream started |
| UNAUTHORIZED | Provider auth rejected | Show “Invalid API key” |
| RATE_LIMITED | Provider 429 | Retry after delay |
| NETWORK_ERROR | Stream/connect timeout | Retry enabled |
| SERVICE_UNAVAILABLE | Provider error, parse error, tool timeout | Stream ends with Error event |
| CONFLICT | Cancel already completed stream | No UI change |

### ProfileService

| Code | When | Notes |
|------|------|-------|
| VALIDATION_ERROR | Missing name/provider/model/auth, invalid URL | field set for inputs |
| NOT_FOUND | Update/delete unknown profile | No changes persisted |
| CONFLICT | Delete last profile | Keep default intact |
| SERVICE_UNAVAILABLE | Config write failure | Prior config preserved |

### McpService

| Code | When | Notes |
|------|------|-------|
| VALIDATION_ERROR | Missing name/package/transport/env vars | field set for inputs |
| NOT_FOUND | Unknown MCP id | Tool not executed |
| CONFLICT | Start disabled MCP | Status unchanged |
| NETWORK_ERROR | Handshake timeout, tool call timeout | Retry enabled |
| SERVICE_UNAVAILABLE | Docker down, process crash, tool failure | Status=Error |

### ModelsRegistryService

| Code | When | Notes |
|------|------|-------|
| VALIDATION_ERROR | Query too long, empty provider_id | field set for inputs |
| NETWORK_ERROR | Refresh failed with no cache | Providers empty |
| SERVICE_UNAVAILABLE | Parse error for required fields | Exclude invalid models |

---

## Validation Rules

### ConversationService

| Field | Rule | Error Code | Error Message |
|-------|------|------------|---------------|
| id | Must be valid UUID | VALIDATION_ERROR | Invalid conversation id |
| title | <= 120 chars | VALIDATION_ERROR | Title must be 120 chars or less |
| message.role | user/assistant/system | VALIDATION_ERROR | Invalid message role |
| message.content | Non-empty after trim | VALIDATION_ERROR | Message content required |

### ChatService

| Field | Rule | Error Code | Error Message |
|-------|------|------------|---------------|
| conversation_id | Must exist | NOT_FOUND | Conversation not found |
| user_message | Non-empty after trim | VALIDATION_ERROR | Message cannot be empty |
| profile.id | Must exist | NOT_FOUND | Profile not found |
| profile.model_id | Non-empty | VALIDATION_ERROR | Model is required |
| profile.provider_id | Non-empty | VALIDATION_ERROR | Provider is required |

### ProfileService

| Field | Rule | Error Code | Error Message |
|-------|------|------------|---------------|
| name | Non-empty after trim | VALIDATION_ERROR | Name is required |
| provider_id | Non-empty | VALIDATION_ERROR | Provider is required |
| model_id | Non-empty | VALIDATION_ERROR | Model is required |
| api_key + keyfile_path | At least one provided | VALIDATION_ERROR | Auth is required |
| base_url | Valid URL if present | VALIDATION_ERROR | Base URL invalid |

### McpService

| Field | Rule | Error Code | Error Message |
|-------|------|------------|---------------|
| name | Non-empty after trim | VALIDATION_ERROR | Name is required |
| package.identifier | Non-empty | VALIDATION_ERROR | Package identifier required |
| transport | Required | VALIDATION_ERROR | Transport required |
| env_vars | Required secrets present | VALIDATION_ERROR | Missing required credentials |

### ModelsRegistryService

| Field | Rule | Error Code | Error Message |
|-------|------|------------|---------------|
| query | <= 200 chars | VALIDATION_ERROR | Query too long |
| provider_id | Non-empty | VALIDATION_ERROR | Provider required |

---

## UI State Mapping

| Error Code | UI State | Guidance |
|-----------|----------|----------|
| VALIDATION_ERROR | Inline validation + optional banner | Highlight `field`, keep user input |
| NOT_FOUND | Empty state | Offer create or refresh action |
| UNAUTHORIZED | Auth error | Link to profile/MCP settings |
| FORBIDDEN | Locked state | Disable retry, show contact admin |
| CONFLICT | Warning toast | No destructive rollback |
| RATE_LIMITED | Retry with cooldown | Show timer if available |
| NETWORK_ERROR | Offline state | Retry button, keep drafts |
| SERVICE_UNAVAILABLE | Degraded state | Retry button, show status hint |
| INTERNAL_ERROR | Generic error | Log for diagnostics, offer retry |
