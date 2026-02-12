# Remaining Phases Outline

This document outlines phases 06-16 that will be detailed after the bridge phases complete.

**NOTE:** Phases 03-05 are now the Bridge phases (flume channels, GpuiBridge, ViewCommandSink).

---

## Phase 06: Components Stub

**Focus:** Create GPUI component structure with stub implementations.

### Components to Create
- TabBar (Chat/History/Settings tabs)
- MessageBubble (user/assistant message display)
- InputBar (text input + Send/Stop buttons)
- ConversationList (dropdown)
- Toggle (on/off switch)
- Button (standard clickable)

---

## Phase 06a: Components Stub Verification

**Focus:** Verify all component files exist and compile.

---

## Phase 07: Components TDD

**Focus:** Write tests for GPUI components before implementation.

### Tests to Write
- TabBar renders three tabs
- TabBar highlights active tab
- TabBar emits events on click
- MessageBubble renders user style
- MessageBubble renders assistant style
- MessageBubble handles streaming content
- InputBar captures text
- InputBar enables/disables send
- InputBar shows stop when streaming

---

## Phase 07a: Components TDD Verification

**Focus:** Verify tests compile and fail (TDD pattern).

---

## Phase 08: Components Implementation

**Focus:** Implement all GPUI components to pass tests.

### Components
- TabBar (active state, click handlers)
- MessageBubble (user/assistant styling, streaming cursor)
- InputBar (text input, send/stop buttons)
- ConversationList (dropdown)
- Toggle (on/off switch)
- Button (standard clickable)

---

## Phase 08a: Components Implementation Verification

**Focus:** Verify all component tests pass.

---

## Phase 09: Chat View Stub
- **ID:** `PLAN-20250128-GPUI.P09`
- **Goal:** Create ChatView structure with Render trait
- **Files:** `src/ui_gpui/views/chat_view.rs`
- **Includes:** ChatState struct, message list, input bar integration

## Phase 09a: Chat View Stub Verification
- File exists, compiles, has markers

---

## Phase 10: Chat View TDD
- **ID:** `PLAN-20250128-GPUI.P10`
- **Goal:** Write tests for chat interactions
- **Tests:** Message display, streaming updates, input handling

## Phase 10a: Chat View TDD Verification
- Tests compile and fail correctly

---

## Phase 11: Chat View Implementation
- **ID:** `PLAN-20250128-GPUI.P11`
- **Goal:** Implement ChatView to pass all tests
- **Critical:** Full streaming support, thinking toggle, toolbar

## Phase 11a: Chat View Implementation Verification
- NO placeholders, all tests pass
- Chat renders messages correctly

---

## Phase 12: History & Settings Views
- **ID:** `PLAN-20250128-GPUI.P12`
- **Goal:** Implement remaining views
- **Files:** history_view.rs, settings_view.rs
- **Includes:** Conversation list, transparency slider

## Phase 12a: History & Settings Verification
- Views render, interact correctly

---

## Phase 13: Popup Integration
- **ID:** `PLAN-20250128-GPUI.P13`
- **Goal:** Connect NSStatusItem click to GPUI popup window
- **Includes:** Popup positioning, focus handling, click-outside-to-dismiss
- **Uses:** GpuiBridge from Phase 05 for ViewCommand handling

## Phase 13a: Popup Integration Verification
- Popup opens from menu bar click
- Popup closes on click outside
- ViewCommands flow correctly

---

## Phase 14: Visual Polish
- **ID:** `PLAN-20250128-GPUI.P14`
- **Goal:** Theme parity, transparency settings
- **Includes:** Color matching, spacing, font sizes

## Phase 14a: Visual Polish Verification
- Visual comparison with existing UI

---

## Phase 15: Integration Testing
- **ID:** `PLAN-20250128-GPUI.P15`
- **Goal:** End-to-end tests
- **Tests:** Full chat flow, history loading, settings persistence

## Phase 15a: Integration Verification
- Complete user flow works

---

## Phase 16: Documentation
- **ID:** `PLAN-20250128-GPUI.P16`
- **Goal:** Update docs, usage guide
- **Includes:** README updates, architecture docs, feature flag docs

## Phase 16a: Documentation Verification
- Docs complete and accurate

---

## Full Phase Documents

Full phase documents (following PLAN-TEMPLATE.md) will be created as execution progresses. This outline ensures the coordinator understands the full scope.
