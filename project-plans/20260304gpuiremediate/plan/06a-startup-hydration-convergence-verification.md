# Phase 06a: Startup Hydration Convergence Verification

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P06a`

## Prerequisites

- Required: Phase 06 completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P06.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P06.md`

## Verification Commands

```bash
cargo check -q
cargo test --test chat_startup_scrollback_layout_regression_tests --test presenter_selection_and_settings_tests -- --nocapture
grep -R -n "@plan[: ]PLAN-20260304-GPUIREMEDIATE.P06" src/ui_gpui src/main_gpui.rs --include="*.rs"

grep -R -n "@requirement[: ]REQ-ARCH-002.1\|@requirement[: ]REQ-ARCH-002.2\|@requirement[: ]REQ-ARCH-002.5\|@requirement[: ]REQ-ARCH-006.3" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -R -n "@pseudocode[: ]analysis/pseudocode/01-app-store.md:\|@pseudocode[: ]analysis/pseudocode/03-main-panel-integration.md:" src/ui_gpui src/main_gpui.rs --include="*.rs"
grep -rn "startup_commands\|apply_startup_commands\|current_snapshot\|reduce_batch\|revision" src/main_gpui.rs src/ui_gpui/views/main_panel.rs src/ui_gpui --include="*.rs"
grep -E "^(## )?(Named startup-atomicity proof artifact|Startup hydration mode classification|Exact test|Command|Observed result|Why this proves the invariant)" project-plans/20260304gpuiremediate/plan/.completed/P06a.md
```

## Structural Verification Checklist

- [ ] Startup state flows through authoritative store/reducer path
- [ ] First render consumes current snapshot
- [ ] Required `@plan`, `@requirement`, and `@pseudocode` markers are present in touched production items
- [ ] Remaining bootstrap-only logic, if any, is explicitly bounded
- [ ] verification artifact names the exact shared startup/runtime selection-loading entry path used in code (including `begin_selection(...)` and the reducer-batch function)

## Semantic Verification Checklist

- [ ] Startup first-frame correctness preserved
- [ ] No divergence between startup and runtime transcript semantics
- [ ] Startup hydration is batched so no intermediate empty/loading flash appears for known startup data
- [ ] Startup publication is atomic from subscriber perspective for already-known transcript data
- [ ] named deterministic startup-atomicity proof uses one mandatory harness artifact that combines a subscriber-visible snapshot/revision observer with first-subscriber `current_snapshot()` readback, and states exact test, command, observed result, and why it proves the required startup ordering: full startup batch completed, final snapshot/revision committed, any pre-subscription publication was silent by contract, and the first subscriber/render reads the already-committed `current_snapshot()`
- [ ] named deterministic proof falsifies any startup implementation that mounts against pre-hydration default state or publishes visible `Loading` when startup transcript was already known
- [ ] if Startup Mode B was required, the verification artifact names the chosen seam class, exact source file/function origin, and why the other two seam classes do not apply

- [ ] verification evidence shows first render reads the already-current snapshot rather than replaying a queued startup transcript event
- [ ] grep/readback in this phase is supporting evidence only; PASS requires the named startup-atomicity proof artifact above
- [ ] Startup scrollback/layout regressions remain green

## Success Criteria

- Verification can explain exactly how startup now reaches the same durable state owner as runtime
