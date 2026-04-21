# Feature Specification: Concurrent Conversations (Parallel Streams)

Plan ID: `PLAN-20260416-ISSUE173`
Issue: #173
Depends on: #168 / PR #170 (landed — per-conversation store/UI isolation)

## Purpose

Enable true concurrent conversations — ChatGPT-style. A user must be able to start
streams in conversations A, B, and C in any order and have all three run in
parallel, each producing its own output, tool approvals, and lifecycle events,
without any of them cancelling or interfering with the others. Switching between
conversations shows each one's live buffer.

## Background

PR #170 isolated the **UI/store** side: in-flight streaming buffers and tool
approval bubbles are now per-conversation, and finalize guards prevent A's output
from migrating into B. That PR explicitly kept the **chat service** single-stream
(see `project-plans/issue168/overview.md` → "Out of scope").

This plan completes the picture on the service layer, the cancel pipeline, the
approval gate, the app store `active_streaming_target`, and the sidebar.

## Architectural Decisions

- **Pattern**: per-conversation ownership in the service layer via
  `HashMap<Uuid, ActiveStream>` replacing three global fields.
- **Cancellation**: `CancellationToken` (tokio-util) per stream in addition to
  `JoinHandle::abort()`, giving tool loops a cooperative cancel point while still
  supporting hard abort.
- **Concurrency policy**:
  - Unlimited concurrent streams **across** conversations.
  - Within a single conversation: **reject** a second concurrent send with a
    clear error (`"Stream already in progress for this conversation"`). Queueing
    within a conversation is an explicit follow-up.
- **Event propagation**: `UserEvent::StopStreaming { conversation_id }` threads a
  conversation id through presenter → `ChatService::cancel(conversation_id)` →
  `cancel_active_stream(conversation_id)`.
- **Approval gate scoping**: `ApprovalGate` already stores `conversation_id` on
  each `PendingApproval` (from PR #170). We add
  `resolve_all_for_conversation(conversation_id, approved) -> Vec<(Uuid, String)>`
  and use it from `cancel_active_stream`.
- **Store**: `active_streaming_target: Option<Uuid>` becomes
  `active_streaming_targets: HashSet<Uuid>`. `StreamingStoreSnapshot::active_target`
  remains `Option<Uuid>` at the projection boundary (selected conversation only)
  to avoid churn in consuming views, but the underlying set drives a new
  `HistoryStoreSnapshot.streaming_conversation_ids: HashSet<Uuid>` so the sidebar
  can render a live indicator for background streams.
- **Sidebar indicator**: subtle pulsing dot on conversation rows whose id is in
  `history.streaming_conversation_ids`.

## Integration Points (MANDATORY)

### Existing code that will USE the new API

- `src/presentation/chat_presenter.rs` — `handle_stop_streaming` will call
  `chat_service.cancel(conversation_id)` with the id from the user event.
- `src/ui_gpui/views/chat_view/render.rs` — stop button and escape key handler
  emit `UserEvent::StopStreaming { conversation_id }` using the selected
  conversation id already in view state.
- `src/ui_gpui/views/render_sidebar.rs` (currently lives under
  `src/ui_gpui/views/chat_view/render_sidebar.rs`) — checks
  `history.streaming_conversation_ids.contains(&conversation.id)` to draw the
  indicator.
- `src/ui_gpui/app_store.rs` — `project_history_snapshot` will populate
  `streaming_conversation_ids` from the inner `active_streaming_targets` set.

### Existing code to be REPLACED

- `ChatServiceImpl::{is_streaming, current_conversation_id, stream_task}` triple
  of globals — removed, replaced by `active_streams: HashMap<Uuid, ActiveStream>`.
- `UserEvent::StopStreaming` (unit variant) — replaced by struct variant with
  `conversation_id`.
- `ChatService::cancel(&self)` — replaced by `cancel(&self, conversation_id: Uuid)`.
- `ApprovalGate::resolve_all` call in `cancel_active_stream` — replaced by
  `resolve_all_for_conversation`.
- `AppStoreInner::active_streaming_target: Option<Uuid>` — replaced by
  `active_streaming_targets: HashSet<Uuid>`.

### User access points

- Any conversation can be streamed independently at any time via normal send UI.
- Stop button / Esc cancels only the currently viewed conversation.
- Sidebar shows a pulsing dot next to any conversation with an active stream,
  whether selected or not.

### Migration requirements

- None. All new state is in-memory; on-disk conversation format unchanged.

## Formal Requirements

### REQ-173-001 Concurrent service-layer streams
- [REQ-173-001.1] `begin_stream(A)` followed by `begin_stream(B)` both succeed
  when `A != B`, and both appear in `active_streams`.
- [REQ-173-001.2] `begin_stream(A)` while `A` is already in `active_streams`
  returns `ServiceError::Internal("Stream already in progress for this conversation")`.
- [REQ-173-001.3] A stream's spawned task removes its own entry from
  `active_streams` on completion, without touching other entries.

### REQ-173-002 Per-conversation cancellation
- [REQ-173-002.1] `cancel(A)` aborts only the task keyed by `A`; `B`'s task
  continues running.
- [REQ-173-002.2] `cancel(A)` emits `ChatEvent::StreamCancelled { conversation_id: A, .. }`
  and does not emit one for any other conversation.
- [REQ-173-002.3] `UserEvent::StopStreaming { conversation_id }` flows through the
  presenter and reaches `ChatService::cancel(conversation_id)` unchanged.

### REQ-173-003 Approval gate isolation
- [REQ-173-003.1] Resolving A's `request_id` does not send on B's waiter.
- [REQ-173-003.2] `resolve_all_for_conversation(A, false)` returns only A's
  pending requests and leaves B's in the map.
- [REQ-173-003.3] `cancel_active_stream(A)` calls
  `resolve_all_for_conversation(A, false)` — never `resolve_all`.

### REQ-173-004 Store multi-target tracking
- [REQ-173-004.1] `AppStoreInner.active_streaming_targets` inserts on stream
  start and removes on finalize/cancel, scoped to the specific target.
- [REQ-173-004.2] `project_streaming_snapshot` marks the selected conversation
  active iff it is contained in `active_streaming_targets`.
- [REQ-173-004.3] `project_history_snapshot` publishes
  `streaming_conversation_ids` = `active_streaming_targets` (cloned).

### REQ-173-005 Sidebar streaming indicator
- [REQ-173-005.1] When `conversation.id ∈ streaming_conversation_ids`, the
  sidebar row renders a streaming indicator (pulsing dot).
- [REQ-173-005.2] The indicator disappears when the id is removed from the set
  (finalize or cancel).
- [REQ-173-005.3] The currently selected conversation's own streaming state is
  still rendered inline in the chat view and is not duplicated by the sidebar
  indicator visually (indicator is a subtle dot, not a full "streaming…" label).

### REQ-173-006 End-to-end concurrency
- [REQ-173-006.1] An integration test drives three interleaved
  `send_message` calls on three distinct conversation ids using a deterministic
  mock `LlmClient`/`ConversationService` and asserts each conversation's final
  transcript independently, each conversation's `StreamCompleted` event, and
  that `cancel(B)` mid-flight leaves A and C's streams intact.

## Data Schemas

```rust
// src/services/chat_impl.rs
pub(super) struct ActiveStream {
    pub(super) task: tokio::task::JoinHandle<()>,
    pub(super) cancel: tokio_util::sync::CancellationToken,
}

// ChatServiceImpl fields (replaces is_streaming / current_conversation_id / stream_task)
active_streams: Arc<StdMutex<HashMap<Uuid, ActiveStream>>>,
```

```rust
// src/services/chat.rs
#[async_trait]
pub trait ChatService: Send + Sync {
    async fn send_message(&self, conversation_id: Uuid, content: String)
        -> ServiceResult<Box<dyn Stream<Item = ChatStreamEvent> + Send + Unpin>>;
    fn cancel(&self, conversation_id: Uuid);
    fn is_streaming(&self) -> bool;              // any stream
    fn is_streaming_for(&self, conversation_id: Uuid) -> bool; // new
    async fn resolve_tool_approval(...);
}
```

```rust
// src/events/types.rs
pub enum UserEvent {
    ...
    StopStreaming { conversation_id: Uuid },
    ...
}
```

```rust
// src/llm/client_agent.rs
impl ApprovalGate {
    pub fn resolve_all_for_conversation(&self, conversation_id: Uuid, approved: bool)
        -> Vec<(Uuid, String)>;
}
```

```rust
// src/ui_gpui/app_store.rs (inner state)
pub(super) active_streaming_targets: HashSet<Uuid>,

// src/ui_gpui/app_store_types.rs
pub struct HistoryStoreSnapshot {
    pub conversations: Vec<ConversationSummary>,
    pub selected_conversation_id: Option<Uuid>,
    pub streaming_conversation_ids: HashSet<Uuid>, // new
}
```

## Constraints

- No blocking on the main thread; all async work via tokio.
- Per-conversation lock (the whole `active_streams` map uses a std Mutex, held
  for constant-time insert/remove only — never while awaiting).
- Must keep `Cargo.toml` additions minimal; `tokio-util` with `sync` feature if
  not already enabled.

## Out of scope

- Wire-protocol / provider-specific session changes.
- Multi-device sync.
- Queueing of within-conversation sends (still rejected).
- API-key env-var race when different concurrent streams use different providers
  (documented limitation — the test suite will continue to use a single provider
  mock).

## Performance requirements

- Adding a 2nd and 3rd concurrent stream must not measurably slow the first
  stream's time-to-first-token.
- Sidebar indicator update latency ≤ one snapshot revision after
  `active_streaming_targets` changes.
