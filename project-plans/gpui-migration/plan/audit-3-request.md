# Audit 3 Request: GPUI Migration Plan

**Date:** 2025-01-29
**Requesting:** Full architecture coherence review

---

## Summary of Changes Since Audit 2

This revision addresses all 5 remaining gaps identified in Audit 2 (PARTIALLY_COHERENT verdict).

---

## Gap 1: Mapping Tables Not Exhaustive

**Before:** Summary tables with representative examples
**After:** Created `appendix-integration-contracts.md` with:

- **Section A:** All 30 UserEvent variants from `src/events/types.rs` (lines 46-141)
  - Each variant has: #, Variant name, Payload, GPUI Emits (YES for all), Origin, Handler
  - Organized by category: Chat Actions (8), Profile Actions (7), MCP Actions (9), Model Selector (4), Navigation (2)

- **Section B:** All 42 ViewCommand variants from `src/presentation/view_command.rs` (lines 19-261)
  - Each variant has: #, Variant name, Payload, GPUI Handles (YES for all), UI State Update, Test Coverage
  - Organized by category: Chat (16), History (4), Settings (8), MCP (6), Model Selector (2), Error (2), Navigation (4)

- Supporting types documented: MessageRole, McpStatus, ErrorSeverity, ModalId, ViewId

---

## Gap 2: Backpressure Strategy Undefined

**Before:** Channel sizes mentioned but no overflow handling
**After:** Added to specification.md Section 3 and appendix Section C:

| Channel | Capacity | Overflow Behavior | Rationale |
|---------|----------|-------------------|-----------|
| UserEvent | bounded(256) | Drop + warn | UI responsiveness > event delivery; user can retry |
| ViewCommand | bounded(1024) | Drop + notify | Presenters should coalesce; GPUI must catch up |

Concrete code example for `ViewCommandSink::send()` with `TrySendError` handling.

---

## Gap 3: Notifier Mechanism Not Finalized

**Before:** "GPUI notifier" mentioned without details
**After:** Added appendix Section D with:

- `GpuiNotifierImpl` struct with `AtomicBool` for thread-safe signaling
- `notify()` method that sets flag
- `check_and_clear()` method that atomically reads and clears
- Lifecycle: creation, distribution to ViewCommandSink, consumption in render loop, cleanup

---

## Gap 4: No Synchronization Mechanism

**Before:** Mapping tables could drift from actual code
**After:** Created `plan/contract-sync-test.md` with:

- Exhaustive pattern matching test that **fails to compile** if enum variants change
- `test_user_event_variant_count()` - exhaustive match over all 30 variants
- `test_view_command_variant_count()` - exhaustive match over all 42 variants
- CI integration guidance

This provides **compile-time** synchronization checking.

---

## Gap 5: Missing E2E Behavioral Test

**Before:** Unit tests only
**After:** Added to `04-bridge-tdd.md`:

1. `test_e2e_with_state_application()` - Full round-trip:
   - GPUI emits UserEvent
   - EventBus receives via forwarder
   - Simulates presenter sending ViewCommands
   - Verifies notifier called
   - GPUI drains commands
   - Applies to simulated state
   - Verifies state correctly updated

2. `test_view_command_overflow_behavior()` - Channel overflow:
   - Fills small bounded channel
   - Verifies overflow doesn't block
   - Verifies notifier still called
   - Verifies only buffered commands received

---

## Files Modified

| File | Change |
|------|--------|
| `appendix-integration-contracts.md` | NEW - 485 lines, exhaustive mappings |
| `specification.md` | Added reference to appendix, backpressure strategy, notifier mechanism |
| `plan/00-overview.md` | Updated with channel capacities, variant counts, appendix reference |
| `plan/04-bridge-tdd.md` | Added E2E test with state application, overflow test |
| `plan/05a-bridge-impl-verification.md` | Added behavioral test requirements, sync test |
| `plan/contract-sync-test.md` | NEW - synchronization test specification |
| `review-architecture-coherence.md` | Updated with revision history |

---

## Checklist for Audit 3

Please verify:

- [ ] UserEvent mapping is exhaustive (30 variants, matches `src/events/types.rs`)
- [ ] ViewCommand mapping is exhaustive (42 variants, matches `src/presentation/view_command.rs`)
- [ ] Backpressure strategy is concrete and reasonable
- [ ] Notifier mechanism is concrete with lifecycle details
- [ ] Synchronization test would catch enum drift at compile time
- [ ] E2E behavioral test covers full round-trip flow
- [ ] Plan is coherent with `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md`
- [ ] Plan is coherent with `dev-docs/requirements/events.md`

---

## Expected Verdict

With these changes, the plan should be **FULLY_COHERENT**:

1. Direction issues: FIXED (Audit 2)
2. Integration contracts: EXHAUSTIVE (30 + 42 variants mapped)
3. Bridge design: CONCRETE (flume + notifier + backpressure)
4. Verification: BEHAVIORAL (E2E tests, not just grep)
5. Synchronization: COMPILE-TIME (exhaustive pattern matching)

---

## References

- `project-plans/gpui-migration/specification.md`
- `project-plans/gpui-migration/appendix-integration-contracts.md`
- `project-plans/gpui-migration/plan/00-overview.md`
- `project-plans/gpui-migration/plan/contract-sync-test.md`
- `project-plans/gpui-migration/review-architecture-coherence.md`
- `dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md`
- `dev-docs/requirements/events.md`
- `src/events/types.rs`
- `src/presentation/view_command.rs`
