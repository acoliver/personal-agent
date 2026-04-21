# Phase 05: Cancel Pipeline Implementation

Plan ID: `PLAN-20260416-ISSUE173.P05`

## Prerequisites

- P04 verified PASS.

## Requirements implemented

- REQ-173-002.3 end-to-end event flow.

## Tasks

### `src/events/types.rs`

Change:
```rust
StopStreaming,
```
to
```rust
/// User requested to stop a conversation's active stream.
///
/// @plan PLAN-20260416-ISSUE173.P05
/// @requirement REQ-173-002.3
StopStreaming { conversation_id: Uuid },
```

Add any needed `use uuid::Uuid;` import (likely already present).

### `src/ui_gpui/views/chat_view/render.rs`

Two emission sites:
- Around line 177 (Esc key handler):
  - Read the currently selected conversation id from view state. If none, do
    NOT emit `StopStreaming`. Otherwise emit
    `UserEvent::StopStreaming { conversation_id: id }`.
- Around line 766 (Stop button `on_mouse_down`):
  - Same change.

Both sites should use the existing way the view accesses its snapshot's
`selected_conversation_id` (check `self.state` or snapshot struct; see how the
view reads other per-conversation info nearby).

### `src/presentation/chat_presenter.rs`

Match arm (~line 157):
```rust
UserEvent::StopStreaming { conversation_id } => {
    Self::handle_stop_streaming(chat_service, view_tx, conversation_id).await;
}
```

`handle_stop_streaming` signature:
```rust
/// @plan PLAN-20260416-ISSUE173.P05
/// @requirement REQ-173-002.3
async fn handle_stop_streaming(
    chat_service: &Arc<dyn ChatService>,
    _view_tx: &mut mpsc::Sender<ViewCommand>,
    conversation_id: Uuid,
) {
    chat_service.cancel(conversation_id);
}
```

Remove the P03-era temporary `conversation_service.get_active()` fallback — it's
no longer needed. If you left any, delete them now.

### Serialization

`UserEvent` derives `Serialize`. Confirm the struct-variant serialization still
compiles. Update any snapshot tests that embed `UserEvent::StopStreaming` if
they exist — check: `grep -rn "StopStreaming" src/`.

## Verification

```bash
cargo build --all-targets 2>&1 | tail -10
cargo test --lib --tests 2>&1 | grep -E "test result|FAILED" | tail -10
grep -rn "UserEvent::StopStreaming[^ ]" src/     # should show struct-style patterns everywhere
grep -rn "unimplemented!\\|todo!\\|// TODO\\|placeholder" src/events/ src/presentation/ src/ui_gpui/views/chat_view/ src/services/
grep -c "@plan PLAN-20260416-ISSUE173.P05" src/
```

## Deliverable

`project-plans/issue173/plan/.completed/P05.md` with:
- Diff summary.
- Full output of each verification command.
- Proof the four P02 tests and both P04 tests pass.
- Verdict PASS | FAIL.
