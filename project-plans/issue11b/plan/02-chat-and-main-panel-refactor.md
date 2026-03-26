# Phase 02: Chat View and Main Panel Refactor

## Phase ID

`PLAN-20260325-ISSUE11B.P02`

## Objective

Break down `chat_view.rs` and `main_panel.rs` first, since they are the worst structural offenders and central UI surfaces.

## Baseline debt driving this phase

- `chat_view.rs` — `2527` lines
- `main_panel.rs` — `2019` lines
- lizard hotspots:
  - `chat_view.rs::handle_command` — 393 length
  - `chat_view.rs::render_top_bar` — 140 length
  - `chat_view.rs::render_title_bar` — 186 length
  - `chat_view.rs::render_input_bar` — 159 length
  - `chat_view.rs::render` — 224 length
  - `main_panel.rs::handle_command` — 647 length, `51` CCN
  - `main_panel.rs::render` — 230 length

## Grounded decomposition targets

These targets are preliminary until Phase 01 produces grounded decomposition maps from the real file structure. If Phase 01 materially changes the grouping, update this phase document before implementation begins.

### `chat_view.rs`
Expected decomposition map must be grounded around real code groups such as:
- state/data types (`ChatMessage`, `MessageRole`, `StreamingState`, `ChatState`)
- transcript reconstruction / store projection helpers
- conversation dropdown behavior
- profile dropdown behavior
- command handling / command reducers
- input editing helpers / entity input handler behavior
- render subtrees (`top_bar`, `title_bar`, `chat_area`, `input_bar`, overlays)

### `main_panel.rs`
Expected decomposition map must be grounded around real code groups such as:
- `route_view_command` routing matrix and observable test target helpers
- store snapshot application and subscription setup
- bridge/runtime startup helpers
- child-view initialization and forwarding helpers
- `handle_command` routing logic
- render/navigation composition

`route_view_command` is an especially strong early extraction candidate because it already has a standalone signature. However, its current support coverage is mostly inline and structural; external behavioral safety-net coverage must be established before extraction rather than assumed.

## Required test-first work

Before moving code in this phase:
- confirm a green behavioral safety net for `chat_view` using real GPUI tests covering command handling, dropdown behavior, streaming/thinking transitions, and selection/history-visible state
- treat `tests/chat_view_handle_command_tests.rs` as the current primary external `chat_view` safety-net file and verify that its 9 `#[gpui::test]` tests actually cover the required contracts before extraction
- treat `tests/gpui_chat_view_tests.rs` as empty and do not count it as existing safety-net coverage
- create and confirm a green behavioral safety net for `main_panel` because `tests/main_panel_state_tests.rs` is currently empty and cannot be relied upon
- treat `route_view_command` / `CommandTargets` tests as structural support tests only; they do not satisfy the behavioral safety-net requirement by themselves
- if new missing behavioral coverage is added, that new coverage may follow a red→green cycle, but extraction safety is proven with green-before-move and green-after-move
- identify and remediate all `include_str!()` tests referencing `chat_view.rs` or `main_panel.rs`
- decide where inline `main_panel.rs` tests will live after extraction if private helpers move

Known external/source-text dependencies to account for include:
- `tests/chat_view_handle_command_tests.rs`
- `tests/gpui_chat_view_tests.rs` (currently empty; must not be treated as existing safety-net coverage)
- `tests/main_panel_state_tests.rs` (currently empty; must be populated or replaced by another real safety-net file before extraction)
- `tests/gpui_wiring_command_routing_tests.rs` (currently empty; must not be treated as existing safety-net coverage)
- `tests/chat_view_title_visibility_tests.rs` (stub-only; must not count toward safety-net minimums)
- `tests/chat_startup_scrollback_layout_regression_tests.rs`
- `tests/gpui_popup_independence_tests.rs`
- `tests/chat_view_conversation_switch_regression_tests.rs`
- `tests/api_key_manager_ui_regression_tests.rs`
- `tests/seven_bugs_regression_tests.rs`

## Public API / import guardrails

- keep `personal_agent::ui_gpui::views::chat_view::*` consumers working via re-exports where practical
- keep `personal_agent::ui_gpui::views::main_panel::*` consumers working or deliberately update them in the same batch
- update `src/ui_gpui/views/mod.rs` deliberately, not incidentally

## Coverage guardrail

The workspace is already below the enforced `80%` gate. This batch therefore cannot claim success merely by preserving the current failing state. After the batch:
- `cargo coverage` must still run and its result must be recorded
- newly extracted substantial modules must not be left effectively untested
- if coverage does not improve, the evidence must explain why the refactor was still necessary and what follow-on coverage work remains

## Success Criteria

- `chat_view.rs` and `main_panel.rs` are both `<= 1000` lines after the batch, with `<= 750` as the target where practical
- no newly created extracted `.rs` file exceeds `750` lines without written justification
- touched functions satisfy lizard `-L 100` and `-C 50`
- required behavioral safety-net tests pass before extraction and after extraction
- source-text tests referencing moved code were replaced, narrowed, or deliberately updated with rationale
- `route_view_command` was handled deliberately as part of the decomposition plan
- full batch verification passes, including `cargo coverage`
