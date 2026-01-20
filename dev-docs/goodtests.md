# Good Tests Guidelines

This project values **behavioral tests**: tests that assert real, externally visible behavior or meaningful processing outcomes. Avoid tests that simply restate implementation details.

## What Makes a Test “Good”

A good test:

- **Validates behavior, not structure.** It proves something observable happened (state changes, IO written, errors returned, data transformed).
- **Exercises realistic scenarios.** It uses real inputs and checks real outputs (even in-memory), not just “method X returns default.”
- **Respects boundaries.** It tests the public API or observable effects, not private helpers or internal wiring.
- **Fails for real regressions.** If behavior changes incorrectly, the test fails; if internals are refactored without behavior change, it should still pass.
- **Uses stable contracts.** It asserts the shape and semantics of data, not intermediate variables.

## Examples of Good Tests

- **Storage persistence**: Save a conversation, read it back, assert fields match.
- **Error handling**: Missing config file returns the documented error.
- **Parsing or transformation**: Convert model responses into tool uses and verify the result.
- **Registry caching**: Write cache, read cache, assert metadata and contents.

## What Makes a Test “Bad”

A bad test:

- **Re-states implementation.** “Call method X and assert it returns what we just passed in.”
- **Checks private wiring.** “Ensure function A calls function B.”
- **Asserts defaults without meaning.** “Default struct has field = 0” when no behavior depends on it.
- **Mocks away all behavior.** If everything is mocked, there’s no real processing left to verify.

## Examples of Bad Tests

- **Identity tests**: `Message::user("hi")` returns role=User (no behavior, just constructor).
- **Mock theater**: A “test” that asserts a mocked call happened, not the outcome.
- **Trivial invariants**: Tests that only confirm formatting or superficial constants.

## Heuristics

Ask these questions before adding a test:

- *Would this test fail if a real bug was introduced?*
- *Does the test assert something observable and meaningful?*
- *Can the implementation change without breaking the test?*

If the answers are **yes**, **yes**, **yes**, it’s likely a good test.

## Preferred Style

- Favor black-box tests over white-box tests.
- Use real inputs and outputs (even with temp files).
- Avoid time-based sleeps unless the behavior depends on timing.
- Keep tests small, focused, and behavior-oriented.

## Test Checklist

- **Behavior first**: The test verifies an observable outcome, not internal wiring.
- **Real processing**: Uses actual inputs/outputs (file IO, parsing, transformations).
- **Stable contract**: Assertions won’t break due to refactors that preserve behavior.
- **Failure signal**: The test would fail if a real regression occurred.
- **Minimal mocks**: Only mock external dependencies, not the logic under test.
