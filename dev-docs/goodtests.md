# Good Tests Guidelines

This project values **behavioral tests**: tests that assert real, externally visible behavior or meaningful processing outcomes.

Good tests increase confidence that the feature actually works.
Bad tests mainly increase confidence that the test harness, mocks, or current implementation wiring behave the way the test arranged them.

See also: [RUST-RULES.md](./RUST-RULES.md)

---

## The Standard

A bad test proves the test’s own setup or the current implementation wiring.
A good test proves a behavior or contract that matters outside the code under test.

---

## The Three Categories

## 1. Behavioral Tests — preferred

A behavioral test proves something meaningful outside the code’s internal structure.

Examples:
- save data, reload it, and verify persistence
- invalid input returns the documented error
- stale response is ignored and current state is preserved
- pressing a button causes the user-visible state to change correctly
- a request built for an external boundary has the correct contract

These are the tests that should carry most of the confidence.

## 2. Structural Tests — allowed but weak

A structural test mainly proves shape, construction, routing, or state projection.

Examples:
- builder/setter methods store fields correctly
- enum variants are constructible
- routing function maps command X to target Y
- default state starts empty

These are not worthless, but they are weak evidence and do not substitute for behavioral coverage.

## 3. Mock Theater — usually bad

A mock-theater test mainly proves the test harness or mocked plumbing, not the product behavior.

Examples:
- asserting a mock was called without proving the user-visible effect
- asserting internal steps in a call chain instead of the resulting behavior
- forwarding fake values through fake layers and then asserting the same fake values arrived

These tests may occasionally help while building a seam, but they are weak evidence and should not dominate the suite.

---

## What Makes a Test “Good”

A good test:

- **Validates behavior, not structure.** It proves something observable happened: state changes, IO written, errors returned, data transformed, UI changed, or a real contract held.
- **Exercises realistic scenarios.** It uses real inputs and checks real outputs, even if the test uses temp files or in-memory boundaries.
- **Respects public boundaries.** It tests a public API, a semantic contract, or an observable effect, not private helpers or incidental internals.
- **Fails for real regressions.** If behavior changes incorrectly, the test fails. If internals are refactored without behavior change, it should usually still pass.
- **Uses stable contracts.** It asserts semantics and outcomes, not incidental variables or exact internal choreography.

## Examples of Good Tests

- **Storage persistence**: Save a conversation, read it back, assert fields match.
- **Error handling**: Missing config file returns the documented error.
- **Parsing or transformation**: Convert model responses into tool uses and verify the result.
- **Registry caching**: Write cache, read cache, assert metadata and contents.
- **UI behavior**: Mounted GPUI view reflects realistic store or command changes.
- **Regression protection**: A previously broken user-visible flow now stays correct.

---

## What Makes a Test “Bad”

A bad test:

- **Creates the fact it later asserts.** The test injects a value and then “proves” the same value is present.
- **Re-states implementation.** “Call method X and assert it returns what we just passed in.”
- **Checks private wiring.** “Ensure function A calls function B” when the call itself is not the contract.
- **Asserts defaults without meaning.** “Default struct has field = 0” when no real behavior depends on it.
- **Mocks away all behavior.** If everything important is mocked, there is no real processing left to verify.
- **Overfits to the current call graph.** A harmless refactor breaks the test even though behavior stayed correct.

## Examples of Bad Tests

### Mock tautology / self-fulfilling test

```rust
let mut thing = MockThing::default();
thing.value = "some string".to_string();
assert_eq!(thing.value, "some string");
```

This only proves that the test can read back the value it just assigned.

### Interaction-only test

```rust
sut.do_work();
assert!(dependency.was_called());
```

Usually weak or bad because it proves implementation structure rather than outcome.

### Wiring-chain theater

```rust
button.click();
assert!(presenter.was_called());
assert!(backend.was_called());
assert_eq!(label.text(), "mocked value we invented");
```

This is better than a pure tautology, but still weak if the main evidence is only that mocked layers forwarded mocked values.

### Other low-value patterns

- **Identity tests**: constructor returns the same fields you passed in
- **Mock theater**: asserting mocked calls instead of outcomes
- **Trivial invariants**: tests that only confirm formatting or superficial constants

---

## When Interaction Assertions Are Acceptable

Interaction assertions are acceptable when the interaction itself is the behavior contract at a real boundary, such as:
- writing to storage
- publishing an event
- sending an external request
- enqueueing a job

Even then, prefer asserting meaningful payload or contract details rather than just call counts.

---

## Heuristics

Ask these questions before adding or keeping a test:

- *Did the test create the fact it later asserts?*
- *Would this test fail if a real bug was introduced?*
- *Does the test assert something observable and meaningful?*
- *Can the implementation change without breaking the test?*
- *Are mocks acting as inputs, or are they the whole thing being “proven”?*
- *Is this the strongest test we can reasonably write for this behavior?*

If the answers are broadly **no**, **yes**, **yes**, **yes**, **inputs**, **yes**, the test is probably strong.

---

## Preferred Style

- Favor black-box tests over white-box tests.
- Use real inputs and outputs, even when using temp files or deterministic fakes.
- Avoid time-based sleeps unless timing is part of the behavior.
- Keep tests small, focused, and behavior-oriented.
- Use mocks and fakes as boundary controls, not as the main evidence.

---

## Test Checklist

- **Behavior first**: The test verifies an observable outcome, not internal wiring.
- **Real processing**: Uses actual inputs/outputs, transformations, persistence, or visible state changes.
- **Stable contract**: Assertions should survive refactors that preserve behavior.
- **Failure signal**: The test would fail if a real regression occurred.
- **Minimal mocks**: Only mock external dependencies or nondeterministic boundaries.
- **No tautologies**: The test does not merely prove values the test itself injected.
- **No theater**: The test does not rely mainly on mocked call chains as evidence.
