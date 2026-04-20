# Phase 11: Sidebar Streaming Indicator Impl

Plan ID: `PLAN-20260416-ISSUE173.P11`

## Prerequisites

- P10 verified PASS.

## Requirements implemented

- REQ-173-004.3, REQ-173-005.1, REQ-173-005.2, REQ-173-005.3

## Tasks

### `src/ui_gpui/app_store_types.rs`

Add field:
```rust
#[derive(Clone, Debug, Default)]
pub struct HistoryStoreSnapshot {
    pub conversations: Vec<ConversationSummary>,
    pub selected_conversation_id: Option<Uuid>,
    /// Conversation ids currently streaming in the background.
    ///
    /// @plan PLAN-20260416-ISSUE173.P11
    /// @requirement REQ-173-004.3
    pub streaming_conversation_ids: HashSet<Uuid>,
}
```

Add `use std::collections::HashSet;`.

### `src/ui_gpui/app_store.rs`

Wherever `HistoryStoreSnapshot` is projected (look for `project_history_snapshot`
or the place `inner.snapshot.history` is rebuilt), populate
`streaming_conversation_ids: inner.active_streaming_targets.clone()`.

### `src/ui_gpui/views/chat_view/render_sidebar.rs`

Add the helper from P10 as real production code (no longer test-only):

```rust
/// @plan PLAN-20260416-ISSUE173.P11
/// @requirement REQ-173-005.1
#[must_use]
pub(crate) fn conversation_has_streaming_indicator(
    conversation_id: Uuid,
    streaming_ids: &HashSet<Uuid>,
) -> bool {
    streaming_ids.contains(&conversation_id)
}
```

In the conversation row render function, consume the sidebar render context's
history snapshot (or thread `streaming_conversation_ids` in explicitly) and,
when the helper returns true, render a subtle indicator. Concretely:

- Add a small circular dot element on the row, painted with
  `Theme::accent()` or the existing "streaming" colour used by the chat view's
  thinking bubble. The dot diameter ≈ 6 px, placed just before the
  conversation title. Fall back to a dim "●" text glyph if GPUI's div API for a
  sized filled circle is awkward — prefer whatever GPUI primitive is already
  used elsewhere in the sidebar for status glyphs.
- Avoid extra padding shifts when the dot is absent (allocate a fixed-width
  leading slot so layout is stable).

### Update the sidebar's render context

Find the struct/func that passes data to row rendering — the render function
for the sidebar currently takes the list of conversations. Thread the
streaming id set through, OR pass `HistoryStoreSnapshot` directly.

### Tests

The TDD tests from P10 should now pass. If the helper's visibility changed
between P10 and P11, update the tests accordingly (keep them behavioural; do
not weaken them).

## Verification

```bash
cargo build --all-targets 2>&1 | tail -10
cargo test --lib --tests 2>&1 | grep -E "test result|FAILED" | tail -10
grep -n "streaming_conversation_ids" src/ui_gpui/app_store.rs src/ui_gpui/app_store_types.rs src/ui_gpui/views/chat_view/render_sidebar.rs
grep -rn "unimplemented!\\|todo!\\|// TODO\\|placeholder" src/ui_gpui/
grep -c "@plan PLAN-20260416-ISSUE173.P11" src/ui_gpui/
```

### Manual smoke (record but do not block on)

- `cargo run` and verify that while one conversation is streaming you can
  switch to another and see the first's row indicator persist.
- If the manual run is not feasible in CI, document this in the evidence file
  and rely on the helper unit tests plus P12 integration test.

Deliverable: `project-plans/issue173/plan/.completed/P11.md`.
