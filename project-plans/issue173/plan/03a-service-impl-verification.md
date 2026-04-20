# Phase 03a: Service Layer Impl Verification

Plan ID: `PLAN-20260416-ISSUE173.P03a`

## Role

Skeptical auditor. Assume nothing works until proven.

## Mandatory checks (paste EXACT output for each)

1. `cargo build --all-targets 2>&1 | tail -5`
2. `cargo test --lib chat_impl 2>&1 | grep -E "^test |test result"` — all five
   `concurrent_streams::*` tests pass.
3. `cargo test --lib --tests 2>&1 | tail -20` — zero failures.
4. Placeholder detection (each MUST return no matches):
   - `grep -rn "unimplemented!" src/services/ src/llm/ src/events/ src/presentation/ src/ui_gpui/`
   - `grep -rn "todo!" src/services/ src/llm/ src/events/ src/presentation/ src/ui_gpui/`
   - `grep -rn "// TODO\\|// FIXME\\|// HACK\\|// STUB" src/services/ src/llm/ src/events/ src/presentation/ src/ui_gpui/`
   - `grep -rn "placeholder\\|not yet implemented\\|will be implemented" src/services/`
5. Global-field removal: `grep -n "is_streaming: Arc<AtomicBool>\\|current_conversation_id: Arc<StdMutex<Option<Uuid>>>\\|stream_task: Arc<StdMutex<Option<JoinHandle" src/services/chat_impl.rs` — MUST be empty.
6. Per-conv map present: `grep -n "active_streams: Arc<StdMutex<HashMap<Uuid" src/services/chat_impl.rs` — MUST match once.
7. Plan markers present: `grep -c "@plan PLAN-20260416-ISSUE173.P03" src/services/` (use `-r`).

## Code inspection (MUST do)

- Open `src/services/chat_impl.rs`, read `begin_stream`, confirm it checks map
  membership, not a CAS.
- Read `cancel_active_stream`, confirm it takes `conversation_id: Uuid`, removes
  from the map, calls `cancel` and `task.abort()` on the removed entry, and
  emits `ChatEvent::StreamCancelled` with that id.
- Read the spawned-task's closure: confirm that on completion the task removes
  its own entry from `active_streams` (via `clear_streaming_state`).

## Verdict

- PASS: everything above passes.
- FAIL: any check fails.

Write `project-plans/issue173/plan/.completed/P03A.md` with the exact outputs
and code-inspection notes.
