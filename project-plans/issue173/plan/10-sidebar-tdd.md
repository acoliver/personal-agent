# Phase 10: Sidebar Streaming Indicator TDD

Plan ID: `PLAN-20260416-ISSUE173.P10`

## Prerequisites

- P09 verified PASS.

## Requirements implemented (tests only)

- REQ-173-004.3, REQ-173-005.1, REQ-173-005.2

## Tasks

### Test: history snapshot publishes streaming ids

Add to `src/ui_gpui/app_store/` tests (or equivalent):

```rust
/// @plan PLAN-20260416-ISSUE173.P10
/// @requirement REQ-173-004.3
#[test]
fn history_snapshot_exposes_streaming_conversation_ids() {
    // Build an AppStoreInner with active_streaming_targets = {A, B}.
    // Project history_snapshot (however the current code does it — see
    // `project_history_snapshot` or equivalent in app_store.rs).
    // Assert snapshot.streaming_conversation_ids == {A, B}.
}
```

### Test: sidebar renders indicator when id in set

Find how existing sidebar row tests work. If there is a pure-logic helper
(e.g. `row_has_indicator(conversation: &ConversationSummary, streaming_ids: &HashSet<Uuid>) -> bool`), test it.
If not, add one to `src/ui_gpui/views/chat_view/render_sidebar.rs` with a
doc-tested signature so it can be unit-tested without GPUI:

```rust
/// Returns true if this conversation should display a streaming indicator.
///
/// @plan PLAN-20260416-ISSUE173.P10
/// @requirement REQ-173-005.1
#[must_use]
pub(crate) fn conversation_has_streaming_indicator(
    conversation_id: Uuid,
    streaming_ids: &HashSet<Uuid>,
) -> bool {
    streaming_ids.contains(&conversation_id)
}
```

For P10 the TDD phase this helper does **not yet exist in production**. Write
unit tests in a new `#[cfg(test)] mod` that reference the helper, expecting
compile failure. Tests to include:

1. `conversation_has_streaming_indicator_true_when_in_set`.
2. `conversation_has_streaming_indicator_false_when_not_in_set`.
3. `conversation_has_streaming_indicator_false_for_empty_set`.

Mark each with P10 and REQ-173-005.1.

## Verification

- `cargo build --all-targets 2>&1 | tail -10` — expected failure: "no field
  `streaming_conversation_ids`" and/or "no function named
  `conversation_has_streaming_indicator`".
- `grep -c "@plan PLAN-20260416-ISSUE173.P10" src/ui_gpui/`

Deliverable: `project-plans/issue173/plan/.completed/P10.md`.
