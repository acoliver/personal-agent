# Phase 04: Remaining GPUI Structural Reduction

## Phase ID

`PLAN-20260325-ISSUE11B.P04`

## Objective

Finish the remaining oversized GPUI reductions needed to satisfy structural gates project-wide.

## Primary targets for this phase

- `src/ui_gpui/views/mcp_configure_view.rs`
- `src/ui_gpui/views/model_selector_view.rs`
- `src/ui_gpui/views/api_key_manager_view.rs`

## Conditional targets only if evidence justifies them

- `src/ui_gpui/views/history_view.rs` if function-level lizard cleanup is still needed outside the already-known `render_conversation_card` violation
- GPUI components if extracted modules or later measurements reveal new structural debt

## Baseline debt driving this phase

- `mcp_configure_view.rs` — `1471` lines
- `model_selector_view.rs` — `1404` lines
- `api_key_manager_view.rs` — `1200` lines
- `history_view.rs` — `647` lines, already file-length compliant
- lizard hotspot: `api_key_manager_view.rs::render_edit_form` — 239 length
- lizard hotspot: `history_view.rs::render_conversation_card` — 116 length

## Required test-first work

Before moving code in this phase:
- strengthen existing suites such as `tests/mcp_configure_view_can_save_tests.rs`, `tests/model_selector_presenter_tests.rs`, `tests/history_view_tests.rs`, and any existing GPUI tests in the target files
- if `history_view.rs` has not already been fixed earlier, add or confirm safety-net coverage for `render_conversation_card` behavior because it already exceeds the `-L 100` threshold
- replace or narrow any remaining `include_str!()` tests that reference these files and are invalidated by decomposition
- add tests covering real save/selection/filtering/auth/edit behavior rather than source text or routing trivia

Known source-text dependency:
- `tests/api_key_manager_ui_regression_tests.rs`
- `tests/seven_bugs_regression_tests.rs` for `history_view.rs`

## Success Criteria

- each oversized target file is `<= 1000` lines after the batch, with `<= 750` as the target where practical
- `history_view.rs::render_conversation_card` is remediated in this phase unless it was already fixed in an earlier batch
- no newly created extracted `.rs` file exceeds `750` lines without written justification
- touched functions satisfy lizard `-L 100` and `-C 50`
- structural checks pass without GPUI exemptions
- batch verification includes `cargo coverage`
