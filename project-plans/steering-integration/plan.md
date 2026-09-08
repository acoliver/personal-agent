# Plan: Mid-turn steering delivery at the tool-call boundary

Plan ID: PLAN-20260905-STEERINT
Generated: 2026-09-05
Base: origin/main 92b5bc3 (branch `steering-integration`)
Total Phases: 5
Requirements: REQ-SI-001 .. REQ-SI-008

## Problem

Issue #222 shipped steering with LOCAL TURN CHAINING: a steer typed during a
running turn waits in `steering_queues` and is delivered by persisting it and
running a whole new `AgentStream` turn after the current one finishes
(`src/services/chat_impl/streaming/steering_delivery.rs:121-175`). The user
waits for the entire turn to end before the model hears the correction.

Upstream `acoliver/serdesAI` now supports true mid-turn delivery. Commit
`c0e7327a880f8d3013bb47c56c2fb231e3e93264` (branch `feature/steering-input`,
read at `/Volumes/XS1000/acoliver/projects/serdesAI/branch-steering`) adds to
crate `serdes-ai-agent`:

- `SteeringQueue` (`serdes-ai-agent/src/steering.rs`): `Clone + Default`,
  `new()`, `steer(String) -> bool` (false only when every receiver is dropped
  and none can be claimed again), `pending_len() -> usize`. Backed by a tokio
  unbounded mpsc. A run claims the single receiver via
  `take_receiver()`; `SteeringReceiver::Drop` PARKS IT BACK on the queue, so
  undelivered texts survive a run and a later run from the SAME queue delivers
  them at its first tool boundary. Drain is FIFO by channel order.
- `RunOptions::steering(queue)` (`serdes-ai-agent/src/run.rs`), mirroring
  `message_history`.
- `AgentStreamEvent::SteeringDelivered { step: u32, text: String }`
  (`serdes-ai-agent/src/stream.rs:100-112`), emitted once per delivered text by
  `deliver_queued_steering` (`stream.rs:214-233`), AFTER that step's
  `ToolExecuted` events and BEFORE the next `RequestStart`. Steps are 1-based.
  The drained text is appended to the run's history as its own `ModelRequest`
  carrying a user prompt part.
- Drain happens inside the spawned stream task at the tool-call boundary in
  BOTH `AgentStream::new` (`stream.rs:913`) and `AgentStream::new_with_cancel`
  (`stream.rs:1530`). Nothing drains before the first model request. Nothing
  drains at a text-only end of turn; leftovers stay queued.

This plan moves PA delivery from between-turns to that boundary.

## PA call sites this touches (read, verified)

- `src/llm/client_agent.rs:475-534` `handle_agent_stream_event` ends with
  `other => tracing::debug!(...)` at line 532. `SteeringDelivered` will COMPILE
  against the new rev and be SILENTLY SWALLOWED unless an explicit arm is
  added. The compiler will not warn. Phase P02 writes a regression test that
  fails if the arm is missing.
- `src/llm/client_agent.rs:558-562` builds `RunOptions`, then line 565 calls
  `AgentStream::new(agent, UserContent::text(prompt), context, options)`.
- `src/llm/stream.rs:124-128` is a second, legacy path
  (`send_message_stream`, `run_stream_with_options`). It has no production
  caller (only the re-export at `src/llm/mod.rs:21`), no service context, no
  event-bus access, and no queue to correlate against. It gets NO steering.
  Justification: attaching a queue requires a delivery sink that can persist
  and announce; this path has neither. With no queue attached the fork drains
  nothing, so its behavior is byte-identical after the rev bump. Its
  `_ => ChatStreamEvent::text(String::new())` catch-all (`stream.rs:139-142`)
  is unreachable for `SteeringDelivered` for the same reason.
- `src/services/chat_impl.rs:66-76` `ActiveStream` (per-conversation slot:
  stream_id, task handle, cancel token, lifecycle state).
- `src/services/chat_impl.rs:245-266` `begin_stream` creates the slot
  atomically; `src/services/chat_impl.rs:320-372` `cancel_active_stream` fires
  the token, ABORTS the task handle, drains the PA steering queue, and emits
  `SteeringDiscarded` for each entry.
- `src/services/chat_impl/steering.rs` PA queue: cap at
  `MAX_QUEUED_STEERING_MESSAGES = 5` (line 26), `queue_steering` (153-197),
  `confirm_or_withdraw_steering` (237-249), `push_steering` (263-274),
  `remove_steering` (293-304), `drain_steering` (311-313), discard emitters
  (104-124). Lock discipline: `active_streams` and `steering_queues` never
  held simultaneously, nothing emitted while either is held (37-56).
- `src/services/chat_impl/streaming.rs`: `stream_agent_response` (67-100)
  calls `client.run_agent_stream(...)` with a sync `FnMut(StreamEvent)`
  callback; `StreamTranscript` (31-43); `run_stream_task` (317-407) owns the
  chain; `handle_llm_stream_event` (409-460) is sync; `clear_streaming_state`
  (753-776) drains + discards PA-queue leftovers whenever the send's slot is
  released.
- `src/services/chat_impl.rs:486-568` `build_llm_messages` rebuilds turn input
  from persisted rows in insertion order. Insertion order IS reload order.
- `src/events/types.rs:470/480/496` `SteeringQueued` / `SteeringDelivered` /
  `SteeringDiscarded`; the view removes the waiting bubble on
  `SteeringDelivered` (`src/ui_gpui/views/chat_view/command.rs:191-204`).

## THE CENTRAL DESIGN CONFLICT: when does a delivered steer reach the database

Today `deliver_steering` (`steering_delivery.rs:205-238`) persists with
`add_message` FIRST and emits `SteeringDelivered` only after the write
succeeds; on failure it stops the chain and announces `SteeringDiscarded` for
that message and everything behind it (`steering_delivery.rs:211-227`). The
model is never shown text the database rejected. That ordering is available
only because PA controls the sequence between turns.

Mid-turn, the ordering inverts. The fork appends the text to the model request
inside its spawned task (`stream.rs:214-233`) and PA learns about it
afterwards, via `SteeringDelivered`. By then the model has seen it. "Not
seeded, not announced" is structurally unachievable at this boundary.

### Option (a): persist at steer-acceptance time, before `queue.steer(text)`

The row is durable before the text can reach the model, and the model can
never see unpersisted text, restoring today's guarantee one level up.

Rejected, for three reasons.

1. It persists rows the model may never see. A steer accepted during a
   text-only tail, or cancelled before any boundary, stays in the database
   with its UI bubble withdrawn as discarded. #222 established that a
   discarded steer leaves no trace; this option makes every discard a
   deletion, and deleting a user row makes the transcript lie about what was
   typed.
2. Delivery then needs "already persisted" bookkeeping so `add_message` is not
   called twice, and the row sits at acceptance-order position even when the
   model saw it later (or never).
3. Stop semantics rot: `cancel_active_stream` discards queued steers, but the
   rows would already exist, so cancel would need row deletion to stay
   consistent with the announced discard.

### Option (b): persist on receipt of `SteeringDelivered`

PA writes the row exactly when it learns the text reached the model. The
database reflects reality: the model saw it if and only if it is persisted,
modulo the failure window. Correlation and ordering stay with the delivery
event. Cost: a persist failure happens after the model saw the text.

### Option (c): option (b), and the persist failure fails the turn (RECOMMENDED)

(b) leaves the failure response undefined. The house style is fail fast, so
define the single behavior: when `add_message` fails for a delivered steer, PA
fails the stream immediately. Concretely: the sink returns `Err`, the event
loop maps it to `StreamEvent::Error` plus a returned `LlmError` (same shape as
the existing mid-stream failure path, `client_agent.rs:573-585` and
`streaming.rs:111-130`), `transcript.error` is set, and finalization runs the
interrupted path (`finalize_by_outcome`, `streaming.rs:263-277`, persisting
partial output marked interrupted). The failed steer's own bubble gets
`SteeringDiscarded` (its truthful terminal: not recorded), entries behind it
stay queued and are discarded by the send's teardown (`clear_streaming_state`,
`streaming.rs:753-776`). No retries, no swallowing, no guard layers.

**Failure mode of the recommendation, stated plainly.** If the database fails
at the moment of delivery, the model has already seen text that is not in the
store, and that divergence is permanent: the aborted turn's continuation dies
with it, the next send re-seeds from the database without the lost text, and
nothing can recover it. PA announces the divergence twice (stream error, steer
discarded) and never pretends it did not happen. This is strictly narrower
than option (a)'s failure mode, where the database silently accumulates
instructions the model never received and cancellation must delete user rows
to hide them.

### Reload ordering (the second half of the conflict)

#222 kept this invariant: persisted history equals the history a chained turn
is seeded with, so a reload reconstructs what the model saw.

Within one mid-turn-steered run the fork's history is `[user prompt, assistant
partial + tool calls, tool returns, steer text, continuation...]`. PA's
persistence model is already coarser than that: ONE assistant row per turn
holding the accumulated response text plus all tool calls and results
(`persist_assistant_response`, `streaming.rs:660`; reloaded by
`build_llm_messages:524-557`). The per-step interleaving of partial text and
tool calls inside a turn is already collapsed by that design and is not
reconstructed on reload today.

The order PA can and must preserve is the order of PERSISTED ROWS, and
insertion order gives the right one with zero schema or `build_llm_messages`
changes:

1. user row for the turn prompt: written at send start
   (`prepare_message_context`, `chat_impl.rs:389-391`);
2. steer rows: written by the sink at `SteeringDelivered`, mid-turn;
3. assistant row for the turn: written at end of turn (chain persist or
   finalization).

A reloaded conversation reads `[turn prompt, steers that shaped the answer,
the answer]`, which is the order things actually happened at the granularity
PA persists. `build_llm_messages` (`chat_impl.rs:486-568`) iterates rows in
insertion order and maps user rows to `LlmMessage::user`, so reload and seed
agree with NO change to that function.

To keep the seeded history equal to the reload on chained turns, the chain
must re-insert each turn's delivered steer texts into its in-memory `messages`
BEFORE the assistant message push (mirroring the row order). The sink records
delivered texts in order on the transcript; `run_steered_turns` splices them
in before `assistant_message(&transcript)` (the push currently at
`steering_delivery.rs:167-169`). REQ-SI-004 pins this.

## Two queues, one relationship

- PA's service-level queue (`SteeringQueues`, `VecDeque<QueuedSteering>` with
  `id` + `text`) remains the STATE MACHINE: acceptance cap (5), UI bubbles via
  `SteeringQueued`, terminal events, withdrawal, cancel/teardown discard. It
  is the source of truth for "accepted but not yet resolved".
- The fork's `SteeringQueue` is the TRANSPORT: it and only it decides when a
  text reaches the model. PA never reads it back, never drains it, and never
  seeds from it.

Write path: acceptance pushes to the PA queue first, announces
`SteeringQueued`, passes `confirm_or_withdraw_steering`, and only then calls
`steer(text)` on the fork queue. The fork `steer()` call is the COMMIT POINT:
once it returns true, PA no longer withdraws the entry (it can only be
resolved by delivery, teardown discard, or cancel discard). Calling `steer()`
before the re-check would let a withdrawn text linger in the transport and
deliver as a ghost; the ordering above closes that.

Read path: on `AgentStreamEvent::SteeringDelivered { text, .. }` the sink pops
the HEAD of the conversation's PA queue and treats it as the delivered entry.

**Correlation is by FIFO position, never by text.** The fork drains FIFO from
a channel; PA pushes in the same order it pushes to its own deque; exactly one
`AgentStream` per send holds the receiver at any moment (the chain is
sequential), so deliveries arrive in exactly the PA queue's order. Two
identical texts occupy positions k and k+1 on both sides and resolve
correctly. A duplicate `steer_id` match or a text match would break here; a
position match cannot.

Residual races, each with exactly one defined behavior:

- Cancel/teardown drained the PA queue between the fork's drain and the sink's
  pop (empty deque on delivery): the model DID see the text, so the sink
  still persists the row, records it for chain seeding, and emits no UI
  terminal (the discard already fired; the at-least-one-terminal guarantee of
  #222 is intact). Bubble count: zero, correctly, because cancel announced it.
- `steer()` returns false at commit time (run gone between the re-check and
  the commit): same refusal shape as `confirm_or_withdraw_steering`: remove
  the entry, emit `SteeringDiscarded`, return the same
  `no_active_turn()` validation error.
- A steer accepted while a delivery is being persisted: goes to the TAIL of
  both queues; delivery order is unaffected.

## Queue lifetime

ONE `SteeringQueue` per SEND, not per turn. It is created in `begin_stream`,
stored on the conversation's `ActiveStream` entry, and dropped with that entry
in `clear_streaming_state` / `cancel_active_stream`. Every chained turn of the
send passes the same clone via `RunOptions::steering`.

The park-back-on-drop rule makes this the only correct lifetime: a fresh queue
per turn would strand leftovers forever (turn N+1 would never see turn N's
undelivered texts), while reusing one queue delivers them at turn N+1's first
tool boundary, which is exactly the leftover path the chaining exists to
provide.

**Double-delivery is resolved by deletion, not by guards.** Today the chain
drains the PA queue at the turn boundary and seeds the texts into the next
turn's history (`drain_steering_queue` at `steering_delivery.rs:143`, push at
`steering_delivery.rs:235`). If that survived alongside the transport, a
leftover would be BOTH seeded into turn N+1's history by PA AND delivered
inside turn N+1 by the fork. The PA drain-and-seed path is deleted. The fork
queue is the only thing that moves text into a model request.

Cross-send leakage is impossible: the next user send creates a new
`ActiveStream` and a new queue; the old queue dies with the old slot, and any
texts still parked in it were already announced as discarded by the teardown
(`clear_streaming_state` drains the PA queue and emits discards; the parked
copies in the dropped transport vanish with it, model never having seen them).

## What remains of end-of-turn chaining

- STAYS: the loop in `run_steered_turns`; the clean-turn precondition
  `reaches_delivery_boundary` (`steering_delivery.rs:186-188`: completed, no
  error, not cancelled); intermediate-turn persistence
  (`persist_assistant_response` at `steering_delivery.rs:159-166`) so a chained
  turn's seed matches the reload; finalization once, on the last turn; the
  turn cap; the discard announcements on every ending that leaves entries
  queued.
- CHANGES: the loop's leftover check becomes a PEEK
  (`has_queued_steering(queues, conversation_id)`), not
  `drain_steering_queue`, because the transport still owns the texts and must
  deliver them itself. The cap branch removes the entries and announces them
  (they will never be delivered within this send; the dropped transport takes
  its parked copies away at teardown).
- DELETED: `deliver_steering` (`steering_delivery.rs:205-238`) and the
  `LlmMessage::user` push into the next turn's seed; the
  `ChainOutcome::OutputPersisted` variant (`steering_delivery.rs:108-113`)
  and the `deliver_steering -> false` exit that produced it
  (`steering_delivery.rs:171-173`). With delivery inside the turn, there is no
  step between a turn's persist and the next turn's start that can fail, so
  `run_steered_turns_and_finalize` always ends in `finalize_by_outcome`.
  `ChainOutcome` collapses to a boolean or disappears with the match.
- `MAX_STEERING_TURNS = 10` STAYS, with unchanged semantics: it counts
  `run_turn` invocations (initial turn included, `steering_delivery.rs:131,
  137, 148`) and is still the only bound on the refill loop, since the queue
  cap bounds depth but a user can refill during every follow-up turn. A
  text-only leftover turn that triggers another text-only leftover turn still
  consumes the cap and then discards, exactly like today.

## Cancellation

PA uses `AgentStream::new` (`client_agent.rs:565`), not `new_with_cancel`.
`create_stream_agent` receives the token as `_cancel` and ignores it
(`chat_impl.rs:761`). Stop works at the PA level: `cancel_active_stream`
(`chat_impl.rs:320-331`) fires the `CancellationToken` and ABORTS the spawned
stream task; the token's remaining uses are the chain preconditions
(`steering_delivery.rs:186-188`).

No constructor change. Both fork constructors drain at the boundary, so
staying on `new` keeps delivery behavior identical while preserving the abort
semantics Stop depends on. Defined cancel interactions:

- Steer still parked in the transport at abort: announced discarded by
  `cancel_active_stream`, row never written, transport dropped with the send.
  Same as today's discarded-but-never-delivered entries.
- Fork drained a text and the sink persisted it, THEN the user hits Stop: the
  row stays. The model saw it mid-turn and it is in the store; only the
  turn's assistant output is discarded, which is today's cancelled-turn
  behavior for output.
- Abort lands between the fork's drain and the sink's persist: the text was
  inside the fork task's in-memory history for a turn whose output is being
  discarded anyway. PA announces the discard (the cancel path runs
  unconditionally) and writes no row. No guard layer is added; a stopped turn
  is the terminal state of everything it was holding.

## Requirements

### REQ-SI-001: Steering is delivered at the tool-call boundary inside the running turn

**Behavior**
- GIVEN a running turn that executes at least one tool
- WHEN the user steers while a tool is in flight
- THEN the text reaches the model in that same turn, appended after that
  step's tool returns and before the next model request
- AND PA observes it via `AgentStreamEvent::SteeringDelivered`
- AND no new user-initiated send and no chained turn is needed for it.

### REQ-SI-002: One `SteeringQueue` per send, shared by all its chained turns

**Behavior**
- GIVEN a send with queued steering that a turn ended without delivering
- WHEN the chain runs the next turn
- THEN the same queue is attached and the leftover is delivered at that
  turn's first tool boundary
- AND the leftover is never also seeded into the turn's history by PA.

### REQ-SI-003: Delivery persists, and persist failure fails the turn

**Behavior**
- GIVEN a `SteeringDelivered` event carrying a text
- WHEN the sink receives it
- THEN PA persists the text as a user message and then emits
  `ChatEvent::SteeringDelivered { steer_id }` for the correlated entry
- AND when `add_message` fails, the stream fails with a reported error, the
  entry is announced `SteeringDiscarded`, and finalization takes the
  interrupted path.

### REQ-SI-004: Reload order equals seed order equals row order

**Behavior**
- GIVEN a turn that received steering mid-turn and then produced output
- WHEN the conversation is reloaded and its next send is built
- THEN the history reads `[turn prompt, steer texts, turn assistant output]`
- AND the persisted rows are in exactly that insertion order
- AND `build_llm_messages` is unchanged.

### REQ-SI-005: Correlation is by FIFO position, not by text

**Behavior**
- GIVEN two queued steers with identical text
- WHEN both are delivered
- THEN each `SteeringDelivered` resolves the PA-queue head, so each `steer_id`
  gets exactly one delivered terminal event, in queue order.

### REQ-SI-006: Text-only ends of turn still chain for leftovers, bounded

**Behavior**
- GIVEN a turn that finished cleanly without crossing a tool boundary while
  steers are still queued
- WHEN the chain decides
- THEN it runs another turn on the same queue (the fork delivers there)
- AND when the chain hits `MAX_STEERING_TURNS` it announces every remaining
  entry `SteeringDiscarded` and stops.

### REQ-SI-007: The #222 invariants survive

**Behavior**
- GIVEN any steering traffic during a turn
- THEN steering never cancels or disturbs the generation, never touches the
  cancellation token, and never resolves an approval
- AND `MAX_QUEUED_STEERING_MESSAGES = 5` and `MAX_STEERING_TURNS = 10` still
  bind
- AND every `SteeringQueued` is followed by at least one terminal event
- AND queued transcript entries remain chrome
- AND `active_streams` and `steering_queues` are never held simultaneously
  and nothing is emitted while either is held.

### REQ-SI-008: Stop semantics are unchanged

**Behavior**
- GIVEN a running turn with or without steering traffic
- WHEN the user presses Stop
- THEN the task is aborted and the token fires exactly as today
- AND steers already persisted before the abort keep their rows
- AND steers never delivered are announced `SteeringDiscarded`.

## Phases

### P01: Move the dependency pins to the steering rev

Plan ID `PLAN-20260905-STEERINT.P01`

Files: `Cargo.toml`, `Cargo.lock`

- Change `rev` on all EIGHT `serdes-ai-*` entries (`Cargo.toml:46-55`) from
  `e675674ef802a0ac90c03cd8bcdbd5114fabb44f` to
  `c0e7327a880f8d3013bb47c56c2fb231e3e93264` in ONE commit. Cargo resolves one
  source per package name and `serdes-ai-responses` exists only on the fork,
  so partial moves do not resolve.
- Verified upstream delta (`git log/diff e675674..c0e7327` in the fork): one
  commit touching only `serdes-ai-agent` (steering.rs, stream.rs, run.rs,
  lib.rs) plus workspace Cargo files. No other crate's behavior changes.
- No PA source changes in this phase. Nothing references `SteeringQueue` yet;
  the fork drains nothing when no queue is attached.

Tests:
- `cargo tree -i serdes-ai-agent` shows a single source at `c0e7327`
- full existing suites pass unchanged (empty-queue behavior is byte-identical)

### P02: Steering channel in the LLM layer, with the explicit event arm (test-first)

Plan ID `PLAN-20260905-STEERINT.P02`

Prerequisites: P01 (the event variant and `RunOptions::steering` must exist).

Files: new `src/llm/steering.rs`; `src/llm/mod.rs`; `src/llm/client_agent.rs`

- `src/llm/steering.rs`: `SteeringDeliverySink` trait (`#[async_trait]`, one
  method `delivered(&mut self, text: &str) -> StdResult<(), LlmError>`) and
  `SteeringInput<'a> { queue: serdes_ai_agent::SteeringQueue, sink: &'a mut
  dyn SteeringDeliverySink }`. The trait object keeps the storage layer out of
  the LLM crate; the repo already uses `async-trait = "0.1"`
  (`Cargo.toml:35`).
- `AgentClientExt::run_agent_stream` and `do_run_agent_stream`
  (`client_agent.rs:361-392, 537-588`) gain one parameter
  `steering: Option<SteeringInput<'_>>`. When `Some`, chain
  `.steering(channel.queue.clone())` onto the `RunOptions` built at
  `client_agent.rs:558-562`.
- `handle_agent_stream_event` (`client_agent.rs:475-534`) becomes `async`,
  takes `Option<&mut SteeringInput<'_>>`, and adds an EXPLICIT arm for
  `AgentStreamEvent::SteeringDelivered { text, .. }` placed before the
  catch-all at line 532. With a channel it awaits `sink.delivered(text)`;
  on sink `Err` it emits `StreamEvent::Error` and the caller returns `Err`
  (the existing mid-stream failure shape at `client_agent.rs:579-583`). With
  `None` (impossible by construction: no queue, no drain, no event) it emits
  `StreamEvent::Error` naming the bug rather than trusting the catch-all.
- Update the production caller (`stream_agent_response`,
  `streaming.rs:78-88`) to pass `None` for now; real wiring lands in P04.
  The legacy `src/llm/stream.rs` path is deliberately untouched (see call
  sites section).

Tests (`src/llm/client_agent.rs` `#[cfg(test)]` module or a sibling test
file, against a synthetic stream of `AgentStreamEvent`s):
- REGRESSION GUARD for the silent swallow: GIVEN a recording sink and a
  synthetic `SteeringDelivered { step: 1, text }` WHEN
  `handle_agent_stream_event` processes it THEN the sink recorded exactly
  that text AND no `StreamEvent::Error` was emitted. This test FAILS if the
  explicit arm is missing, because the catch-all at line 532 never calls the
  sink.
- GIVEN a sink that returns `Err` WHEN `SteeringDelivered` is processed THEN
  `StreamEvent::Error` is emitted and the function reports `Err`
- GIVEN `steering: None` and a `SteeringDelivered` event WHEN processed THEN
  `StreamEvent::Error` is emitted (loud, not debug-logged)
- GIVEN no steering events WHEN processed THEN output events are identical to
  today (regression).

### P03: Acceptance commits through the fork queue (test-first)

Plan ID `PLAN-20260905-STEERINT.P03`

Prerequisites: P02.

Files: `src/services/chat_impl.rs`, `src/services/chat_impl/steering.rs`

- `ActiveStream` (`chat_impl.rs:66-76`) gains `steering:
  serdes_ai_agent::SteeringQueue`, created in `begin_stream`
  (`chat_impl.rs:245-266`) so its lifetime is the send's slot.
- `queue_steering` (`steering.rs:153-197`) extends its tail: after
  `confirm_or_withdraw_steering` confirms, lock `active_streams`, clone the
  conversation's queue handle, DROP the lock, call `steer(trimmed_text)`. On
  `false`: remove the entry, emit `SteeringDiscarded`, return the same
  `no_active_turn()` validation error. Lock discipline preserved: the two
  locks are still never held together and nothing is emitted while either is
  held. `steer()` takes the trimmed text, matching what the PA queue stores,
  so transport and state machine hold identical bytes.
- `spawn_stream_task` / `run_stream_task` signatures gain the queue clone so
  turns can attach it (used fully in P04).

Tests (`src/services/chat_impl/tests/steering/`, same harness as
`withdrawal.rs`):
- GIVEN a running stream WHEN `steer` succeeds THEN the fork queue reports
  `pending_len() == 1` AND the PA queue holds the entry
- GIVEN a stream torn down between the confirm re-check and the commit WHEN
  `steer()` returns false THEN the entry is removed, `SteeringDiscarded` is
  emitted, and `steer` returns `Err`
- GIVEN a withdrawn steer (confirm re-check fails) WHEN the transport is
  inspected THEN the fork queue is empty (withdrawal never reaches it)
- GIVEN `MAX_QUEUED_STEERING_MESSAGES` entries WHEN a sixth steer arrives
  THEN it is refused and the fork queue received nothing new
- GIVEN steering acceptance at any point THEN the cancel token is untouched
  and no approval is resolved (existing REQ-222-006/008 tests keep passing).

### P04: Mid-turn delivery: persistence, correlation, chaining rewrite (test-first)

Plan ID `PLAN-20260905-STEERINT.P04`

Prerequisites: P02, P03.

Files: `src/services/chat_impl/streaming.rs`;
`src/services/chat_impl/streaming/steering_delivery.rs` (or a new
`steering_delivery/sink.rs` alongside `discard.rs`)

- `MidTurnSteering` sink (services layer, implements `SteeringDeliverySink`):
  owns `Arc<dyn ConversationService>`, `conversation_id`, the
  `SteeringQueues` handle, and a delivered-text buffer. `delivered(text)`:
  pop the PA-queue head under the `steering_queues` lock and release it;
  `add_message(conversation_id, Message::user(text))`; on success record the
  text in the buffer and emit `ChatEvent::SteeringDelivered {
  conversation_id, steer_id }`; on failure emit `ChatEvent::SteeringDiscarded`
  for that id and return `Err`. Empty deque (cancel raced the delivery):
  persist the row and record the text, emit nothing (the discard already
  fired). No text matching anywhere.
- `stream_agent_response` (`streaming.rs:67-100`) takes
  `Option<(&serdes_ai_agent::SteeringQueue, &mut MidTurnSteering)>`, builds
  the `SteeringInput`, and after `run_agent_stream` returns copies the sink
  buffer into a new `StreamTranscript::delivered_steering: Vec<String>`
  (`streaming.rs:31-43`).
- `run_stream_task` (`streaming.rs:317-407`) constructs the sink from its
  existing `conversation_service` + `steering_queues` and passes queue + sink
  into the turn runner closure.
- `run_steered_turns` (`steering_delivery.rs:121-175`) rewrite:
  - DELETE `deliver_steering` (205-238) and the `ChainOutcome` machinery
    (98-113, 171-173); `run_steered_turns_and_finalize` always ends in
    `finalize_by_outcome`.
  - The leftover check becomes a peek: new `has_queued_steering(queues, id)`
    in `steering.rs` (lock, `!queue.is_empty()`, drop). Nothing is removed on
    the happy path; the transport owns the texts.
  - Cap branch (`steering_delivery.rs:148-157`): drain the PA queue, announce
    every entry `SteeringDiscarded`, return.
  - Chaining seed: after the intermediate `persist_assistant_response`,
    splice `transcript.delivered_steering` texts as `LlmMessage::user` BEFORE
    the existing `assistant_message(&transcript)` push, reproducing row order
    `[prompt, steers, assistant]` in the next turn's seed (REQ-SI-004).
- `MAX_STEERING_TURNS` stays at 10 with unchanged counting.

Tests (`src/services/chat_impl/tests/steering_delivery/`, harness as
`discard.rs`; mock `ConversationService` recording `add_message` order):
- GIVEN a steer submitted while a tool runs WHEN the tool completes THEN the
  model's next request in that turn contains the steer text AND a persisted
  user row exists before the turn's assistant row (REQ-SI-001, REQ-SI-003)
- GIVEN two steers with identical text WHEN both are delivered THEN each gets
  its own `SteeringDelivered` with distinct ids in queue order (REQ-SI-005)
- GIVEN a delivery WHEN `add_message` fails THEN the transcript records an
  error, the entry is announced discarded, entries behind it are discarded at
  teardown, and no follow-up turn runs (REQ-SI-003)
- GIVEN a delivered steer WHEN the next turn is chained THEN its seed is
  `[prompt, steer, assistant]`, matching a fresh `build_llm_messages` reload
  of the same rows (REQ-SI-004)
- GIVEN a text-only turn that left a steer queued WHEN the chain decides
  THEN another turn runs on the same queue and the leftover delivers at its
  first tool boundary (REQ-SI-006, REQ-SI-002)
- GIVEN `MAX_STEERING_TURNS` exhausted with entries queued WHEN the chain
  stops THEN every remaining entry is announced discarded (REQ-SI-006)
- GIVEN a cancel that lands after a steer was persisted THEN the row remains
  and no discard is announced for it (REQ-SI-008)
- GIVEN a cancel that lands before delivery THEN the entry is announced
  discarded and no row is written (REQ-SI-008)
- GIVEN an empty queue WHEN a turn runs THEN behavior is identical to today
  (regression)
- GIVEN a pending tool approval WHEN a steer arrives THEN it is delivered
  only after the approval resolves and the boundary passes (REQ-222-008
  carried over; the fork drains after `ToolExecuted`).

### P05: Verification sweep

Plan ID `PLAN-20260905-STEERINT.P05`

- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --lib --tests`
- structural/lizard checks used by the repo
- `@plan` / `@requirement` markers on every new item
- Manual smoke: run the app, steer during a tool-using turn, confirm the
  bubble resolves mid-turn while the assistant is still streaming, confirm
  the persisted order after reload, press Stop with a queued steer and
  confirm the discard.

## Test strategy summary

Every phase above is test-first; the behavioral GIVEN/WHEN/THEN list lives in
the phases. The critical items:

- The P02 regression guard that fails if the explicit `SteeringDelivered`
  match arm is missing (the catch-all at `client_agent.rs:532` would
  otherwise swallow the event with a debug log and zero compiler
  diagnostics).
- The P04 order test that pins `[prompt, steer, assistant]` in rows, reload,
  and seed simultaneously.
- The P04 FIFO test with duplicate texts, which fails under any
  text-matching correlation.
- The P03 empty-transport test that fails if withdrawal order regresses.

## Risks and rollback

- Blast radius of the rev bump: all eight pinned crates move, but the
  upstream delta is one commit confined to `serdes-ai-agent`; the other
  crates are rebuilt from identical sources. Compile-time risk is contained
  to `serdes-ai-agent`'s new exports (additive).
- Behavior risk concentrates in P04 (delivery, correlation, chaining). The
  phase is one commit; its tests pin the ordering and correlation invariants.
- The two-queue relationship has one commit point (fork `steer()` at the tail
  of acceptance) and one read rule (pop head on delivery). Both are pinned by
  tests; there are no other couplings.
- Rollback: revert the branch's commits (or the whole branch) and pin
  `Cargo.toml:46-55` back to `e675674`. Schema is untouched (`add_message`
  user rows written by the new code are ordinary user messages the old code
  reads happily), so a rollback never orphans data. Steering rows persisted
  mid-turn remain valid transcript entries after rollback.

## Out of scope

- The legacy `send_message_stream` path (`src/llm/stream.rs`): no steering,
  no changes; its removal is a separate cleanup decision.
- Delivering between two tool calls of one step, or before the first model
  request: the fork does not drain there, by design.
- Any change to `Stop` partial-output behavior (issue #218) or the Responses
  transport (#217).
- Compression interactions beyond what chaining already does; steering texts
  are ordinary user rows to the compression pipeline.

## Open questions for a human

1. UI timing: a delivered steer's user row now appears in the transcript while
   the assistant is still streaming (today it appeared at a turn boundary).
   The view already removes the waiting bubble on `SteeringDelivered`
   (`command.rs:191-204`) and renders persisted rows from the transcript, but
   the visual behavior of a user message appearing above a streaming answer
   mid-turn deserves a look before P04 ships. No code change expected.
2. Whether the unused legacy `send_message_stream` path should be deleted
   outright now that it diverges further from the agent path. Not needed for
   this feature.
