# Execution Tracker — PLAN-20260325-ISSUE11B

## Objective

Refactor oversized GPUI views/components into maintainable, test-first code and remove the CI structural-check exemptions that previously hid this debt.

## Phase Order

- P00 — expose the debt in CI and capture baseline evidence
- P00a — verify the baseline and plan quality
- P01 — design decomposition maps, public API strategy, and test seams for the worst offenders
- P01a — verify decomposition/test plan quality
- P02 — implement `chat_view` + `main_panel` refactor test-first
- P02a — stabilize, prune, and verify batch A
- P03 — implement settings/editor family refactor test-first
- P03a — stabilize, prune, and verify batch B
- P04 — implement remaining GPUI reductions needed for structural compliance
- P04a — stabilize, prune, and verify batch C
- P05 — final local verification and PR readiness
- P05a — final audit

## Mandatory Gates

A phase cannot be marked complete until all of the following exist:
1. evidence of pre-extraction behavioral safety-net tests that already pass for the batch, unless the task is pure baseline capture
2. evidence of the same safety-net tests still passing after the structural extraction
3. evidence of structural verification for touched files
4. evidence that source-text tests and public API consumers affected by the batch were remediated explicitly
5. evidence that no new extracted file became a replacement god-file
6. evidence that inline tests were deliberately preserved, moved, or externalized when extraction required it
7. evidence that Phase 01 produced grounded decomposition maps for the batch and that later phase docs were updated if the grounded seams materially changed the planned grouping

If a batch also adds genuinely missing behavioral coverage, that incremental coverage may follow a red→green cycle and should be recorded separately.

## Review Loop Rule

After plan creation and after each major implementation stage, run a review/remediation loop with:
1. `rustreviewer` reviewing
2. `deepthinker` remediating

Stop when review feedback is only pedantic or when five review rounds total have completed.

## Quantitative Targets

- touched files must be `<= 1000` lines
- target touched files should be `<= 750` lines where practical
- no newly created extracted `.rs` file may exceed `750` lines without explicit written justification
- no touched function may exceed lizard `-L 100`
- no touched function may exceed lizard `-C 50`
- no extracted file may mix more than two of command handling, render implementation, and data/state ownership without written justification
- if `cargo coverage` regresses, the delta must be explained by dead-code removal or stronger test replacement

## Rollback rule

Before starting each implementation batch, create a branch or checkpoint commit boundary. If a batch still has substantive issues after three remediation attempts, revert to the checkpoint and re-plan the decomposition for that batch.

## Current Status

- CI structural excludes for GPUI views/components: removed locally
- issue11b plan scaffold: created and hardened through review/remediation
- implementation: not started
- review rounds completed: 5 / 5
- current review status: latest clean review found only pedantic follow-up items; plan is execution-ready

## Checkpoint references

P02a, P03a, and P04a stabilization checkpoints live in `plan/09-stabilization-and-pruning.md`.
