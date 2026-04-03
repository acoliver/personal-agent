# Streaming Design — Phase B (Conditional Draft)

**Extracted from:** `project-plans/issue62/overview.md` §6
**Status:** CONDITIONAL DRAFT — pending mdstream validation gate
**Parent document:** [overview.md](overview.md) (Technical Specification)
**Last Updated:** 2026-04-02

> **Normative location:** `spec-phase-b.md` is the sole normative location for Phase B design details while conditional.

---

> **WARNING: CONDITIONAL DRAFT — pending mdstream validation gate.** This entire document is non-authoritative until the mdstream Dependency Validation Gate ([overview.md §4.9](overview.md#49-mdstream-api--assumed-pinned-to-v020)) passes. Phase A is fully self-contained without any content from this document. If mdstream validation fails, the [Normative Fallback Architecture](overview.md#normative-fallback-architecture-if-mdstream-validation-fails) applies. All requirements, acceptance criteria, and state machine definitions herein are tentative and subject to revision after mdstream API validation.

---

## 6.1 MdStream Placement

`MdStream` lives in `ChatView` (view-local), not in the store. The store remains rendering-agnostic — it provides `stream_buffer: String` through `StreamingStoreSnapshot`.

New fields on `ChatView`:

```rust
pub struct ChatView {
    // ... existing fields ...
    md_stream: MdStream,
    md_stream_fed_bytes: usize,
    md_stream_finalized: bool,  // idempotency guard for finalize()
}
```

**Note:** These fields are added only in **Rollout Phase B**. Phase A works without `MdStream` by calling `render_markdown()` on the full streaming content each frame.

## 6.2 mdstream API Usage

> **[Phase B — Conditional]** All API signatures below are [ASSUMED] from upstream docs — see [overview.md §4.9](overview.md#49-mdstream-api--assumed-pinned-to-v020). They are tentative until the Dependency Validation Gate passes.

The crate `mdstream` (v0.2.0, MIT/Apache-2.0, by Latias94) provides:

- `MdStream::new(Options::default())` — construct a new stream
- `md_stream.append(chunk: &str) -> Update` — feed a chunk, get committed blocks + pending block
- `md_stream.finalize() -> Update` — commit any trailing pending content
- `md_stream.reset()` — clear all state for reuse

The `Update` struct contains:
- `committed: Vec<Block>` — newly committed blocks
- `pending: Option<Block>` — current pending block (if any)
- `reset: bool` — if true, consumers must drop all previously rendered state and rebuild
- `invalidated: Vec<BlockId>` — previously committed blocks that need re-rendering

Each `Block` has:
- `id: BlockId` — stable identifier
- `kind: BlockKind` — paragraph, heading, code fence, etc.
- `raw: String` — the raw markdown text for this block
- `display: Option<String>` — transformed display text (if pending transformers are active)

Feature flags: `mdstream` has an optional `pulldown` feature that provides a `PulldownAdapter` with `committed_events()` and `parse_pending()` methods. This is useful but not required — the simpler approach is to call `pulldown_cmark::Parser::new()` on `block.raw` directly. The `pulldown` feature should be evaluated during implementation; the adapter provides caching benefits for committed blocks but adds coupling.

## 6.3 Streaming State Precedence

> **[Phase B — Conditional]** These precedence rules apply only when Phase B is active.

Two state machines co-exist during streaming. Their responsibilities are distinct, and precedence rules must be clear:

| Concern | Authority | Description |
|---|---|---|
| **Whether we are streaming** | `StreamingState` (in `ChatState`) | `StreamingState::Streaming { content, done }` is the sole authority for whether a stream is active. |
| **Block splitting for rendering** | `MdStream` (in `ChatView`) | `MdStream` is a rendering optimization; it splits the content into committed + pending blocks for efficient re-rendering. |

**Important note on the `done` field:** In the current implementation, `StreamingState::Streaming { content, done }` always has `done == false`. The `streaming_state_from_snapshot()` function (in `mod.rs`) constructs `Streaming { content: stream_buffer.clone(), done: false }` unconditionally when streaming is active. Stream completion is detected by the transition to `StreamingState::Idle` — specifically, when `active_target.is_none()` and `stream_buffer.is_empty()` in the next snapshot. The `done` field exists in the enum variant but is not used as a completion signal.

**Precedence rules:**

1. `StreamingState` is the source of truth for stream lifecycle. If `StreamingState` is `Idle` or `Error`, no streaming rendering occurs regardless of `MdStream` state.
2. `MdStream` is derived state — it is fed from `StreamingState::Streaming { content, .. }` and must be reset whenever `StreamingState` transitions to a non-streaming state.
3. When `StreamingState` transitions from `Streaming` to `Idle` (stream completion), the ChatView calls `md_stream.finalize()` to commit any trailing pending block, then resets.
4. If `MdStream` and `StreamingState` disagree (e.g., `MdStream` has pending blocks but `StreamingState` is `Idle`), `StreamingState` wins and `MdStream` is reset.

## 6.4 Authoritative Transition Handler Location

> **[Phase B — Conditional]** All transition handlers below are Phase B additions (pending validation gate).

**Store-driven transitions** are handled in `apply_store_snapshot()`. This is the single authoritative location for transitions triggered by store state changes:
- Token arrival (new bytes in `stream_buffer`)
- Stream completion (transition from `Streaming` to `Idle` when `active_target.is_none()`)
- Stream error (transition to `Error`)

**User-action transitions** are handled in their respective local action hooks, immediately after setting `StreamingState::Idle`:
- Escape key → `handle_key_down()` Escape branch
- Stop button → `render_send_stop_button()` Stop button handler
- New conversation (Cmd+N) → `handle_platform_key()` "n" branch
- New conversation (+ button) → `render_bars.rs` "+" button handler
- Conversation switch → `select_conversation_at_index()`
- ConversationCleared command → `handle_command()` match arm

**Idempotency rules:**
- **`reset()` is always safe to call multiple times.** Calling `md_stream.reset()` on an already-empty `MdStream` is a no-op. If there is any doubt about whether a reset has occurred, call reset again — it is harmless.
- **`finalize()` is guarded by a `finalized` flag.** The `md_stream_finalized` boolean prevents double-finalization. Before calling `finalize()`, check `!self.md_stream_finalized`. After calling `finalize()`, set `self.md_stream_finalized = true`. The `reset_md_stream()` helper (see below) clears this flag.

**Canonical reset helper:** All transition points use a single helper method to ensure consistency:

```rust
impl ChatView {
    /// Reset mdstream state. Safe to call multiple times (idempotent).
    fn reset_md_stream(&mut self) {
        self.md_stream.reset();
        self.md_stream_fed_bytes = 0;
        self.md_stream_finalized = false;
    }

    /// Finalize and reset mdstream state. Only finalizes once per stream.
    fn finalize_and_reset_md_stream(&mut self) {
        if !self.md_stream_finalized {
            let _final_update = self.md_stream.finalize();
            self.md_stream_finalized = true;
            // Process _final_update.committed if needed
        }
        self.reset_md_stream();
    }
}
```

### 6.4.1 Transition: Stream Start

**Trigger:** User sends a message (Send button click or Enter key).
**Code path:** `render_send_stop_button()` Send button `on_mouse_down` handler, or `handle_enter()` → emit `UserEvent::SendMessage`.
**Observable signal:** `self.state.streaming` transitions from `Idle` to `Streaming { content: "", done: false }`.
**mdstream action:** No explicit action needed — `MdStream` starts empty and `md_stream_fed_bytes` should already be `0` from the previous reset. **Defensive guard:** Assert `md_stream_fed_bytes == 0` at stream start. If non-zero, log a warning and call `reset_md_stream()`.

### 6.4.2 Transition: Token Arrival (Steady State)

**Trigger:** Store subscription fires with new `StreamingStoreSnapshot` containing updated `stream_buffer`.
**Code path:** `apply_store_snapshot()` calls `streaming_state_from_snapshot()` which reads `streaming.stream_buffer` and `streaming.active_target`.
**Observable signal:** `self.state.streaming` is `Streaming { content, .. }` and `content.len() > md_stream_fed_bytes`.
**mdstream action:** Delta-feed the new bytes via `md_stream.append(delta)`. See [§6.6](#66-delta-feeding-with-utf-8-safety).

### 6.4.3 Transition: Stream Completes Normally

**Trigger:** The LLM finishes generating. The store clears `active_target` and the stream buffer is incorporated into the conversation transcript.
**Code path:** `apply_store_snapshot()` → `streaming_state_from_snapshot()` returns `Idle` (because `active_target.is_none()` and `stream_buffer.is_empty()`). The transcript now contains the completed message.
**Observable signal:** `self.state.streaming` transitions from `Streaming { .. }` to `Idle`, and `self.state.messages` gains a new assistant message.
**mdstream action:** Call `self.finalize_and_reset_md_stream()`.
**Race prevention:** The snapshot is applied atomically in a single `apply_store_snapshot()` call. There is no window where streaming state and message list are inconsistent within a single snapshot application.

### 6.4.4 Transition: User Presses Escape (Stop Streaming)

**Trigger:** User presses Escape while streaming is active.
**Code path:** `handle_key_down()` Escape branch. Sets `self.state.streaming = StreamingState::Idle`, emits `UserEvent::StopStreaming`.
**Observable signal:** `self.state.streaming` transitions from `Streaming` to `Idle` synchronously in the key handler.
**mdstream action:** `self.reset_md_stream()`. Must be done in the same Escape handler, immediately after setting `StreamingState::Idle`. No `finalize()` — user explicitly aborted.

### 6.4.5 Transition: Stop Button Click

**Trigger:** User clicks the "Stop" button in the input bar while streaming.
**Code path:** `render_send_stop_button()` Stop button `on_mouse_down` handler. Sets `self.state.streaming = StreamingState::Idle`, emits `UserEvent::StopStreaming`.
**Observable signal:** Same as Escape — `Streaming` → `Idle`.
**mdstream action:** `self.reset_md_stream()`. No `finalize()` — explicit abort.

### 6.4.6 Transition: New Conversation (Cmd+N or + Button)

**Trigger:** User presses Cmd+N or clicks the "+" button.
**Code path:** `handle_platform_key()` "n" branch, or `render_bars.rs` "+" button handler. Both clear all state and set `StreamingState::Idle`.
**Observable signal:** `self.state.streaming` set to `Idle`, `messages` cleared, `active_conversation_id` set to `None`.
**mdstream action:** `self.reset_md_stream()`. Must be added to both the Cmd+N handler and the "+" button handler.

### 6.4.7 Transition: Conversation Switch (Dropdown Selection)

**Trigger:** User selects a different conversation from the dropdown.
**Code path:** `select_conversation_at_index()`. If currently streaming, emits `UserEvent::StopStreaming`.
**Observable signal:** `switching_conversation == true`. The next `apply_store_snapshot()` will deliver the new conversation's transcript.
**mdstream action:** `self.reset_md_stream()`. Must be done in `select_conversation_at_index()` when `switching_conversation` is true.

### 6.4.8 Transition: Stream Error

**Trigger:** Store reports an error (API failure, network error, etc.).
**Code path:** `apply_store_snapshot()` → `streaming_state_from_snapshot()` returns `StreamingState::Error(message)` when `streaming.last_error` is `Some`.
**Observable signal:** `self.state.streaming` transitions to `Error(...)`.
**mdstream action:** `self.reset_md_stream()`. No `finalize()` — error state means partial content is unreliable.

### 6.4.9 Transition: ConversationCleared Command

**Trigger:** `ViewCommand::ConversationCleared` arrives via `handle_command()`.
**Code path:** `command.rs` `handle_command()` match arm. Sets `StreamingState::Idle`, clears messages.
**Observable signal:** `self.state.streaming` set to `Idle`.
**mdstream action:** `self.reset_md_stream()`. Must be added to the `ConversationCleared` handler.

### 6.4.10 Transition: Idle Without Active Target (Snapshot-Inferred Completion)

**Trigger:** A snapshot arrives where `active_target.is_none()` and `stream_buffer.is_empty()`, but the previous state was `Streaming`.
**Code path:** `apply_store_snapshot()` → `streaming_state_from_snapshot()` returns `Idle`.
**Observable signal:** `self.state.streaming` transitions from `Streaming` to `Idle`. This is the normal completion path (same as §6.4.3), but is listed separately to emphasize that completion is inferred, not signaled.
**mdstream action:** Same as §6.4.3 — `self.finalize_and_reset_md_stream()`.

### 6.4.11 Race Prevention and Failure Modes

**Coalesced snapshots:** Multiple store snapshots may be coalesced into a single `apply_store_snapshot()` call if the UI thread falls behind. This means the `stream_buffer` may jump by more than one token. This is safe because `md_stream.append()` accepts any-length delta — it doesn't require single-token granularity.

**Skipped transitions:** If `StreamingState` jumps from `Streaming` directly to `Error` or `Idle` without an intermediate step, the mdstream reset still occurs because every non-`Streaming` state triggers a reset.

**Stale `md_stream_fed_bytes`:** If `md_stream_fed_bytes > 0` when a new stream starts, it indicates the previous stream was not properly cleaned up. The stream-start defensive guard ([§6.4.1](#641-transition-stream-start)) catches this and forces a reset with a warning log.

**Double-reset safety:** Calling `reset_md_stream()` on an already-empty state is a no-op. Multiple reset calls are harmless (idempotent by design).

**Double-finalize safety:** The `md_stream_finalized` flag prevents `finalize()` from being called twice on the same stream. If `finalize_and_reset_md_stream()` is called when already finalized, it skips the `finalize()` call and proceeds directly to `reset_md_stream()`.

## 6.5 Streaming Finalization Transition Table — Acceptance Criteria

> **[Phase B — Conditional]** This table defines **tentative** behavior (pending validation gate). Each row must be verified by a test.

| # | Transition | Previous State | New State | `finalize()` first? | `reset()` | Set `fed_bytes = 0` | Rationale | Test ID |
|---|---|---|---|---|---|---|---|---|
| F1 | Stream completes normally | `Streaming` | `Idle` (new message in transcript) | **YES** | YES | YES | Pending content must be committed before discard — it may contain the final partial block. | `test_finalize_on_normal_completion` |
| F2 | User presses Escape | `Streaming` | `Idle` | **NO** | YES | YES | User explicitly aborted — partial content is discarded. No need to commit pending block. | `test_reset_on_escape` |
| F3 | Stop button click | `Streaming` | `Idle` | **NO** | YES | YES | Same as Escape — explicit abort. | `test_reset_on_stop_button` |
| F4 | Conversation switch | `Streaming` | `Idle` (switching) | **NO** | YES | YES | Switching away discards the current stream entirely. Content is not committed to any transcript. | `test_reset_on_conversation_switch` |
| F5 | New conversation (Cmd+N / +) | `Streaming` | `Idle` (new convo) | **NO** | YES | YES | New conversation clears everything. Partial stream is abandoned. | `test_reset_on_new_conversation` |
| F6 | Stream error | `Streaming` | `Error(msg)` | **NO** | YES | YES | Error state — partial content is unreliable and should not be committed. | `test_reset_on_stream_error` |
| F7 | Idle without active target | `Streaming` | `Idle` (inferred) | **YES** | YES | YES | Same as F1 — this IS the normal completion path, just noting the inference mechanism. | `test_finalize_on_idle_inferred` |
| F8 | ConversationCleared command | `Streaming` or `Idle` | `Idle` (cleared) | **NO** | YES | YES | Explicit clear — all state discarded. | `test_reset_on_conversation_cleared` |
| F9 | Already Idle, new stream starts | `Idle` | `Streaming` | N/A | Defensive reset if `fed_bytes > 0` | YES (defensive) | Guard against stale state from prior stream. | `test_defensive_reset_on_stream_start` |

**Key principle:** `finalize()` is called **only** when the stream completed successfully and the pending content should be preserved (F1, F7). All explicit aborts (F2–F6, F8) skip `finalize()` and go straight to `reset()` because the partial content is being discarded. The `md_stream_finalized` flag ensures `finalize()` is never called twice.

**Acceptance criteria:** Each row's Test ID column identifies a test that must exist and pass. These tests verify:
1. The correct method sequence (`finalize_and_reset_md_stream()` vs. `reset_md_stream()` only)
2. That `md_stream_fed_bytes == 0` after the transition
3. That `md_stream_finalized == false` after the transition (reset clears it)
4. That `MdStream` internal state is empty after the transition

## 6.6 Delta Feeding with UTF-8 Safety

> **[Phase B — Conditional]** Delta feeding is a Phase B mechanism (pending validation gate).

On each `apply_store_snapshot()`, the ChatView compares the new `stream_buffer.len()` against `md_stream_fed_bytes`:

```rust
let new_len = streaming.stream_buffer.len();
if new_len > self.md_stream_fed_bytes {
    // UTF-8 safety: the feed offset must be a char boundary.
    // The stream_buffer grows by appending complete UTF-8 strings from the LLM,
    // so md_stream_fed_bytes is always a char boundary — but we assert this
    // rather than trusting the invariant silently.
    //
    // In safe Rust, indexing a &str at a non-char-boundary panics (it does NOT
    // cause undefined behavior — Rust's type system prevents UB in safe code).
    // The assert! here makes the panic intentional and provides a clear message
    // rather than an opaque "byte index is not a char boundary" panic from
    // the standard library.
    assert!(
        streaming.stream_buffer.is_char_boundary(self.md_stream_fed_bytes),
        "md_stream feed offset {} is not a char boundary in stream_buffer of len {}",
        self.md_stream_fed_bytes,
        new_len,
    );
    let delta = &streaming.stream_buffer[self.md_stream_fed_bytes..new_len];
    let update = self.md_stream.append(delta);
    self.md_stream_fed_bytes = new_len;
    // Process update.committed and update.pending for rendering
}
```

**Important:** This uses `assert!`, not `debug_assert!`. The `debug_assert!` macro is compiled away in release builds, which would silently allow a non-char-boundary offset to reach the string slice operation, where it would panic with a less informative message. This was identified as a critical issue in the CodeRabbit review (item #1).

## 6.7 Cursor ▋ Invariants

> **[Phase B — Conditional]** These invariants describe cursor behavior in the Phase B streaming path.

The streaming cursor character (`▋`, U+258B LEFT FIVE EIGHTHS BLOCK) has strict placement rules:

1. **Never in committed blocks.** The cursor is appended only to the pending block's display text, not to its `raw` field. When a block transitions from pending to committed, the cursor is not part of the committed text.

2. **Never in finalized transcript.** When streaming ends and the final message is added to `ChatState.messages`, the cursor character must not be present in `ChatMessage.content`. The cursor exists only in the `AssistantBubble` rendering path when `self.is_streaming == true`.

3. **Appended at render time only.** The cursor is appended in `AssistantBubble::into_element()` via `format!("{}▋", self.content)` — this happens during rendering, not during state updates. The `StreamingState::Streaming { content, .. }` field and `MdStream` block text never contain the cursor.

4. **Assertion:** Any code that commits streaming content to the message history must not include the cursor. This is naturally enforced because `AssistantBubble` only appends the cursor when `self.is_streaming == true`, and the content string passed to the message history comes from `StreamingState::Streaming { content, .. }`, not from the rendered output.

## 6.8 Rendering from mdstream Output

> **[Phase B — Conditional]** This rendering path replaces Phase A's full re-parse strategy.

The render path for streaming messages uses the mdstream blocks:

1. **Committed blocks:** Each committed block's `raw` text is passed through `render_markdown()`. Since committed blocks are stable (their content won't change), their rendered elements produce identical output each frame. (Phase 2 may cache these.)

2. **Pending block:** The pending block's display text (via `display_or_raw()`) is passed through `render_markdown()`. This is re-parsed each frame but is typically a single paragraph — O(1) per token.

3. **Assembly:** Committed elements and pending elements are concatenated and rendered as children of the streaming message div.

## 6.9 Finalization

> **[Phase B — Conditional]** Finalization logic applies only when Phase B streaming is active.

When streaming ends (the `StreamingState` transitions away from `Streaming`), the ChatView calls:

```rust
// Only on successful completion (F1, F7):
self.finalize_and_reset_md_stream();

// On all other transitions (F2-F6, F8):
self.reset_md_stream();
```

This ensures that any content in the pending block at stream-end is committed for successful completions, while explicit aborts discard partial content cleanly.

**Important:** `finalize()` is only called on successful stream completion (transitions F1, F7 in the [Finalization Transition Table](#65-streaming-finalization-transition-table--acceptance-criteria)). Explicit aborts (Escape, Stop, conversation switch, new conversation, error, clear) call `reset_md_stream()` directly without `finalize()`. The `md_stream_finalized` flag prevents double-finalization.

## 6.10 Reset Matrix

> **[Phase B — Conditional]** All entries in this matrix are Phase B additions (pending validation gate).

All transitions that end or change a stream must fully reset mdstream state. Cross-references to [§6.4](#64-authoritative-transition-handler-location) and the [Finalization Transition Table](#65-streaming-finalization-transition-table--acceptance-criteria) provided for each:

| Transition | Code Location | Reset Action | §6.4 Ref | Finalize Table Ref |
|---|---|---|---|---|
| Stream completes normally | `apply_store_snapshot()` | `finalize_and_reset_md_stream()` | §6.4.3 | F1 |
| Idle without active target (inferred) | `apply_store_snapshot()` | `finalize_and_reset_md_stream()` | §6.4.10 | F7 |
| User presses Escape (stop streaming) | `handle_key_down()` Escape branch | `reset_md_stream()` | §6.4.4 | F2 |
| Stop button click | `render_send_stop_button()` Stop button handler | `reset_md_stream()` | §6.4.5 | F3 |
| New conversation (Cmd+N) | `handle_platform_key()` "n" branch | `reset_md_stream()` | §6.4.6 | F5 |
| New conversation (+ button) | `render_bars.rs` "+" button handler | `reset_md_stream()` | §6.4.6 | F5 |
| Conversation switch (dropdown select) | `select_conversation_at_index()` | `reset_md_stream()` | §6.4.7 | F4 |
| Stream error | `apply_store_snapshot()` | `reset_md_stream()` | §6.4.8 | F6 |
| ConversationCleared command | `handle_command()` | `reset_md_stream()` | §6.4.9 | F8 |
