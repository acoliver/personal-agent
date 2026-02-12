# Architecture Coherence Review: GPUI Migration Plan

**Date:** 2025-01-29 (Updated)
**Status:** REVISED - Addressing remaining gaps
**Audit History:** 
- Audit 1: INCOHERENT (direction errors, missing contracts, hand-waved bridge)
- Audit 2: PARTIALLY_COHERENT (fixed direction, added contracts, specified bridge)
- Audit 3: Pending (added exhaustive mappings, sync test, backpressure)

---

## Summary

The GPUI Migration Plan (`PLAN-20250128-GPUI`) has been **revised multiple times** to be coherent with:
- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md`
- `dev-docs/requirements/events.md`
- `project-plans/refactor/` (existing presenter/service implementation)

---

## Key Architectural Alignment

### 1. Event-Driven Architecture [OK]

**Architecture Doc Says:**
> Views emit `UserEvent` on user actions → EventBus → Presenters handle → Services process → Presenters emit ViewCommands → UI updates

**GPUI Plan Implements:**
- GPUI views emit `UserEvent` via `GpuiBridge.emit()` (non-blocking)
- `spawn_user_event_forwarder()` bridges flume→EventBus
- Presenters use `ViewCommandSink` to send ViewCommands
- GPUI drains ViewCommands via `GpuiBridge.drain_commands()`
- `cx.notify()` triggers re-render

### 2. Runtime Bridge (New Addition) [OK]

**Problem Identified:**
- GPUI uses smol runtime
- Presenters/Services use tokio runtime
- Need non-blocking bidirectional communication

**Solution Implemented:**
- `flume` channels (runtime-agnostic)
- `GpuiBridge` for GPUI side (try_send/try_recv)
- `ViewCommandSink` for presenter side (send + notify)
- GPUI notifier wakes UI when ViewCommands arrive

### 3. Integration Contracts [OK - NOW EXHAUSTIVE]

**Documented in `appendix-integration-contracts.md`:**

| Category | Count | Status |
|----------|-------|--------|
| UserEvent variants | 30 | All mapped with GPUI emits: YES |
| ViewCommand variants | 42 | All mapped with GPUI handles: YES |
| Supporting types | 5 | MessageRole, McpStatus, ErrorSeverity, ModalId, ViewId |

**Synchronization mechanism:**
- `tests/integration_contract_sync_test.rs` uses exhaustive pattern matching
- Fails to **compile** if variants added/removed
- Ensures appendix stays synchronized with code

### 4. State Ownership [OK]

**Architecture Doc Says:**
> Presenters manage UI state, transform domain models to view state

**GPUI Plan Implements:**
- `UiState` struct lives 100% in GPUI (no shared mutation)
- Presenters send pure data commands via ViewCommand
- GPUI applies commands to local state
- Single-threaded state updates in GPUI

### 5. Presenter Layer Reuse [OK]

**Existing Code:**
- `src/presentation/chat_presenter.rs` - COMPLETE
- `src/presentation/history_presenter.rs` - COMPLETE
- `src/presentation/settings_presenter.rs` - COMPLETE
- `src/presentation/view_command.rs` - 44 ViewCommand variants

**GPUI Plan:**
- Uses existing ViewCommand enum unchanged
- Uses existing UserEvent enum unchanged
- Only changes: presenters use `ViewCommandSink` instead of `mpsc::Sender`

---

## Issues Fixed in Revision

### 1. ~~Incorrect Responsibility Claim~~

**Before:** "presenters receive ViewCommand"
**After:** "presenters SEND ViewCommands, UI receives"

### 2. ~~Missing Integration Contracts~~

**Before:** No mapping tables
**After (Audit 2):** Section 5 in specification.md has summary tables
**After (Audit 3):** `appendix-integration-contracts.md` has exhaustive tables (30 UserEvent, 42 ViewCommand)

### 3. ~~Undefined Runtime Bridge~~

**Before:** "use smol::block_on" hand-waving
**After (Audit 2):** Concrete `flume` + GPUI notifier design with code sketches
**After (Audit 3):** Added backpressure strategy (bounded channels, drop+warn on overflow)

### 4. ~~Grep-Only Verification~~

**Before:** Just check files exist
**After (Audit 2):** Behavioral verification tests in Phase 05a
**After (Audit 3):** Added E2E test with state application, channel overflow test

### 5. ~~No Synchronization Mechanism~~

**Before (Audit 2):** Mapping tables could drift from code
**After (Audit 3):** `integration_contract_sync_test.rs` with exhaustive pattern matching ensures compile-time synchronization

---

## Phase Structure Coherence

| Phase | Aligns With |
|-------|-------------|
| P01-02 | Prerequisites verification |
| **P03-05** | **Bridge implementation (NEW)** |
| P06-08 | Component layer (`dev-docs/requirements/ui/`) |
| P09-12 | View layer (`ARCHITECTURE_IMPROVEMENTS.md` UI Layer) |
| P13 | Popup integration (ExactoBar patterns) |
| P14-16 | Polish, testing, documentation |

---

## Verdict: PENDING AUDIT 3

The plan now has:
1. [OK] Correctly describes data flow (UI → UserEvent → EventBus → Presenter → Service → ViewCommand → UI)
2. [OK] Concrete bridge implementation (flume + notifier)
3. [OK] **Exhaustive** integration contracts (30 UserEvent, 42 ViewCommand mapped)
4. [OK] Reuses existing presenter/service code unchanged
5. [OK] Behavioral verification (E2E tests, overflow tests)
6. [OK] Synchronization mechanism (compile-time check via exhaustive match)
7. [OK] Backpressure strategy (bounded channels, drop+warn)
8. [OK] Notifier specification (AtomicBool with check_and_clear)

**Awaiting audit to confirm FULLY_COHERENT status.**

---

## Changes Made in This Revision (2025-01-29)

1. Created `appendix-integration-contracts.md` with:
   - All 30 UserEvent variants from `src/events/types.rs` (lines 46-141)
   - All 42 ViewCommand variants from `src/presentation/view_command.rs` (lines 19-261)
   - GPUI emits/handles column for each variant
   - Supporting type definitions

2. Added backpressure strategy:
   - UserEvent channel: bounded(256), drop+warn on overflow
   - ViewCommand channel: bounded(1024), drop+notify on overflow
   - Rationale and code examples

3. Specified notifier mechanism:
   - `GpuiNotifierImpl` with AtomicBool
   - `check_and_clear()` for GPUI render loop
   - Lifecycle ownership details

4. Added synchronization test specification:
   - `plan/contract-sync-test.md` with exhaustive pattern matching
   - Compile-time failure on enum changes
   - CI integration guidance

5. Added behavioral E2E test:
   - `test_e2e_with_state_application` - full round-trip with state mutation
   - `test_view_command_overflow_behavior` - overflow handling

6. Updated overview and specification to reference appendix as authoritative source

---

## References

- `project-plans/gpui-migration/specification.md` - Full revised specification
- **`project-plans/gpui-migration/appendix-integration-contracts.md`** - Authoritative mapping tables
- `project-plans/gpui-migration/plan/contract-sync-test.md` - Synchronization test spec
- `project-plans/gpui-migration/plan/00-overview.md` - Phase overview
- `project-plans/gpui-migration/plan/03-bridge-stub.md` through `05a-bridge-impl-verification.md` - Bridge phases
- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md` - Target architecture
- `dev-docs/requirements/events.md` - Event system requirements
- `src/events/types.rs` - UserEvent source (30 variants)
- `src/presentation/view_command.rs` - ViewCommand source (42 variants)
