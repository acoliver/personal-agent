# Phase 12: End-to-End Concurrent Streams Integration Test

Plan ID: `PLAN-20260416-ISSUE173.P12`

## Prerequisites

- P11 verified PASS.

## Requirements implemented

- REQ-173-006.1 (plus re-confirmation of 001/002/003/004)

## Tasks

Add a new integration-style test module:
`src/services/chat_impl/tests/three_stream_concurrency.rs` (added to
`src/services/chat_impl/tests.rs` mod list).

Test:

```rust
/// @plan PLAN-20260416-ISSUE173.P12
/// @requirement REQ-173-006.1
#[tokio::test]
async fn three_conversations_stream_in_parallel_and_cancel_scopes_correctly() {
    // 1. Build a ChatServiceImpl with mocks that yield deterministic deltas
    //    slowly (e.g. 3 tokens, each with a 10 ms delay). Use the same mock
    //    strategy already used in chat_impl/tests.rs; extend it as needed to
    //    support multiple concurrent conversations.

    // 2. Create conversations A, B, C via the mock ConversationService.

    // 3. Kick off send_message(A, ...), send_message(B, ...), send_message(C, ...)
    //    in quick succession (spawn each, don't await the stream immediately).

    // 4. Assert all three return Ok(Box<Stream>) — none rejected.

    // 5. Assert service.is_streaming_for(A/B/C) all true right after spawn.

    // 6. Drive A and C streams to completion; assert their final transcripts
    //    are correct and isolated.

    // 7. Mid-flight, call service.cancel(B). Assert:
    //    - service.is_streaming_for(B) == false,
    //    - service.is_streaming_for(A) still true (or already completed),
    //    - service.is_streaming_for(C) still true (or already completed),
    //    - exactly one ChatEvent::StreamCancelled observed with conversation_id = B.

    // 8. Assert the mocked ConversationService received add_message calls for
    //    A and C but NOT for B's assistant turn (B was cancelled before
    //    finalize).
}
```

If the existing test mocks don't yet support concurrent streams, add a
`#[cfg(test)]` helper mock (e.g. `DelayingMockLlmClient`) in
`src/services/chat_impl/tests/` that emits controlled deltas. Keep the test
deterministic (use `tokio::time::pause()` if necessary).

Also add a smaller approval-gate-layer test to the same module:

```rust
/// @plan PLAN-20260416-ISSUE173.P12
/// @requirement REQ-173-003.1
#[tokio::test]
async fn cancel_of_a_does_not_resolve_bs_pending_approval() {
    // Build ChatServiceImpl with approval_gate shared.
    // Register two waiters (one for A, one for B) via approval_gate directly.
    // Call service.cancel(A).
    // Assert A's waiter woke with false, B's waiter still pending
    // (use timeout-based assertion).
}
```

## Verification

```bash
cargo build --all-targets 2>&1 | tail -5
cargo test --lib three_stream_concurrency 2>&1 | tail -20
cargo test --lib --tests 2>&1 | grep -E "test result|FAILED" | tail -10
```

Must pass with 0 failures. Record all outputs in
`project-plans/issue173/plan/.completed/P12.md`.
