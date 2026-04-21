# Phase 09: Store active_streaming_targets Set Impl

Plan ID: `PLAN-20260416-ISSUE173.P09`

## Prerequisites

- P08 verified PASS.

## Requirements implemented

- REQ-173-004.1, REQ-173-004.2, REQ-173-004.3

## Tasks

### `src/ui_gpui/app_store.rs`

Replace field in `AppStoreInner`:
```rust
// OLD:
// pub(super) active_streaming_target: Option<Uuid>,
// NEW:
pub(super) active_streaming_targets: HashSet<Uuid>,
```
Add `use std::collections::HashSet;` if not already present.

Update every site that reads or writes `active_streaming_target`:
- Line ~434 (call to `project_streaming_snapshot`): pass
  `&inner.active_streaming_targets`.
- Line ~764 clearing: `inner.active_streaming_targets.remove(&id);`
  (no `== Some(id)` check needed — `HashSet::remove` is a no-op if absent).

### `src/ui_gpui/app_store_streaming.rs`

Every place that currently does:
- `inner.active_streaming_target == Some(target)` → `inner.active_streaming_targets.contains(&target)`
- `inner.active_streaming_target = Some(target)` → `inner.active_streaming_targets.insert(target);`
- `inner.active_streaming_target = None` → `inner.active_streaming_targets.remove(&target);`

Review every match in the earlier grep output (lines 49, 71, 111, 130, 165-166,
183, 197-198) and update accordingly.

In `resolve_nil_or_explicit_target` the current fallback
```rust
inner.active_streaming_target.or(inner.snapshot.chat.selected_conversation_id)
```
becomes:
```rust
// Prefer the selected conversation if it is currently active; otherwise any
// one from the set (arbitrary but stable is unimportant — this is a legacy
// fallback for nil ids).
let selected = inner.snapshot.chat.selected_conversation_id;
selected
    .filter(|id| inner.active_streaming_targets.contains(id))
    .or_else(|| inner.active_streaming_targets.iter().copied().next())
    .or(selected)
```

### `src/ui_gpui/app_store_types.rs`

Change `project_streaming_snapshot` signature:

```rust
#[must_use]
pub fn project_streaming_snapshot<S: BuildHasher, H: BuildHasher>(
    streaming_states: &HashMap<Uuid, ConversationStreamingState, S>,
    selected_conversation_id: Option<Uuid>,
    active_streaming_targets: &HashSet<Uuid, H>,
) -> StreamingStoreSnapshot {
    let Some(conversation_id) = selected_conversation_id else {
        return StreamingStoreSnapshot::default();
    };
    let active = active_streaming_targets.contains(&conversation_id);
    streaming_states
        .get(&conversation_id)
        .cloned()
        .unwrap_or_default()
        .project_for_snapshot(active, conversation_id)
}
```

Add `use std::collections::HashSet;` if not already present.

The `StreamingStoreSnapshot::active_target: Option<Uuid>` field **stays** — it's
a single-selection projection, so no downstream view code needs to change for
this phase. Only the input type changes.

### `src/ui_gpui/views/chat_view/mod.rs` and any other caller of `project_streaming_snapshot`

Update the one call site (app_store.rs line ~432) per the new signature. No
other callers exist in production code; mock call sites in tests must be
updated similarly.

### `src/ui_gpui/app_store.rs` initialization

Where `active_streaming_target` was defaulted to `None`, now default to
`HashSet::new()`. Check the `Default` impl / constructor.

## Verification

```bash
cargo build --all-targets 2>&1 | tail -10
cargo test --lib app_store 2>&1 | tail -40
cargo test --lib --tests 2>&1 | grep -E "test result|FAILED" | tail -10
grep -n "active_streaming_target\b" src/ | grep -v "active_streaming_targets"    # MUST be empty (no singular usages left)
grep -rn "unimplemented!\\|todo!\\|// TODO\\|placeholder" src/ui_gpui/
grep -c "@plan PLAN-20260416-ISSUE173.P09" src/ui_gpui/
```

Deliverable: `project-plans/issue173/plan/.completed/P09.md`.
