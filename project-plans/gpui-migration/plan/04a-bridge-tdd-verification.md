# Phase 04a: Bridge TDD Verification

## Phase ID

`PLAN-20250128-GPUI.P04a`

## Prerequisites

- Phase 04 evidence file exists: `project-plans/gpui-migration/plan/.completed/P04.md`

---

## Verification Protocol

### 1. Test File Exists

```bash
ls -la tests/gpui_bridge_tests.rs
```

Expected: File exists

### 2. Test Count

```bash
grep -c "#\[test\]\|#\[tokio::test\]" tests/gpui_bridge_tests.rs
```

Expected: 12+ tests

### 3. Plan Markers

```bash
grep -c "@plan PLAN-20250128-GPUI.P04" tests/gpui_bridge_tests.rs
```

Expected: 12+ occurrences

### 4. Requirement Markers

```bash
grep -c "@requirement REQ-GPUI-006" tests/gpui_bridge_tests.rs
```

Expected: 10+ occurrences

### 5. Tests Compile

```bash
cargo test --test gpui_bridge_tests --no-run 2>&1 | tail -10
```

Expected: Compiles successfully

### 6. Tests Currently Fail

```bash
cargo test --test gpui_bridge_tests 2>&1 | grep -E "(FAILED|panicked|unimplemented)" | head -5
```

Expected: Tests fail with `unimplemented!` (NOT compile errors)

### 7. Test Coverage Areas

```bash
grep "@scenario" tests/gpui_bridge_tests.rs
```

Expected scenarios:
- GpuiBridge creation
- emit() sends UserEvent
- emit() non-blocking when full
- drain_commands() returns pending
- drain_commands() non-blocking when empty
- has_pending_commands()
- ViewCommandSink send
- ViewCommandSink notifies
- ViewCommandSink notifies when full
- ViewCommandSink clone
- UserEvent forwarder publishes
- UserEvent forwarder exits on disconnect
- Full round-trip test

---

## Evidence File

Create: `project-plans/gpui-migration/plan/.completed/P04A.md`

```markdown
# Phase 04a: Bridge TDD Verification Results

## Verdict: [PASS|FAIL]

## Test File
```bash
$ ls -la tests/gpui_bridge_tests.rs
[paste output]
```

## Test Count
```bash
$ grep -c "#\[test\]\|#\[tokio::test\]" tests/gpui_bridge_tests.rs
[paste output]
```
Count: [N] (expected: 12+)

## Plan Markers
```bash
$ grep -c "@plan PLAN-20250128-GPUI.P04" tests/gpui_bridge_tests.rs
[paste output]
```
Count: [N] (expected: 12+)

## Requirement Markers
```bash
$ grep -c "@requirement REQ-GPUI-006" tests/gpui_bridge_tests.rs
[paste output]
```
Count: [N] (expected: 10+)

## Tests Compile
```bash
$ cargo test --test gpui_bridge_tests --no-run 2>&1 | tail -10
[paste output]
```
Compiles: [YES/NO]

## Tests Fail (TDD)
```bash
$ cargo test --test gpui_bridge_tests 2>&1 | grep -E "(FAILED|panicked)" | head -5
[paste output]
```
Fails with unimplemented: [YES/NO]

## Scenarios Covered
[List scenarios from @scenario markers]

## Verdict Justification
[Explain]
```

---

## Next Phase

After P04a completes with PASS:
--> P05: Bridge Implementation
