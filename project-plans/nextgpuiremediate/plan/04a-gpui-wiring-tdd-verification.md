# Phase 04a: GPUI Wiring TDD Verification

## Phase ID

`PLAN-20260219-NEXTGPUIREMEDIATE.P04a`

## Prerequisites

- Required: Phase 04 completed
- Verification: `test -f project-plans/nextgpuiremediate/plan/.completed/P04.md`

## Verification Commands

```bash
# Files and markers
ls tests/gpui_wiring_event_flow_tests.rs
ls tests/gpui_wiring_command_routing_tests.rs
grep -r "@plan PLAN-20260219-NEXTGPUIREMEDIATE.P04" tests/gpui_wiring_*
grep -r "@requirement REQ-WIRE-" tests/gpui_wiring_*

# Anti reverse-testing
grep -r "should_panic" tests/gpui_wiring_* && exit 1 || true

# Confirm fail-before-impl behavior
cargo test gpui_wiring -- --nocapture
```

## Structural Checklist

- [ ] TDD artifacts exist
- [ ] Marker coverage exists
- [ ] Reverse-testing not present
- [ ] Tests fail prior to implementation

## Semantic Checklist

- [ ] Failure mode indicates missing implementation (not compile chaos)
- [ ] Tests would fail if routing logic were removed
- [ ] Tests would fail if event mismatch handlers were absent

## Success Criteria

- TDD phase validated and implementation-ready

## Failure Recovery

- Refine tests to improve behavior assertions
- Re-run verification

## Phase Completion Marker

Create: `project-plans/nextgpuiremediate/plan/.completed/P04a.md`

```markdown
Phase: P04a
Completed: YYYY-MM-DD HH:MM
Verification: PASS/FAIL
Notes: [why]
```
