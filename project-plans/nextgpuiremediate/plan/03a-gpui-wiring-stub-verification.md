# Phase 03a: GPUI Wiring Stub Verification

## Phase ID

`PLAN-20260219-NEXTGPUIREMEDIATE.P03a`

## Prerequisites

- Required: Phase 03 completed
- Verification: `test -f project-plans/nextgpuiremediate/plan/.completed/P03.md`
- Expected files from previous phase:
  - `src/main_gpui.rs`
  - `src/presentation/model_selector_presenter.rs`
  - `src/presentation/settings_presenter.rs`

## Verification Commands

### Structural Verification

```bash
# Marker checks
grep -r "@plan PLAN-20260219-NEXTGPUIREMEDIATE.P03" src/main_gpui.rs src/presentation | wc -l
grep -r "@requirement REQ-WIRE-" src/main_gpui.rs src/presentation | wc -l

# Build
cargo build --bin personal_agent_gpui || exit 1
```

### Sequencing Verification

```bash
# Ensure no skip past P03 before P03a
test -f project-plans/nextgpuiremediate/plan/.completed/P03.md
```

## Structural Checklist

- [ ] P03 completion marker exists
- [ ] P03 markers present in modified files
- [ ] Compilation succeeds
- [ ] Stub remains minimal and non-destructive

## Semantic Verification Checklist

1. **Does stub preserve the plan direction?**
   - [ ] Constructor signatures now permit unified channel path
   - [ ] No behavioral claims beyond stub scope

2. **Is it real, not fake progress?**
   - [ ] Build passes
   - [ ] No fake test-only changes

3. **Reachability**
   - [ ] Stub code is called from startup path (or wired for immediate next phase)

## Success Criteria

- P03 artifact validated and safe to proceed to TDD phase

## Failure Recovery

- Re-open P03 and patch gaps
- Re-run verification commands

## Phase Completion Marker

Create: `project-plans/nextgpuiremediate/plan/.completed/P03a.md`

```markdown
Phase: P03a
Completed: YYYY-MM-DD HH:MM
Verification:
- Marker checks: PASS/FAIL
- Build: PASS/FAIL
Verdict: PASS/FAIL
```
