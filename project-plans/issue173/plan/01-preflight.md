# Phase 01: Preflight Verification

Plan ID: `PLAN-20260416-ISSUE173.P01`

## Purpose

Verify every assumption in `specification.md` before any code is written.

## Verifications required

Produce a preflight report with **actual command output** (not summaries) for
each of the following:

1. **Dependency**: `tokio-util` availability with the `sync` feature (for
   `CancellationToken`):
   - `grep -n "tokio-util" Cargo.toml`
   - If missing or missing feature `sync`, note it — the impl phase will add it.

2. **Current service globals** confirm their exact location/line numbers so
   refactor plans are accurate:
   - `grep -n "is_streaming\\|current_conversation_id\\|stream_task" src/services/chat_impl.rs src/services/chat_impl/streaming.rs`

3. **Trait**: `grep -n "fn cancel\\|fn is_streaming" src/services/chat.rs`

4. **Event variant**: `grep -n "StopStreaming" src/events/types.rs src/ui_gpui/views/chat_view/render.rs src/presentation/chat_presenter.rs`

5. **Approval gate**: verify `PendingApproval` already has `conversation_id`
   (landed in PR #170):
   - `grep -n "PendingApproval\\|conversation_id" src/llm/client_agent.rs | head -40`

6. **Store inner state**:
   - `grep -n "active_streaming_target\\|streaming_states" src/ui_gpui/app_store.rs`

7. **Store snapshot types**:
   - `grep -n "StreamingStoreSnapshot\\|HistoryStoreSnapshot\\|project_streaming_snapshot\\|project_history_snapshot" src/ui_gpui/app_store_types.rs src/ui_gpui/app_store.rs`

8. **Sidebar render site**:
   - `find src/ui_gpui/views -name render_sidebar.rs`
   - Read the file to identify the exact conversation-row render function.

9. **Build baseline**: run `cargo build --all-targets` and record the last 10
   lines of output.

10. **Test baseline**: run `cargo test --lib --tests -q 2>&1 | tail -20` and
    record the passing test count at baseline.

## Deliverable

Write `project-plans/issue173/plan/.completed/P01.md` containing:

```markdown
# Phase P01 Preflight Results

## Verdict: PASS | FAIL

## Command outputs
(each numbered section above with EXACT command output)

## Discrepancies found
(If anything in specification.md needs adjusting, list it here. If none, say "none".)

## Baseline metrics
- cargo build: PASS/FAIL (last 10 lines)
- cargo test: <N> passed, <M> failed
```

Report PASS only if:
- All greps returned the expected items,
- `cargo build --all-targets` succeeded,
- `cargo test --lib --tests` succeeded at baseline.
