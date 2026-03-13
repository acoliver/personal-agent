# Phase 09: Regression Hardening for Preserved Behaviors

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P09`

## Prerequisites

- Required: Phase 08a completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P08a.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P08a.md`

## Requirements Implemented (Expanded)

### REQ-ARCH-006: Behavior Preservation

**Full Text**: Preserve bulk transcript replacement, no clear on ordinary activation, startup first-frame correctness, Kimi/provider quirks behavior, and existing transcript-loading/layout fixes.

**Behavior**:
- GIVEN: the recovery architecture is in place
- WHEN: regression hardening runs
- THEN: all preserved behaviors are explicitly covered by tests and verification evidence

**Why This Matters**: Recovery is only valid if the known good behavior set remains intact.

## Implementation Tasks

### Files to Modify

- existing regression/integration tests directly covering:
  - startup scrollback/layout
  - conversation switch behavior
  - presenter selection/settings
  - LLM client helper behaviors that remain on the selection/loading path or transcript mapping path
  - Kimi/provider quirks
  - any transcript-loading regressions directly affected by the store migration

### Implementation Requirements

- add missing coverage for preserved behaviors if current tests do not fully cover them
- document exact preserved invariants in evidence
- avoid unrelated test-scope expansion beyond this architecture recovery
- ensure preserved-behavior tests explicitly cover generation-aware success/failure freshness, no-clear activation semantics, and startup no-flash behavior where applicable
- explicitly re-verify active GPUI streaming/thinking behavior for `ShowThinking`, `HideThinking`, `AppendThinking`, `AppendStream`, `FinalizeStream`, `StreamCancelled`, and `StreamError` where the store migration touches the same chat-state surface
- require one named proof for `FinalizeStream` that states:
  1. what exact event or reducer transition makes assistant output durable,
  2. how `Uuid::nil()` resolves to the active conversation before a durable write,
  3. how stale/off-target finalization is rejected
- require one named proof for bounded `ConversationCleared` behavior that shows, in one deterministic harness/test, the mounted clear-handling path (currently `src/ui_gpui/views/chat_view.rs::ChatView::handle_command(ViewCommand::ConversationCleared)`, or an evidence-mapped repo-idiomatic equivalent) restores render state from authoritative `current_snapshot()` within the same synchronous mounted update transaction, before control returns to the event loop, without mutating authoritative transcript snapshot, selected id/title, generation, load state, or revision, and without deferring restoration onto a later tick/task/frame/subscriber callback/remount path

- if `ShowToolCall` / `UpdateToolCall` remain non-rendered GPUI transport variants, document that unchanged behavior explicitly rather than leaving their post-migration status implicit
- pull `llm_client_helpers_tests` into this phase only where helper behavior materially preserves transcript-loading, mapping, or failure-surface semantics touched by the migration

### Required Code Markers

Every created or materially updated test/helper in this phase must include markers matching project conventions:

```rust
/// @plan[: ]PLAN-20260304-GPUIREMEDIATE.P09
/// @requirement[: ]REQ-ARCH-006.1
/// @pseudocode[: ]analysis/pseudocode/02-selection-loading-protocol.md:022-060
```

Use exact requirement IDs per item, including `REQ-ARCH-006.1` through `REQ-ARCH-006.7` where applicable, and cite `analysis/pseudocode/03-main-panel-integration.md` for startup/remount assertions.

## Verification Commands

```bash
cargo test --test presenter_selection_and_settings_tests --test seven_bugs_regression_tests --test chat_startup_scrollback_layout_regression_tests --test chat_view_conversation_switch_regression_tests --test llm_client_helpers_tests --test kimi_provider_quirks_integration_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P09" tests src --include="*.rs"
grep -R -n "@requirement[: ]REQ-ARCH-006.1\|@requirement[: ]REQ-ARCH-006.2\|@requirement[: ]REQ-ARCH-006.3\|@requirement[: ]REQ-ARCH-006.4\|@requirement[: ]REQ-ARCH-006.5\|@requirement[: ]REQ-ARCH-006.6\|@requirement[: ]REQ-ARCH-006.7" tests src --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/02-selection-loading-protocol.md:\|@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" tests src --include="*.rs"
```

## Reachability / Integration Verification

- [ ] preserved behaviors are exercised through active runtime paths
- [ ] regressions would be caught if transcript ownership reverts to popup-local behavior

## Semantic Verification Checklist

- [ ] bulk replacement still works
- [ ] ordinary activation still does not clear transcript
- [ ] startup first frame still correct
- [ ] startup hydration still avoids empty/loading flash for known data
- [ ] stale generation success/failure payloads are ignored
- [ ] active GPUI streaming/thinking behavior still matches pre-migration semantics where applicable
- [ ] named `FinalizeStream` proof shows durable assistant-output materialization, `Uuid::nil()` target resolution, and stale finalization rejection
- [ ] named bounded-`ConversationCleared` proof shows local empty state is overwritten without store mutation
- [ ] any unchanged GPUI treatment of `ShowToolCall` / `UpdateToolCall` is explicit in evidence
- [ ] helper-level transcript mapping/failure semantics touched by the migration remain covered where applicable
- [ ] Kimi/provider quirks still correct
- [ ] transcript-loading/layout fixes still correct

## Success Criteria

- Preserved-behavior regressions are explicitly hardened, not assumed