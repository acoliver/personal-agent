# Phase 02: Service Layer TDD (failing tests)

Plan ID: `PLAN-20260416-ISSUE173.P02`

## Prerequisites

- P01 completed with Verdict: PASS.
- `cargo test` green at baseline.

## Requirements implemented (TESTS ONLY — no production code changes)

- REQ-173-001.1 concurrent starts across different conversations
- REQ-173-001.2 within-conversation second start rejected
- REQ-173-001.3 self-cleanup on completion
- Parts of REQ-173-002.1 and REQ-173-002.2 (cancel scoping) that live inside
  the service

## Implementation tasks

### Create `src/services/chat_impl/tests/concurrent_streams.rs` (new test module)

Add to `src/services/chat_impl/tests.rs`:
```rust
mod concurrent_streams;
```

The new test module MUST contain integration-style unit tests that exercise the
public API of `ChatServiceImpl` (using `ChatServiceImpl::new_for_tests` or an
equivalent constructor). Use the existing mock `ConversationService` and
`ProfileService` that the existing tests use (look in
`src/services/chat_impl/tests.rs` and mirror the setup).

MANDATORY test cases (each annotated `@plan PLAN-20260416-ISSUE173.P02` and
the relevant `@requirement`):

1. `begin_stream_allows_two_different_conversations` (REQ-173-001.1):
   - Construct service.
   - Call `begin_stream(A).await.unwrap()`.
   - Call `begin_stream(B).await.unwrap()`.
   - Assert `is_streaming_for(A)` and `is_streaming_for(B)` both true.
   - Assert `is_streaming()` true.

2. `begin_stream_rejects_same_conversation_twice` (REQ-173-001.2):
   - `begin_stream(A).await.unwrap()`.
   - `let err = begin_stream(A).await.unwrap_err();`
   - Assert `err.to_string().contains("Stream already in progress for this conversation")`.

3. `cancel_scopes_to_conversation` (REQ-173-002.1):
   - `begin_stream(A)` and `begin_stream(B)`.
   - `service.cancel(A)`.
   - Assert `is_streaming_for(A) == false` and `is_streaming_for(B) == true`.

4. `cancel_emits_event_only_for_target` (REQ-173-002.2):
   - Subscribe to the global `AppEvent` bus (see how existing tests in
     `src/events/` or `src/services/chat_impl/tests.rs` do this — if a
     suitable utility exists, use it; otherwise use `crate::events::subscribe()`
     pattern already used elsewhere).
   - Start streams in A and B.
   - `service.cancel(A)`.
   - Drain events and assert exactly one `ChatEvent::StreamCancelled { conversation_id: A, .. }`
     is observed and zero with `conversation_id: B`.

5. `cancel_unknown_conversation_is_noop` (REQ-173-002.1 edge):
   - `service.cancel(Uuid::new_v4())` without prior begin.
   - Must not panic, must not mutate other streams.

### Exposing helpers

`begin_stream`, `is_streaming_for`, and `cancel_active_stream` may be private
today. For these tests we can:
- Add a `pub(crate)` or `pub(in crate::services)` test-only shim in
  `chat_impl.rs` guarded by `#[cfg(test)]`, OR
- Test through `ChatService::cancel(conversation_id)` + `is_streaming()` +
  `is_streaming_for(conversation_id)` on the trait.

Prefer the second approach when practical so the tests exercise the public
trait surface. For items not reachable from the trait (direct `begin_stream`
calls), add a `#[cfg(test)] pub(crate) async fn begin_stream_for_test(...)`
that simply forwards.

### Required markers

Every new test function:
```rust
/// @plan PLAN-20260416-ISSUE173.P02
/// @requirement REQ-173-001.x (or 002.x)
#[tokio::test]
async fn begin_stream_allows_two_different_conversations() { ... }
```

## Verification

- `cargo build --all-targets` — must compile (the trait method
  `is_streaming_for` and the new signature for `cancel` do NOT yet exist).
  Therefore **this phase may legitimately fail to compile**. That is acceptable
  for a tests-first phase **only if** the failures are exclusively in the new
  test file and are the specific trait-method-missing errors.
- `cargo test --lib --tests 2>&1 | tail -40` — document the compile errors.

## Deliverable

Write `project-plans/issue173/plan/.completed/P02.md` with:

- List of test functions added (with file:line).
- `grep -c "@plan PLAN-20260416-ISSUE173.P02"` count.
- Compile output showing the expected "no method named ..." errors.
- Verdict: PASS if all required tests exist and compile errors are only the
  expected new-API shape errors.

## Constraints

- NO production code changes in this phase.
- NO changes to existing passing tests.
