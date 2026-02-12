# Plan: GPUI Migration

**Plan ID:** PLAN-20250128-GPUI
**Generated:** 2025-01-28 (Revised 2025-01-29)
**Total Phases:** 32 (16 implementation + 16 verification)

**Authoritative Contracts:** See `appendix-integration-contracts.md` for exhaustive mappings.

---

## Critical Architecture: Runtime Bridge

**This plan's success depends on correctly bridging GPUI (smol) and tokio runtimes.**

```
        (smol/GPUI thread)                         (tokio runtime)
┌────────────────────────────────────┐            ┌───────────────────────────┐
│ UI widgets / actions       │            │ EventBus (broadcast)      │
│                            │            │ Presenters                │
│  try_send(UserEvent) ──────┼──────────► │ recv_async(UserEvent)     │
│      via flume             │            │   -> process              │
│                            │            │   -> send(ViewCommand)    │
│  try_recv(ViewCommand)     │ ◄──────────┼───+ notifier.notify()     │
│   -> apply to UI state     │            └───────────────────────────┘
│   -> cx.notify()           │
└────────────────────────────┘
```

### Key Design Decisions

| Decision | Implementation |
|----------|----------------|
| Cross-runtime channels | `flume` (runtime-agnostic, works with both tokio and smol) |
| UserEvent channel | `flume::bounded(256)` - GPUI→tokio |
| ViewCommand channel | `flume::bounded(1024)` - tokio→GPUI |
| UserEvent emission | `flume::Sender::try_send()` (non-blocking) |
| ViewCommand reception | `flume::Receiver::try_recv()` drain loop |
| Backpressure (UserEvent) | Drop + warn (user can retry) |
| Backpressure (ViewCommand) | Drop + notify (GPUI must catch up) |
| Re-render trigger | AtomicBool notifier set by tokio, checked by GPUI |
| State ownership | 100% GPUI-side (no shared mutable state) |

---

## Integration Contracts

> **Complete mappings in `appendix-integration-contracts.md`**

### Summary: UserEvents Emitted by GPUI (→ EventBus)

**30 total variants** - GPUI emits ONLY UserEvent (not ChatEvent, McpEvent, etc.)

| UI Action | UserEvent |
|-----------|-----------|
| Send message | `UserEvent::SendMessage { text }` |
| Stop streaming | `UserEvent::StopStreaming` |
| New conversation | `UserEvent::NewConversation` |
| Toggle thinking | `UserEvent::ToggleThinking` |
| Select conversation | `UserEvent::SelectConversation { id }` |
| Select profile | `UserEvent::SelectProfile { id }` |
| Toggle MCP | `UserEvent::ToggleMcp { id, enabled }` |
| (+ 23 more) | See appendix Section A |

### Summary: ViewCommands Handled by GPUI (← Presenters)

**42 total variants** - All handled by GPUI state machine

| ViewCommand | UI Update |
|-------------|-----------|
| `AppendStream { chunk }` | Append to streaming message |
| `FinalizeStream` | Complete message, hide spinner |
| `ShowThinking` / `HideThinking` | Toggle spinner |
| `ConversationActivated { id }` | Load conversation |
| `HistoryUpdated` | Refresh conversation list |
| `ShowError { title, message }` | Display error banner |
| (+ 36 more) | See appendix Section B |

### Synchronization Test

The `integration_contract_sync_test` (see `plan/contract-sync-test.md`) uses exhaustive pattern matching to ensure the appendix stays synchronized with actual code. This test **fails to compile** if variants are added/removed.

---

## Phase Overview

| Phase | Title | Focus |
|-------|-------|-------|
| **01** | Preflight | Verify GPUI dependency, existing code |
| **02** | Analysis | Create pseudocode, component hierarchy |
| **03** | Bridge Stub | Create `flume` channels, GpuiBridge struct |
| **04** | Bridge TDD | Tests for UserEvent/ViewCommand flow |
| **05** | Bridge Implementation | Working cross-runtime bridge |
| **06** | Components Stub | Tab bar, buttons, message bubbles |
| **07** | Components TDD | Component rendering tests |
| **08** | Components Implementation | All components render |
| **09** | Chat View Stub | ChatView structure |
| **10** | Chat View TDD | Chat interaction tests |
| **11** | Chat View Implementation | Working chat with streaming |
| **12** | History & Settings Views | Remaining views |
| **13** | Popup Integration | NSStatusItem → GPUI popup |
| **14** | Visual Polish | Theme, transparency |
| **15** | Integration Testing | End-to-end flows |
| **16** | Documentation | Update docs |

---

## Dependencies

### New Dependencies (Cargo.toml)

```toml
[dependencies]
gpui = { git = "https://github.com/zed-industries/zed", branch = "main", package = "gpui" }
flume = "0.11"
```

### Existing Code Integration

| Existing File | Integration |
|---------------|-------------|
| `src/events/types.rs` | UserEvent variants (emit from GPUI) |
| `src/events/bus.rs` | EventBus (receive forwarded UserEvents) |
| `src/presentation/view_command.rs` | ViewCommand variants (handle in GPUI) |
| `src/presentation/*_presenter.rs` | Will use ViewCommandSink instead of mpsc::Sender |

---

## Verification Strategy

Each phase has:
1. **Structural verification:** Files exist, compile, have markers
2. **Behavioral verification:** Code actually works (not just stubs)
3. **Integration verification:** Connects to existing code correctly

### Implementation Phase Rules

In implementation phases (odd numbers 05+), these are **FAILURES**:
- `unimplemented!()` in delivered code
- `todo!()` in delivered code
- `// TODO` comments
- Placeholder values

---

## Success Criteria

- [ ] `flume` channels bridge GPUI ↔ tokio without blocking
- [ ] UserEvents reach EventBus and trigger presenters
- [ ] ViewCommands reach GPUI and update UI state
- [ ] GPUI popup opens from menu bar click
- [ ] All three tabs work (Chat, History, Settings)
- [ ] Streaming text appears in real-time
- [ ] Old AppKit UI code preserved (not deleted)
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes

---

## References

- `project-plans/gpui-migration/specification.md` - Full specification
- `project-plans/gpui-migration/appendix-integration-contracts.md` - **Authoritative mapping tables**
- `project-plans/gpui-migration/plan/contract-sync-test.md` - Synchronization test specification
- `research/exactobar/` - Reference GPUI menu bar app
- `src/events/types.rs` - UserEvent definitions (30 variants)
- `src/presentation/view_command.rs` - ViewCommand definitions (42 variants)
