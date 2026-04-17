# Issue #168 — Conversation Isolation for Streaming + Tool Approvals (GPUI)

## Problem Summary

Issue #168 reports conversation-state hijacking in GPUI chat:

- Conversation **A** is streaming and/or waiting on tool approval.
- User switches to or creates Conversation **B**.
- A’s in-flight UI state appears in B.

This is caused by selection-coupled state handling (global streaming snapshot + selection-gated reducers + unscoped approval events).

## Verified Root Causes in Current Code

1. Global streaming snapshot in store:
   - `ChatStoreSnapshot.streaming: StreamingStoreSnapshot`
   - `src/ui_gpui/app_store_types.rs`
2. Streaming reducers write only when target matches selected conversation:
   - `*_if_target_matches_selected_or_nil`
   - `src/ui_gpui/app_store_streaming.rs`
3. Selection switch clears visible streaming state:
   - `begin_selection_locked`
   - `src/ui_gpui/app_store.rs`
4. Approval commands lack conversation ownership:
   - `ViewCommand::ToolApprovalRequest/Resolved`
   - `src/presentation/view_command.rs`
5. Tool context lacks conversation ID:
   - `McpToolContext`
   - `src/llm/client_agent.rs`
6. ChatView approval bubbles are global to the view (`Vec`) and cleared on switch:
   - `src/ui_gpui/views/chat_view/state.rs`
   - `src/ui_gpui/views/chat_view/mod.rs`

## Scope

### In scope

- Isolate streaming/thinking state per conversation in store internals.
- Route tool-approval request/resolve events with conversation ownership.
- Isolate approval bubble state per conversation in ChatView.
- Preserve in-flight state across conversation switching.
- Add tests that lock the above behavior.

### Out of scope

- Multi-stream backend/service redesign (`ChatServiceImpl` remains single active stream).
- Introducing or wiring `ShowToolCall`/`UpdateToolCall` UI behavior.

## Acceptance Criteria

1. Switching away from A while A streams does not show A stream/thinking in B.
2. Tool approval for A never renders in B.
3. Creating a new conversation during A’s stream does not hijack A’s state.
4. Switching back to A restores A’s in-flight stream/thinking/approvals.
5. Finalize/cancel/error clear only the targeted conversation’s ephemeral streaming state.

---

## Design Decisions

## D1) Per-conversation streaming state lives in `AppStoreInner` (not published map)

To avoid cloning a full streaming-state map on every streaming token publish, the map is internal:

- Add `ConversationStreamingState` in `src/ui_gpui/app_store_types.rs`:
  - `thinking_visible`
  - `thinking_buffer`
  - `stream_buffer`
  - `last_error`
  - `model_id`
- Add to `AppStoreInner` in `src/ui_gpui/app_store.rs`:
  - `streaming_states: HashMap<Uuid, ConversationStreamingState>`
  - `active_streaming_target: Option<Uuid>`
- Keep `ChatStoreSnapshot.streaming: StreamingStoreSnapshot` as a **projected selected-conversation view**.

### Projection rule

On reducer changes and selection changes, recompute `snapshot.chat.streaming` from:

- selected conversation’s `ConversationStreamingState`
- whether `active_streaming_target == selected_conversation_id`

This keeps published snapshots small and avoids hot-path map cloning.

## D2) Reducers become ownership-targeted, not selection-targeted

In `app_store_streaming.rs` + app store reducer call sites:

- Remove selected-conversation guards for append/show/hide/finalize/cancel/error writes.
- Resolve target conversation and mutate `inner.streaming_states[target]`.
- `finalize` behavior:
  - If target is selected: append assistant payload to selected transcript, then clear target ephemera.
  - If target is background: clear target ephemera only, do not mutate selected transcript.
- Cleanup removed conversations:
  - Remove their `streaming_states` entry and clear `active_streaming_target` if needed.

## D3) Selection does not destroy ownership state

In `begin_selection_locked`:

- Stop clearing owned streaming buffers on switch.
- Selection changes only update selected conversation metadata/load state and re-project visible streaming snapshot.

## D4) Finalization guard remains selected-transcript-scoped

Keep `last_finalized_stream_guard: Option<FinalizedStreamGuard>` (no per-conversation map).

Rationale:

- Snapshot transcript is selected-conversation scoped.
- Duplicate-append suppression is only needed for selected transcript path.

## D5) Tool approvals carry conversation ownership end-to-end

- Add `conversation_id: Uuid` to `McpToolContext`.
- Pass it from stream setup (`chat_impl.rs` + `chat_impl/streaming.rs`).
- Extend `ViewCommand`:
  - `ToolApprovalRequest { request_id, context, conversation_id }`
  - `ToolApprovalResolved { request_id, approved, conversation_id }`
- Update all native tool executors + MCP executor to include conversation_id on request emission.
- Update chat service approval resolution to include conversation_id on resolved emission.
  - Use existing single-stream invariant via `current_conversation_id` for resolved tagging.

## D6) ChatView approval bubbles become per-conversation map

- `ChatState.approval_bubbles: HashMap<Uuid, Vec<ToolApprovalBubble>>`
- Request handling inserts/groups in `approval_bubbles[conversation_id]`.
- Resolved handling mutates/removes only in the same conversation bucket.
- Remove `approval_bubbles.clear()` in `apply_store_snapshot` conversation-switch path.
- Render only pending bubbles for active/selected conversation bucket.

---

## Test-First Plan (RED → GREEN → REFACTOR)

## Phase A — Store streaming isolation (internal map + projection)

### RED

Update/add tests in:

- `src/ui_gpui/app_store_streaming/tests.rs`
- `src/ui_gpui/app_store/tests.rs`

Required failing tests first:

1. Appending stream/thinking for background target updates `inner.streaming_states[target]`.
2. Selecting B after A stream preserves A owned state (no destructive clear).
3. Switching back to A re-projects A stream/thinking to snapshot.
4. Finalize for background target clears target ephemera only (no transcript corruption).
5. Cancel/error clear only targeted state entry.
6. Deleting conversation removes its `streaming_states` entry and active target if matching.

### GREEN

Implement in:

- `src/ui_gpui/app_store_types.rs`
- `src/ui_gpui/app_store_streaming.rs`
- `src/ui_gpui/app_store.rs`

### REFACTOR

- Remove selection-coupled helper naming.
- Consolidate target-state + projection helper utilities.

## Phase B — Approval ownership plumbing

### RED

Add failing tests in:

- `src/services/chat_impl/tests.rs`
- `src/ui_gpui/views/main_panel/tests/tool_approval.rs`
- tool executor tests in changed files

Required failing tests first:

1. Approval request includes correct conversation_id.
2. Approval resolved includes correct conversation_id.
3. MainPanel forward path preserves conversation_id.
4. Stop/deny flow emits resolved with conversation ownership.

### GREEN

Implement in:

- `src/llm/client_agent.rs`
- `src/services/chat_impl.rs`
- `src/services/chat_impl/streaming.rs`
- `src/presentation/view_command.rs`
- `src/agent/tools/read_file.rs`
- `src/agent/tools/write_file.rs`
- `src/agent/tools/edit_file.rs`
- `src/agent/tools/search.rs`
- `src/agent/tools/shell_exec.rs`
- `src/agent/tools/activate_skill.rs`
- `src/llm/mcp_tool_executor.rs`
- plus all compile-driven pattern-match updates

### REFACTOR

- Factor approval-request command construction helper(s) where practical.

## Phase C — ChatView per-conversation approvals

(Depends on Phase B command shape changes)

### RED

Add failing tests in:

- `src/ui_gpui/views/chat_view/approval_tests.rs`
- `src/ui_gpui/views/chat_view/mod_tests.rs`

Required failing tests first:

1. Request for A while B is active stores in A bucket, not B.
2. Resolve for A does not mutate B bucket.
3. Switch no longer clears all approvals.
4. Rendering shows only active conversation’s pending bubble queue.

### GREEN

Implement in:

- `src/ui_gpui/views/chat_view/state.rs`
- `src/ui_gpui/views/chat_view/command.rs`
- `src/ui_gpui/views/chat_view/mod.rs`
- `src/ui_gpui/views/chat_view/render.rs`

### REFACTOR

- Add helper accessor methods for active conversation bubble lists.

---

## Verification

Run full project verification:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test --lib --tests`
4. `python -m lizard -C 50 -L 100 -w src/`

---

## PR and Remediation Flow

1. Commit focused changes.
2. Push `issue168`.
3. Open PR title containing issue number and fix clause:
   - `Fix conversation isolation for streaming and tool approvals in GPUI (Fixes #168)`
4. Include verification evidence in PR body.
5. Watch checks:
   - `gh pr checks <PR_NUM> --watch --interval 300`
6. Resolve CI failures + CodeRabbit items; comment on each issue action.
7. Re-run full verification after remediation cycles.

---

## Done Definition

Complete when:

- Acceptance criteria are covered by tests and validated behavior.
- Full verification suite is green locally.
- PR includes `Fixes #168`, CI is green, and CodeRabbit issues are addressed/resolved with comments.