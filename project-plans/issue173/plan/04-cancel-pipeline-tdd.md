# Phase 04: Cancel Pipeline TDD

Plan ID: `PLAN-20260416-ISSUE173.P04`

## Prerequisites

- P03 verified PASS.

## Requirements implemented (tests only)

- REQ-173-002.3 `UserEvent::StopStreaming { conversation_id }` flows to
  `ChatService::cancel(conversation_id)`.

## Tasks

### `src/presentation/chat_presenter_tests.rs`

Add new test module section with at least two tests, each marked
`@plan PLAN-20260416-ISSUE173.P04` and `@requirement REQ-173-002.3`:

1. `handle_stop_streaming_forwards_conversation_id`:
   - Build a mock `ChatService` that records `cancel(conversation_id)` calls.
   - Dispatch `UserEvent::StopStreaming { conversation_id: A }` through the
     presenter.
   - Assert the mock received exactly one `cancel(A)` call.

2. `handle_stop_streaming_does_not_cancel_other_conversations`:
   - Using the same mock, dispatch `StopStreaming { conversation_id: A }`.
   - Assert `cancel(B)` was NEVER called.

The existing test harness in `src/presentation/chat_presenter_tests.rs` already
uses mock services; mirror its style. If a `MockChatService` is not yet
available for capturing `cancel` calls, add one local to the test module, with
internal `Arc<Mutex<Vec<Uuid>>>` capturing cancel conversation ids.

### `src/events/types.rs` — do NOT modify yet

The tests must reference `UserEvent::StopStreaming { conversation_id }`, which
will not compile until P05. That is expected.

### Render-site test (optional but preferred if feasible)

If `src/ui_gpui/views/chat_view/render.rs` has testable emission helpers, add
a unit test that invokes the stop-button handler with a view-state that has
`selected_conversation_id = Some(A)` and asserts it emits
`UserEvent::StopStreaming { conversation_id: A }`. Otherwise, document that
this path is covered by the service-layer integration test in P12.

## Verification

- `cargo build --all-targets 2>&1 | tail -20` — expected to fail with the
  specific error "expected unit variant, found struct variant" or
  "no field `conversation_id` on ..." — that is the intended failing signal.
- `grep -c "@plan PLAN-20260416-ISSUE173.P04" src/presentation/`

Write `project-plans/issue173/plan/.completed/P04.md` with evidence.
