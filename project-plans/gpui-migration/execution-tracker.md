# Execution Tracker: PLAN-20250128-GPUI

## Status Summary

- **Total Phases:** 32 (16 implementation + 16 verification)
- **Completed:** 0
- **In Progress:** 0
- **Remaining:** 32
- **Current Phase:** P01 (not started)

---

## Critical Architecture Note

**This plan's success depends on the runtime bridge (Phases 03-05).**

The bridge uses `flume` channels for cross-runtime communication:
- GPUI (smol) ↔ tokio presenters
- `try_send()` / `try_recv()` - never block
- GPUI notifier wakes UI when ViewCommands arrive

---

## Phase Status

| Phase | ID | Title | Status | Attempts | Notes |
|-------|-----|-------|--------|----------|-------|
| 01 | P01 | Preflight | PENDING | 0 | Verify deps, existing code |
| 01a | P01a | Preflight Verification | PENDING | 0 | |
| 02 | P02 | Analysis | PENDING | 0 | Pseudocode, component hierarchy |
| 02a | P02a | Analysis Verification | PENDING | 0 | |
| 03 | P03 | **Bridge Stub** | PENDING | 0 | flume channels, GpuiBridge |
| 03a | P03a | Bridge Stub Verification | PENDING | 0 | |
| 04 | P04 | **Bridge TDD** | PENDING | 0 | 12+ tests for bridge |
| 04a | P04a | Bridge TDD Verification | PENDING | 0 | |
| 05 | P05 | **Bridge Implementation** | PENDING | 0 | CRITICAL: All tests pass |
| 05a | P05a | Bridge Impl Verification | PENDING | 0 | |
| 06 | P06 | Components Stub | PENDING | 0 | Tab bar, bubbles, buttons |
| 06a | P06a | Components Stub Verification | PENDING | 0 | |
| 07 | P07 | Components TDD | PENDING | 0 | |
| 07a | P07a | Components TDD Verification | PENDING | 0 | |
| 08 | P08 | Components Implementation | PENDING | 0 | |
| 08a | P08a | Components Impl Verification | PENDING | 0 | |
| 09 | P09 | Chat View Stub | PENDING | 0 | |
| 09a | P09a | Chat View Stub Verification | PENDING | 0 | |
| 10 | P10 | Chat View TDD | PENDING | 0 | |
| 10a | P10a | Chat View TDD Verification | PENDING | 0 | |
| 11 | P11 | Chat View Implementation | PENDING | 0 | |
| 11a | P11a | Chat View Impl Verification | PENDING | 0 | |
| 12 | P12 | History & Settings Views | PENDING | 0 | |
| 12a | P12a | History & Settings Verification | PENDING | 0 | |
| 13 | P13 | Popup Integration | PENDING | 0 | NSStatusItem → GPUI |
| 13a | P13a | Popup Integration Verification | PENDING | 0 | |
| 14 | P14 | Visual Polish | PENDING | 0 | Theme, transparency |
| 14a | P14a | Visual Polish Verification | PENDING | 0 | |
| 15 | P15 | Integration Testing | PENDING | 0 | End-to-end |
| 15a | P15a | Integration Verification | PENDING | 0 | |
| 16 | P16 | Documentation | PENDING | 0 | |
| 16a | P16a | Documentation Verification | PENDING | 0 | |

---

## Integration Contract Checklist

### UserEvents GPUI Must Emit

- [ ] `UserEvent::SendMessage { text }` - Chat send
- [ ] `UserEvent::StopStreaming` - Chat stop
- [ ] `UserEvent::NewConversation` - New conversation
- [ ] `UserEvent::ToggleThinking` - Toggle thinking
- [ ] `UserEvent::SelectConversation { id }` - History select
- [ ] `UserEvent::SelectProfile { id }` - Settings profile
- [ ] `UserEvent::ToggleMcp { id, enabled }` - Settings MCP

### ViewCommands GPUI Must Handle

- [ ] `AppendStream { chunk }` - Streaming text
- [ ] `FinalizeStream` - Complete message
- [ ] `ShowThinking` / `HideThinking` - Spinner
- [ ] `ConversationActivated { id }` - Load conversation
- [ ] `HistoryUpdated` - Refresh list
- [ ] `ShowError` / `ClearError` - Error display
- [ ] `ProfileCreated/Updated/Deleted` - Profile changes
- [ ] `McpStatusChanged` - MCP status

---

## Completion Markers

Evidence files:
- [ ] `.completed/P01.md`
- [ ] `.completed/P01A.md`
- [ ] `.completed/P02.md`
- [ ] `.completed/P02A.md`
- [ ] `.completed/P03.md` (Bridge Stub)
- [ ] `.completed/P03A.md`
- [ ] `.completed/P04.md` (Bridge TDD)
- [ ] `.completed/P04A.md`
- [ ] `.completed/P05.md` (Bridge Implementation)
- [ ] `.completed/P05A.md`
- [ ] `.completed/P06.md` through `.completed/P16A.md`

---

## Final Checklist

- [ ] All phases completed with PASS
- [ ] `flume` channels work (no blocking)
- [ ] UserEvents reach EventBus
- [ ] ViewCommands update GPUI state
- [ ] GPUI popup opens from menu bar
- [ ] All three tabs functional
- [ ] Streaming works end-to-end
- [ ] Old AppKit code preserved
- [ ] `cargo build` succeeds
- [ ] `cargo test` passes

---

## Notes

Plan revised 2025-01-29 with:
- Correct runtime bridge architecture using `flume`
- Complete UserEvent/ViewCommand integration contracts
- Bridge phases (03-05) as critical path
