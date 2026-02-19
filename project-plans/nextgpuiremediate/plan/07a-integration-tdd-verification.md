# Phase 07a: Integration TDD Verification

## Phase ID

`PLAN-20260219-NEXTGPUIREMEDIATE.P07a`

## Prerequisites

- Required: Phase 07 completed
- Verification: `test -f project-plans/nextgpuiremediate/plan/.completed/P07.md`

## Verification Commands

```bash
ls tests/gpui_integration_profile_flow.rs
ls tests/gpui_integration_conversation_flow.rs
ls tests/gpui_integration_mcp_flow.rs

grep -r "@plan PLAN-20260219-NEXTGPUIREMEDIATE.P07" tests/gpui_integration_*
grep -r "@requirement REQ-INT-002" tests/gpui_integration_*

grep -r "should_panic" tests/gpui_integration_* && exit 1 || true
cargo test gpui_integration -- --nocapture
```

## Structural Checklist

- [ ] Test artifacts exist
- [ ] Required markers exist
- [ ] Fail-before-impl behavior confirmed

## Semantic Checklist

- [ ] Tests model real user paths in active GPUI runtime
- [ ] Assertions are outcome-based, not implementation-detail-based

## Success Criteria

- Integration TDD phase validated; safe to proceed to integration implementation

## Phase Completion Marker

Create: `project-plans/nextgpuiremediate/plan/.completed/P07a.md`
