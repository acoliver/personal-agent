# Phase 01: Decomposition Design and Test Seams

## Phase ID

`PLAN-20260325-ISSUE11B.P01`

## Objective

Define how the worst GPUI files will be broken apart, what contracts they must preserve, how public module paths will remain stable, and what behavioral safety nets must already be green before any code movement begins.

## Requirements

### REQ-STRUCT-003
Large GPUI files must be decomposed along real behavioral and ownership boundaries rather than arbitrary line-count boundaries.

### REQ-TEST-001
Each refactoring batch must be test-first, with meaningful tests that protect externally visible behavior or stable view/store contracts.

### REQ-API-001
Public API surfaces and re-exports must remain stable where practical, especially for downstream tests and `views/mod.rs` exports.

### REQ-STATIC-001
Source-text tests that reference target files must be explicitly remediated as part of the batch plan.

## Target files

Priority A:
- `src/ui_gpui/views/chat_view.rs`
- `src/ui_gpui/views/main_panel.rs`

Priority B:
- `src/ui_gpui/views/profile_editor_view.rs`
- `src/ui_gpui/views/mcp_add_view.rs`
- `src/ui_gpui/views/settings_view.rs`

Priority C:
- `src/ui_gpui/views/mcp_configure_view.rs`
- `src/ui_gpui/views/model_selector_view.rs`
- `src/ui_gpui/views/api_key_manager_view.rs`

Conditional only:
- `src/ui_gpui/views/history_view.rs` if needed for lizard cleanup or shared refactoring fallout

## Decomposition design rules

- extract by responsibility, not by convenience
- use actual struct/method/function groupings and line ranges from the file
- keep event wiring thin at the view boundary
- move pure data mapping/state transition logic away from render-heavy modules
- maintain or re-export original module paths where practical to avoid unnecessary consumer churn
- any shared abstraction requires at least two real call sites in current scope; otherwise keep it local

## Test-first rules

For every implementation batch, the first executable step must be:
1. identify the behavioral safety-net tests that cover the intended contracts for the batch
2. confirm those tests already pass against the pre-extraction code and capture that green baseline as evidence
3. if the batch also adds genuinely missing behavioral coverage, record that addition separately with a red→green cycle
4. make the smallest structural extraction needed to improve file/function structure while preserving the established green safety net
5. re-run the same targeted tests after extraction, then full local verification for the batch

## Existing test topology notes that must be respected

- `chat_view.rs` currently has no inline test module; its stronger tests live in external GPUI suites such as `tests/chat_view_handle_command_tests.rs` and `tests/gpui_chat_view_tests.rs`
- most other major GPUI view files do contain inline `#[cfg(test)]` GPUI tests
- `tests/main_panel_state_tests.rs` is currently empty and must not be treated as an existing safety net
- `tests/gpui_wiring_command_routing_tests.rs` is currently empty and must not be treated as an existing safety net
- `tests/chat_view_title_visibility_tests.rs` is a zero-value specification stub file and must not count toward safety-net minimums
- do not add brittle source-text tests to replace real GPUI behavioral tests

## Required planning artifacts per target family

- decomposition map with actual code groups / line ranges
- behavior contract inventory
- public API/re-export plan
- `include_str!()` remediation plan
- green-before-move safety-net plan
- inline-test handling decision: keep inline, move with extracted code, or externalize
- expected file/module endpoints after extraction

## Inline `#[cfg(test)]` decision rule

For each target file that currently contains inline tests, the decomposition plan must decide one of:
- keep inline if the tested private invariants remain local to the parent module
- move the inline tests with the extracted code if they still need private access in the extracted module
- externalize the tests if the behavior is better protected through public/behavioral access after extraction

Do not weaken a meaningful inline behavioral test into a routing-only or source-text assertion merely to make extraction easier.

## Scope-creep guard for Phase 01

If decomposition analysis for any single file reveals more than 3 major extraction targets, or requires introducing more than 2 new shared abstractions, stop and update the plan before implementation begins. Phase 01 should clarify execution, not expand into speculative redesign.

## Disallowed shortcuts

- moving code into equally giant helper files
- adding mock-heavy tests that only prove handler routing
- splitting files without reducing conceptual complexity
- leaving dead transitional helpers behind
- silently deleting broken source-text tests without replacement or justification
- breaking `views/mod.rs` exports without an explicit migration plan

## Success Criteria

- each target family has a grounded decomposition map
- each batch has a concrete failing-test-first plan
- public API impacts are documented
- `include_str!()` dependency handling is documented
- planned tests align with `dev-docs/goodtests.md`
