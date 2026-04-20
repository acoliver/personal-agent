# Phase 03: Service Layer Implementation

Plan ID: `PLAN-20260416-ISSUE173.P03`

## Prerequisites

- P02 verified PASS.
- All P02 tests currently fail to compile or assert.

## Requirements implemented

- REQ-173-001.1, REQ-173-001.2, REQ-173-001.3
- Foundation for REQ-173-002.1, REQ-173-002.2 (signature change,
  scoped internal lookup — the full presenter/event wiring happens in P05).

## Implementation tasks

### Files to modify

#### `src/services/chat_impl.rs`

1. Remove imports `AtomicBool`, `Ordering` (no longer needed at top level; the
   streaming sub-module will stop using them too). Add:
   ```rust
   use std::collections::HashMap;
   use tokio_util::sync::CancellationToken;
   ```
2. Add an `ActiveStream` struct:
   ```rust
   /// @plan PLAN-20260416-ISSUE173.P03
   /// @requirement REQ-173-001.1
   pub(super) struct ActiveStream {
       pub(super) task: JoinHandle<()>,
       pub(super) cancel: CancellationToken,
   }
   ```
3. Replace the three fields on `ChatServiceImpl`:
   ```rust
   // OLD:
   // is_streaming: Arc<AtomicBool>,
   // current_conversation_id: Arc<StdMutex<Option<Uuid>>>,
   // stream_task: Arc<StdMutex<Option<JoinHandle<()>>>>,

   // NEW:
   active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
   ```
4. Update `ChatServiceImpl::new` and `new_for_tests` / `new_with_settings`
   initialization accordingly.
5. Rewrite `begin_stream`:
   ```rust
   async fn begin_stream(&self, conversation_id: Uuid) -> ServiceResult<()> {
       let map = self.active_streams.lock().expect("active_streams poisoned");
       if map.contains_key(&conversation_id) {
           return Err(ServiceError::Internal(
               "Stream already in progress for this conversation".to_string(),
           ));
       }
       Ok(())
   }
   ```
   Note: NO insertion here — insertion happens in `spawn_stream_task` where we
   have the `JoinHandle`.
6. Rewrite `spawn_stream_task`:
   - Create a `CancellationToken`.
   - Pass it to `run_stream_task` (extend its signature; see streaming.rs
     changes below).
   - Insert the new `ActiveStream { task, cancel }` into the map keyed by
     `conversation_id`.
   - Do NOT abort any prior task (there should not be one for the same id due
     to the `begin_stream` guard).
7. Rewrite `cancel_active_stream`:
   ```rust
   fn cancel_active_stream(&self, conversation_id: Uuid) {
       let removed = {
           let mut map = self.active_streams.lock().expect("active_streams poisoned");
           map.remove(&conversation_id)
       };
       if let Some(active) = removed {
           active.cancel.cancel();  // cooperative cancel first
           active.task.abort();     // hard abort as backstop
           // Approval scoping is added in P07; for P03 we can keep resolve_all here
           // with a TODO-marker — but we will REPLACE it in P07. (See below.)
           let resolved = self.approval_gate.resolve_all(false);
           for (rc, request_id) in resolved {
               let _ = self.view_tx.try_send(ViewCommand::ToolApprovalResolved {
                   conversation_id: rc,
                   request_id,
                   approved: false,
               });
           }
           let _ = emit(AppEvent::Chat(ChatEvent::StreamCancelled {
               conversation_id,
               message_id: Uuid::new_v4(),
               partial_content: String::new(),
           }));
       }
   }
   ```
   **Note**: P07 replaces `resolve_all` with `resolve_all_for_conversation`.
   For P03 we accept the known-overbroad scope (resolving all approvals on any
   cancel) because we have not yet implemented the scoped method. This is
   allowed because the impl-phase zero-tolerance rule applies to placeholders
   / unimplemented work, NOT to feature slicing across phases.
8. Update `ChatService::cancel(&self)` → `cancel(&self, conversation_id: Uuid)`:
   - Change trait in `src/services/chat.rs`.
   - Change impl in `chat_impl.rs` to call `cancel_active_stream(conversation_id)`.
9. Add `is_streaming_for` to trait and implementation:
   ```rust
   fn is_streaming_for(&self, conversation_id: Uuid) -> bool {
       self.active_streams.lock().expect("active_streams poisoned").contains_key(&conversation_id)
   }
   ```
10. Update `is_streaming` to `!map.is_empty()`.
11. Add `#[cfg(test)] pub(crate) async fn begin_stream_for_test(&self, id: Uuid)`
    if needed by P02 tests.

#### `src/services/chat_impl/streaming.rs`

1. Replace `is_streaming: Arc<AtomicBool>` and
   `current_conversation_id: Arc<StdMutex<Option<Uuid>>>` parameters in
   `run_stream_task`, `finalize_stream_task`, `create_stream_agent`, and
   `clear_streaming_state` with
   `active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>>` and a
   `cancel: CancellationToken`.
2. `clear_streaming_state` becomes:
   ```rust
   pub(super) fn clear_streaming_state(
       active_streams: &Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
       conversation_id: Uuid,
   ) {
       let mut map = active_streams.lock().expect("active_streams poisoned");
       map.remove(&conversation_id);
   }
   ```
3. Plumb `cancel: CancellationToken` through — it is used by the stream loop
   when it is ready (tool loops can check `cancel.is_cancelled()` in a future
   follow-up; for now the `task.abort()` path remains the primary cancel
   mechanism and the token is stored for future cooperative cancellation).

#### `src/services/chat_impl/tests.rs` and any dependent test files

Update any direct references to the removed fields (`is_streaming`,
`current_conversation_id`, `stream_task`). Touch them only where necessary to
keep the build green.

#### `src/services/chat_impl.rs` – `resolve_tool_approval`

The `ToolApprovalResponseAction::Denied` branch calls `self.cancel_active_stream()`
today with no argument. It must now pass a conversation_id. Use the
`conversation_id` returned by `approval_gate.resolve_and_take_identifiers` — it
already returns `(Uuid, Vec<String>)` where the `Uuid` is the conversation id.

#### `Cargo.toml`

If `tokio-util` is not already a dependency with the `sync` feature, add:
```toml
tokio-util = { version = "0.7", features = ["sync"] }
```

#### Presenter / event call-sites

`src/presentation/chat_presenter.rs`: `handle_stop_streaming` calls
`chat_service.cancel()`. For P03 the signature changed — update the call site to
pass a conversation_id. For this phase we need SOME value: use a placeholder
of the currently selected conversation id via
`conversation_service.get_active().await.ok().flatten()`. **This will be
rewritten cleanly in P05 when the event carries the id.** Log a warning if
`None` and early-return without calling cancel. Do NOT introduce a panic path.

All other callers of `ChatService::cancel` must be updated to pass
a `Uuid`. Use `grep -rn "\\.cancel(" src/` + read context to make sure no
call-site is left unchanged.

### Required markers

Every newly written function, struct, or field MUST carry:
```rust
/// @plan PLAN-20260416-ISSUE173.P03
/// @requirement REQ-173-001.x or REQ-173-002.x
```

## Verification commands (MUST run before reporting PASS)

```bash
cargo build --all-targets 2>&1 | tail -20
# Expected: 0 errors

cargo test --lib chat_impl 2>&1 | tail -40
# Expected: the five concurrent_streams tests from P02 all pass

cargo test --lib --tests 2>&1 | grep -E "test result|FAILED" | tail -20
# Expected: 0 failed

# Placeholder detection
grep -rn "unimplemented!\\|todo!" src/services/ src/llm/ src/presentation/ src/events/ src/ui_gpui/
# Expected: no output

grep -rn "// TODO\\|// FIXME\\|placeholder\\|not yet implemented" src/services/ src/llm/ src/presentation/ src/events/ src/ui_gpui/
# Expected: no output

grep -c "@plan PLAN-20260416-ISSUE173.P03" src/services/chat_impl.rs src/services/chat_impl/streaming.rs src/services/chat.rs
# Expected: ≥ 6 total
```

## Deliverable

Write `project-plans/issue173/plan/.completed/P03.md` with:

- File diff summary.
- Full output of each verification command above.
- Confirmation that the five P02 tests pass.
- Verdict: PASS | FAIL (no conditional).
