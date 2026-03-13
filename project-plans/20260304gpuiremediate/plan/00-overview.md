# Plan: GPUI Chat State Delivery Recovery Architecture

Plan ID: PLAN-20260304-GPUIREMEDIATE
Generated: 2026-03-04
Total Phase Pairs: 13
Execution Steps: 26 (`P00 -> P00a -> ... -> P12 -> P12a`)
Requirements: REQ-ARCH-001, REQ-ARCH-002, REQ-ARCH-003, REQ-ARCH-004, REQ-ARCH-005, REQ-ARCH-006, REQ-INT-001, REQ-INT-002

## Critical Reminders

Before implementing ANY execution phase, ensure you have:

1. Completed overview alignment (P00) and preflight verification (P00a)
2. Proven the current failure mode with file-cited evidence
3. Written failing convergence tests before implementation
4. Verified startup/manual selection use one store contract before deprecating bootstrap logic
5. Preserved first-frame correctness by batching startup hydration into one initial store publication

## Scope

This plan is focused strictly on stabilizing GPUI chat state delivery in the active GPUI runtime.

In scope:
- `src/main_gpui.rs`
- `src/ui_gpui/app_store.rs` as the concrete target store module path unless P00a proves a better existing equivalent
- `src/ui_gpui/views/main_panel.rs`
- `src/ui_gpui/views/chat_view.rs`
- `src/ui_gpui/views/history_view.rs`
- `src/presentation/chat_presenter.rs`
- `src/presentation/history_presenter.rs`
- `src/presentation/view_command.rs`
- Any additional directly required store/runtime integration files and tests

Out of scope:
- unrelated UI redesign
- unrelated presenter/service refactors
- legacy non-GPUI UI migration work
- speculative architecture expansion beyond chat-state delivery recovery

## Core Defect Driving This Plan

Startup first-frame rendering works because a synchronous bootstrap path applies startup commands directly into mounted views. Manual conversation selection is broken because post-startup `ConversationMessagesLoaded` delivery depends on popup-coupled forwarding/polling instead of one authoritative durable state owner.

## Mandatory Sequence

`P00 -> P00a -> P01 -> P01a -> P02 -> P02a -> P03 -> P03a -> P04 -> P04a -> P05 -> P05a -> P06 -> P06a -> P07 -> P07a -> P08 -> P08a -> P09 -> P09a -> P10 -> P10a -> P11 -> P11a -> P12 -> P12a`

No skipped steps permitted.

## Phase Map

- **P00/P00a**: overview alignment + preflight verification gate
- **P01/P01a**: prove current failure mode and document state-path split
- **P02/P02a**: define authoritative store contract and explicit loading protocol
- **P03/P03a**: write failing tests for startup/manual convergence first
- **P04/P04a**: implement store skeleton and snapshot ownership
- **P05/P05a**: integrate presenter/runtime updates through store
- **P06/P06a**: migrate startup hydration onto the same authoritative reducer semantics/state contract
- **P07/P07a**: prove popup independence and reopen correctness
- **P08/P08a**: simplify MainPanel and deprecate redundant bootstrap assumptions
- **P09/P09a**: preserve transcript/layout/provider behaviors and regressions
- **P10/P10a**: code quality, coverage expectations, anti-placeholder gates
- **P11/P11a**: full project verification suite
- **P12/P12a**: final recovery evidence and handoff

## Pseudocode Mapping

- `analysis/pseudocode/01-app-store.md`
- `analysis/pseudocode/02-selection-loading-protocol.md`
- `analysis/pseudocode/03-main-panel-integration.md`

Implementation and test-writing phases must cite pseudocode line ranges explicitly and require matching `@plan`, `@requirement`, and `@pseudocode` markers in the touched tests/functions/structs.

## Marker Syntax Rule

Repo planning docs show mixed marker examples (`@plan:...` in some grep/template sections and `@plan ...` in code-traceability examples). For this plan:

- canonical emitted code-marker form remains the space form used in `dev-docs/PLAN.md` code traceability examples:
  - `/// @plan[: ]PLAN-20260304-GPUIREMEDIATE.PNN`
  - `/// @requirement[: ]REQ-...`
  - `/// @pseudocode[: ]analysis/pseudocode/...:start-end`
- verification commands and evidence checks must tolerate either space-form or colon-form markers when reading legacy/template-aligned output
- if implementation chooses colon-form markers for a bounded repo-convention reason, phase evidence must call that out explicitly and verification must still accept it rather than failing on syntax alone

## Normative Shorthand -> Repo-Mapped Equivalent Rule

Several seam names in this plan are normative shorthand for responsibilities, not mandatory final function spelling. To keep execution literal-safe, every implementation and verification artifact must map any repo-idiomatic equivalent back to this table one-to-one.

| Plan shorthand | Required responsibility | Acceptable equivalent shape | Forbidden split/near-miss shapes | Evidence requirement |
|---|---|---|---|---|
| `spawn_runtime_bridge_pump(...)` | sole production GPUI-owned bridge drainer after Phase 05 | one runtime-owned app-root ingress function/task in `src/main_gpui.rs` or preflight-approved replacement | popup-owned drainer, dual drainers, popup-retained proxy as semantic ingress owner | evidence names exact file/function and proves sole-production-drainer responsibility |
| `handle_select_conversation_intent(...)` | sole ordinary-runtime GPUI selection dispatch boundary before async transcript load begins | one GPUI-owned selection-intent handler rooted at app runtime boundary | direct presenter dispatch from views, remount/history-refresh/compatibility hook initiating selection load work | evidence names exact file/function and proves all production selection dispatch flows through it |
| `begin_selection(...)` | sole ordinary-runtime minting/selection-loading authority transition | one authoritative store-boundary minting helper/function | presenter/bridge/view minting, second helper that can mint independently, split mint + load-state ownership | evidence names exact file/function and proves it is the only ordinary-runtime minting site |
| `reduce_startup_batch(startup_inputs)` | sole startup selected-conversation semantic transaction | one startup-only authoritative store transaction entrypoint | second startup semantic mutator, compatibility shim mutating selected state outside the entrypoint | evidence names exact file/function and proves startup-selected hydration semantics are centralized there |

Interpretation rule: if implementation uses different names, phase evidence must explicitly map those names to this table and prove the responsibilities were not split.


## Forbidden Implementation Patterns

This recovery must modify/integrate existing seams and must not introduce parallel ownership structures. The following patterns are forbidden unless the specification is explicitly amended first:

- popup-local durable transcript ownership after the store becomes authoritative
- bridge-side or presenter-side snapshot caches that mirror authoritative chat state
- a second reducer boundary in `MainPanel`, `ChatView`, or any compatibility shim
- duplicate modules or files such as `app_store_v2`, `*_v2`, `*_new`, `*_copy`, or a parallel `MainPanelSnapshotState`
- test-only enums/structs that mirror `ViewCommand` instead of using the real protocol types
- leaving old direct replay/bootstrap paths semantically authoritative after the phase that claims authority transfer
- claiming Phase 04 ownership success with a compile-only shell that does not transfer any live render-driving field to store authority
