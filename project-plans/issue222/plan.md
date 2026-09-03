# Plan: Dedicated Stop control + mid-turn steering via Send

Plan ID: PLAN-20260903-ISSUE222
Generated: 2026-09-03
Issue: #222
Requirements: REQ-222-001 .. REQ-222-008

## Problem

During an active turn the chat offers exactly one control. `render_send_stop_button`
(`src/ui_gpui/views/chat_view/render.rs:740`) renders a single button that reads
"Stop" while streaming and "Send" otherwise, and the Send branch bails out with
`"Send button ignored while stream is active"`. `handle_enter`
(`src/ui_gpui/views/chat_view/mod.rs:822`) returns early on
`StreamingState::Streaming`. The only way to redirect the agent is to kill the
turn and re-send, which discards the work the turn already produced.

## Delivery boundary: what is buildable without a dependency change

The issue names the injection point as "after tool results, before the next model
call". That point exists in the dependency, not here:
`serdes-ai-agent/src/stream.rs` pushes `tool_req` onto `messages` and `continue`s
the loop inside a `tokio::spawn`ed task. `AgentStream`'s public surface is
`new` / `new_with_cancel` / `cancel` / `Stream` — there is no input channel, and
`grep -rn "steer" serdes-ai-agent/src/` at the pinned rev `e675674` returns
nothing.

Three shapes were considered:

1. **Upstream queue in `AgentStream`.** Drain a steering handle where `tool_req`
   is appended, in both `new` and `new_with_cancel`. Delivers at the tool
   boundary as the issue describes. Requires a commit on `acoliver/serdesAI` and
   a rev bump across all seven pinned `serdes-ai-*` entries in `Cargo.toml` —
   a dependency change, which is held for explicit approval and is **not** part
   of this plan.
2. **Drive `AgentRun::step()` locally.** Rejected: `step()` calls
   `model.request()`, not `request_stream()`, so adopting it would trade token
   streaming for steering. That is a regression against current behavior.
3. **Turn chaining, local only.** `AgentStreamEvent::RunComplete` carries the
   full `messages: Vec<ModelRequest>` for the turn. When a turn ends with a
   non-empty steering queue, start a follow-up `AgentStream` seeded with those
   messages as `message_history` and the steering text as the prompt, inside the
   same `run_stream_task`. No upstream change.

This plan implements **shape 3**. It satisfies the issue's stated user value
("without losing the work it has already produced") and every Decision recorded
on 2026-08-31: no implicit aborts, text-only turns deliver at end of turn,
mid-thinking sends wait for a real boundary. It does **not** deliver between two
tool calls of a single turn; a steer typed during a long tool sequence lands when
that sequence finishes. Closing that gap is shape 1 and is deferred pending
approval of the dependency change.

## Requirements

### REQ-222-001: Stop is a dedicated control

**Behavior**
- GIVEN a conversation whose stream is active
- WHEN the composer renders
- THEN a Stop control (`chat-stop-button`) and a Send control
  (`chat-send-button`) are both present, as separate elements
- AND when the conversation is idle, only `chat-send-button` is present.

Stop keeps its current behavior: emit `UserEvent::StopStreaming`, reset local
streaming state, refocus the composer.

### REQ-222-002: Send stays active during streaming

**Behavior**
- GIVEN an active stream and non-empty trimmed composer text
- WHEN the user clicks Send or presses Enter
- THEN `UserEvent::SteerStreaming { conversation_id, text }` is emitted
- AND `UserEvent::SendMessage` is **not** emitted
- AND the composer clears.

Whitespace-only input remains a no-op while streaming and while idle. Idle Send
and idle Enter keep emitting `SendMessage` unchanged.

### REQ-222-003: Queued steering messages are visible

**Behavior**
- GIVEN a steering message has been submitted and not yet delivered
- WHEN the transcript renders
- THEN the text appears as a queued entry, distinct from a sent user message
- AND when the message is delivered it stops rendering as queued.

### REQ-222-004: Active-turn precondition

**Behavior**
- GIVEN a conversation with no stream in `StreamLifecycle::Running`
- WHEN `ChatService::steer(conversation_id, text)` is called
- THEN it returns `Err(ServiceError::Validation(..))` naming no active turn
- AND the queued entry is withdrawn from the transcript with a visible error.

Steering reaches only the conversation that owns the running stream. A steer for
conversation A never lands on conversation B's turn.

### REQ-222-005: Delivery at a turn boundary

**Behavior**
- GIVEN a queued steering message and a running turn
- WHEN the turn reaches its boundary
- THEN the queued text is delivered to the model as a user message seeded with
  the completed turn's message history
- AND the queue is drained in FIFO order
- AND delivery happens without a new user-initiated send.

### REQ-222-006: No implicit abort

**Behavior**
- GIVEN a queued steering message
- WHEN it is submitted or delivered
- THEN the in-flight generation is never cancelled on the user's behalf
- AND `chat_service.cancel` is reached only from `UserEvent::StopStreaming`.

### REQ-222-007: Ordering survives reload

**Behavior**
- GIVEN a turn that produced tool results, then received a steering message,
  then produced assistant output
- WHEN the conversation is reloaded
- THEN the steering message is persisted as a user message ordered after the
  preceding assistant output and before the assistant output it produced.

### REQ-222-008: Approvals are not raced

**Behavior**
- GIVEN a pending tool approval for a conversation
- WHEN a steering message arrives for that conversation
- THEN the approval is neither resolved nor cancelled by the steer
- AND the steering message is delivered only after the turn passes the
  approval and reaches its boundary.

## Bounded queue

Queue depth is capped per conversation at `MAX_QUEUED_STEERING_MESSAGES = 5`.
A steer submitted against a full queue is rejected with
`ServiceError::Validation`, surfaced the same way as REQ-222-004. Unlimited
depth is rejected: an unbounded queue lets a user stack arbitrary instructions
that all flush at one boundary, which is the unpredictable-flush failure mode
reported against Claude Code (anthropics/claude-code#49373).

The steering text is delivered as a plain user message with no wrapper or
framing text.

## Out of scope

- Whether Stop preserves partial output (issue #218).
- The Responses websocket transport (#217).
- Mid-turn delivery between two tool calls (shape 1 above; needs the dependency
  change).

## Phases

### P01 — Events and service contract (test-first)

Files: `src/events/types.rs`, `src/services/chat.rs`, `src/services/chat_impl.rs`

- Add `UserEvent::SteerStreaming { conversation_id: Uuid, text: String }`.
- Add `ChatEvent::SteeringQueued { conversation_id, steer_id, text }` and
  `ChatEvent::SteeringDelivered { conversation_id, steer_id }`.
- Add `ChatService::steer(&self, conversation_id: Uuid, text: String) ->
  ServiceResult<Uuid>` returning the steer id.
- `ChatServiceImpl` gains a per-conversation FIFO queue keyed the same way as
  `active_streams`, guarded by the existing `StreamLifecycle::Running` check.

Tests (behavioral, `src/services/chat_impl/tests/`):
- steer with no active stream → `Err`, queue unchanged (REQ-222-004)
- steer with a running stream → `Ok`, entry queued, `SteeringQueued` emitted
- steer for conversation A while only B is running → `Err` (REQ-222-004)
- sixth steer against a full queue → `Err`, queue still holds five
- steer never calls `cancel` (REQ-222-006)

### P02 — Delivery at the turn boundary (test-first)

Files: `src/llm/client_agent.rs`, `src/services/chat_impl/streaming.rs`

- `do_run_agent_stream` returns the turn's final `Vec<ModelRequest>` from
  `RunComplete` instead of discarding it.
- `run_stream_task` loops: after `stream_agent_response` returns, drain the
  conversation's steering queue; for each drained message persist it as a user
  message, emit `SteeringDelivered`, and run another `AgentStream` seeded with
  the accumulated history. The loop exits when the queue is empty, the
  cancellation token fires, or the turn errored.
- The existing single-turn finalization runs once, on the last turn.

Tests:
- a queued steer produces a second model request carrying the steer text
  (REQ-222-005)
- FIFO order across two queued steers
- an empty queue leaves behavior byte-identical to today (regression)
- a cancelled turn drains nothing and starts no follow-up (REQ-222-006)
- a steer queued while an approval is pending is delivered after the decision
  (REQ-222-008)
- the steering user message persists between the preceding assistant output and
  the following one (REQ-222-007)

### P03 — Presenter wiring (test-first)

Files: `src/presentation/chat_presenter.rs`, `src/presentation/chat_presenter_event.rs`

- `handle_steer_streaming` calls `chat_service.steer`; on `Err` it sends
  `ViewCommand::SteeringRejected { conversation_id, error }` and a `ShowError`,
  on `Ok` nothing further (the service emits `SteeringQueued`). `SteeringRejected`
  carries no `steer_id`: `steer` returns `ServiceResult<Uuid>`, so a refusal
  yields an error and no id, and a refused steer was never queued under one.
- Route `ChatEvent::SteeringQueued` / `SteeringDelivered` to
  `ViewCommand::SteeringQueued` / `SteeringDelivered`.

Tests (`src/presentation/chat_presenter_tests.rs`): each branch asserts the
view commands emitted, using the existing mock `ChatService`.

### P04 — UI controls and queued rendering (test-first)

Files: `src/ui_gpui/views/chat_view/render.rs`, `mod.rs`, `state.rs`,
`transcript.rs`

- Split `render_send_stop_button` into `render_stop_button` and
  `render_send_button`; the composer row renders Stop only while streaming and
  Send always, so both are present mid-stream (REQ-222-001).
- The Send handler branches on `StreamingState::Streaming` to emit
  `SteerStreaming` instead of returning early (REQ-222-002).
- `handle_enter` drops its streaming early-return and takes the same branch.
- `ChatViewState` gains `queued_steering: Vec<QueuedSteering { id, text }>`;
  `SteeringDelivered` and `SteeringRejected` remove by id (REQ-222-003).
- The transcript renders queued entries after the live stream block, styled
  distinctly from a sent user message.

Tests (`mod_tests.rs`, `render_bars_tests.rs`): button presence per state,
emitted event per state and per input, queued-entry lifecycle add/deliver/reject.

### P05 — Verification

`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test --lib --tests`, structural checks, lizard complexity.
