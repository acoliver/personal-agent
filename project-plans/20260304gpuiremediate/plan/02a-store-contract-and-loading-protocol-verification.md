# Phase 02a: Store Contract and Loading Protocol Verification

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P02a`

## Prerequisites

- Required: Phase 02 completed
- Verification: `test -f project-plans/20260304gpuiremediate/plan/.completed/P02.md && grep -E "^(## )?Verdict: PASS$" project-plans/20260304gpuiremediate/plan/.completed/P02.md`

## Verification Commands

```bash
grep -n "selected conversation id\|transcript snapshot\|loading state" project-plans/20260304gpuiremediate/analysis/02-authoritative-store-contract.md
grep -n "ConversationActivated\|ConversationMessagesLoaded\|Loading\|Ready\|Error" project-plans/20260304gpuiremediate/analysis/02-authoritative-store-contract.md project-plans/20260304gpuiremediate/analysis/pseudocode/*.md
grep -n "popup closed\|popup reopened\|startup hydration" project-plans/20260304gpuiremediate/analysis/pseudocode/03-main-panel-integration.md
grep -n "drain_commands\|minting site\|startup transaction\|ConversationCleared\|FinalizeStream" project-plans/20260304gpuiremediate/analysis/02-authoritative-store-contract.md project-plans/20260304gpuiremediate/analysis/pseudocode/01-app-store.md project-plans/20260304gpuiremediate/analysis/pseudocode/03-main-panel-integration.md
```

## Structural Verification Checklist

- [ ] Contract file exists
- [ ] Three pseudocode files exist
- [ ] Numbered pseudocode lines present
- [ ] Startup/runtime convergence language present

## Semantic Verification Checklist

- [ ] Store contract is singular and authoritative
- [ ] Loading protocol is explicit and generation-aware
- [ ] MainPanel is no longer the semantic transcript owner in the target design
- [ ] runtime-ingress owner, single minting site, startup visibility model, finalize authority, and bounded-clear owner are each named explicitly in the analysis set
- [ ] structural grep/readback in this phase is supporting evidence only; PASS requires a coherent written contract with no alternate semantic owner or second startup mutation path left available to implementation
- [ ] Preservation constraints remain intact

## Success Criteria

- Verification shows a coherent target architecture with no ambiguous state owner
