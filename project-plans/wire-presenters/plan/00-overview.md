# Plan Overview: Wire Presenters to EventBus

**Plan ID**: PLAN-20250128-PRESENTERS
**Status**: Active
**Created**: 2025-01-28
**Total Phases**: 12 (1 preflight + 5 implementation + 6 verification)

## Objective

Wire all presentation layer components to the event bus for real-time state updates. Per dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md section "Presentation Layer Isolation", presenters must react to events from the domain layer (chat, MCP, profile services) without direct dependencies.

## Architecture Context

This plan implements the event-driven architecture specified in:
- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` sections "Event System" and "Presentation Layer Isolation"
- `dev-docs/requirements/presentation.md` requirements for reactive presenters
- `src/events/types.rs` defining all event enums (AppEvent, UserEvent, ChatEvent, McpEvent, etc.)

**Key architectural constraints:**
1. Presenters subscribe to `AppEvent` via the event bus using `bus.subscribe()` which returns `broadcast::Receiver<AppEvent>`
2. Presenters emit `ViewCommand` to update UI state
3. No direct service calls from presenters - only events
4. All state changes flow through event handlers

## Event Types Reference

**Source file**: `src/events/types.rs`

**NOTE**: P00 Preflight will verify these types exist. This section documents the SUBSET of events used by this plan. For full definitions, see source file.

### Events Used by ChatPresenter

| Variant Name | Full Fields | Line | Purpose |
|--------------|-------------|------|---------|
| `StreamStarted` | `{ conversation_id: Uuid, message_id: Uuid, model_id: String }` | 164 | Show loading |
| `TextDelta` | `{ text: String }` | 171 | Append text |
| `ThinkingDelta` | `{ text: String }` | 174 | Append thinking |
| `ToolCallStarted` | `{ tool_call_id: String, tool_name: String }` | 177 | Show tool UI |
| `ToolCallCompleted` | `{ tool_call_id: String, tool_name: String, success: bool, result: String, duration_ms: u64 }` | 183 | Update tool |
| `StreamCompleted` | `{ conversation_id: Uuid, message_id: Uuid, total_tokens: Option<u32> }` | 192 | Hide loading |
| `StreamCancelled` | `{ conversation_id: Uuid, message_id: Uuid, partial_content: String }` | 199 | Show cancelled |
| `StreamError` | `{ conversation_id: Uuid, error: String, recoverable: bool }` | 206 | Show error |
| `MessageSaved` | `{ conversation_id: Uuid, message_id: Uuid }` | 213 | Confirm save |
| `SendMessage` | `{ text: String }` | 49 | User sent |
| `StopStreaming` | (unit) | 52 | User stop |
| `NewConversation` | (unit) | 55 | New chat |
| `ToggleThinking` | (unit) | 61 | Toggle UI |
| `ConfirmRenameConversation` | `{ id: Uuid, title: String }` | 67 | Rename |

### Events Used by HistoryPresenter

| Variant Name | Full Fields | Line | Purpose |
|--------------|-------------|------|---------|
| `SelectConversation` | `{ id: Uuid }` | 58 | User selected |
| `Created` | `{ id: Uuid, title: String }` | 325 | New convo |
| `TitleUpdated` | `{ id: Uuid, title: String }` | 331 | Title changed |
| `Deleted` | `{ id: Uuid }` | 334 | Convo deleted |
| `ListRefreshed` | `{ count: usize }` | 343 | List updated |

### Events Used by SettingsPresenter

| Variant Name | Full Fields | Line | Purpose |
|--------------|-------------|------|---------|
| `Starting` | `{ id: Uuid, name: String }` | 226 | MCP starting |
| `Started` | `{ id: Uuid, name: String, tools: Vec<String>, tool_count: usize }` | 229 | MCP ready |
| `StartFailed` | `{ id: Uuid, name: String, error: String }` | 237 | MCP failed |
| `Stopped` | `{ id: Uuid, name: String }` | 244 | MCP stopped |
| `Created` (Profile) | `{ id: Uuid, name: String }` | 289 | Profile created |
| `Updated` (Profile) | `{ id: Uuid, name: String }` | 292 | Profile updated |
| `Deleted` (Profile) | `{ id: Uuid, name: String }` | 295 | Profile deleted |
| `DefaultChanged` | `{ profile_id: Option<Uuid> }` | 298 | Default changed |
| `Error` | `{ source: String, error: String, context: Option<String> }` | 399 | Global error |

### EventBus API (lines 25-66 in bus.rs)

```rust
impl EventBus {
    pub fn new(capacity: usize) -> Self;                              // line 31
    pub fn publish(&self, event: AppEvent) -> Result<usize, EventBusError>; // line 44
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent>;         // line 55
    pub fn subscriber_count(&self) -> usize;                          // line 63
}
```

**CRITICAL**: `subscribe()` returns ALL events. No filter parameter. Presenters must filter in their event loop.

## EventBus API (from src/events/bus.rs)

```rust
impl EventBus {
    pub fn new(capacity: usize) -> Self;
    pub fn publish(&self, event: AppEvent) -> Result<usize, EventBusError>;
    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent>;
    pub fn subscriber_count(&self) -> usize;
}
```

**Note**: There is NO filter-based subscription. Presenters must subscribe to ALL events and filter in their event loop.

## Scope

### In Scope
- Wiring `ChatPresenter` to chat/user/conversation events
- Wiring `HistoryPresenter` to conversation/user events
- Wiring `SettingsPresenter` to profile/MCP/system events
- E2E integration tests via `cargo test --test e2e_presenter_tests`
- Verification that events trigger correct ViewCommands

### Out of Scope
- New presenter implementations (assumed to exist in `src/presentation/`)
- UI view layer (assumes ViewCommand interface exists)
- Event bus implementation (assumed complete from prior work)

## Phases

| Phase | Name | Type | Prerequisites | Deliverable |
|-------|------|------|---------------|-------------|
| P00 | Preflight Verification | Preflight | None | Evidence of API compatibility |
| P01 | Wire EventBus to ChatPresenter | Implementation | P00 PASS | ChatPresenter event handlers |
| P01a | Verify ChatPresenter | Verification | P01 complete | Evidence of event → ViewCommand flow |
| P02 | Wire EventBus to HistoryPresenter | Implementation | P01a PASS | HistoryPresenter event handlers |
| P02a | Verify HistoryPresenter | Verification | P02 complete | Evidence of refresh events |
| P03 | Wire EventBus to SettingsPresenter | Implementation | P02a PASS | SettingsPresenter event handlers |
| P03a | Verify SettingsPresenter | Verification | P03 complete | Evidence of settings events |
| P04 | E2E Test: Chat Stream Events | Implementation | P03a PASS | Test file |
| P04a | Verify Chat Stream Test | Verification | P04 complete | Test output |
| P05 | E2E Test: MCP Lifecycle Events | Implementation | P04a PASS | Test file |
| P05a | Verify MCP Lifecycle Test | Verification | P05 complete | Test output |

**Phase Numbering Convention**: `##a` phases are verification-only and must follow their implementation phase. Per dev-docs/PLAN-TEMPLATE.md, verification phases are suffixed with 'a'.

## Success Criteria

### Functional
- [ ] All presenters receive events via `AppEvent` subscriptions
- [ ] Presenters emit correct `ViewCommand` for each event type
- [ ] E2E tests pass: `cargo test --test e2e_presenter_tests` (exit code 0)
- [ ] No `unimplemented!()`, `todo!()`, or placeholder strings in delivered code

### Evidence
- [ ] All verification phases have PASS verdict in evidence files
- [ ] `grep -rn "unimplemented!\|todo!" src/presentation/` returns no matches
- [ ] `grep -rn "placeholder\|not yet implemented" src/presentation/` returns no matches
- [ ] Test output captured to `evidence/PLAN-20250128-PRESENTERS/` directories

## Configuration

### LLM Profiles
- Synthetic profile: `~/.llxprt/profiles/synthetic.json`
- API key file: `~/.synthetic_key`

### MCP Servers
- Exa: No API key required (works out of box)

## Test Commands

```bash
# Run all presenter E2E tests
cargo test --test e2e_presenter_tests 2>&1 | tee evidence/PLAN-20250128-PRESENTERS/phase-04a/test-output.txt

# Run specific test
cargo test --test e2e_presenter_tests test_chat_stream_events

# Build check
cargo build --all-targets

# Placeholder detection
grep -rn "unimplemented!\|todo!" src/presentation/
grep -rn "placeholder\|not yet implemented" src/presentation/
```

## Inputs

- `dev-docs/COORDINATING.md` - Multi-phase coordination protocol
- `dev-docs/requirements/presentation.md` - Presenter requirements
- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` - Architecture context
- `src/events/types.rs` - Event enum definitions
- `src/presentation/*.rs` - Existing presenter implementations
- `src/events/bus.rs` - EventBus implementation

## Outputs

- Modified presenter files with event subscriptions
- E2E test files in `tests/e2e_presenter_tests.rs` or similar
- Evidence files in `project-plans/wire-presenters/plan/.completed/`
- Test output in `evidence/PLAN-20250128-PRESENTERS/`
