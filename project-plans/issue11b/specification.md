# Feature Specification: Remove GPUI Structural Exemptions and Refactor Oversized GPUI Views Test-First

## Purpose

Stop hiding GPUI structural debt behind CI exclusions and replace the current oversized view files with maintainable, test-protected code that still improves honest release confidence and does not damage the enforced 80% coverage path.

## Problem Statement

The current GPUI UI layer contains multiple view files that are far beyond the project’s own structural standards. The CI workflow previously excluded `src/ui_gpui/views/*` and `src/ui_gpui/components/*` from file-length and complexity checks instead of reducing the debt. The user explicitly rejected that workaround.

This plan therefore does two things together:
1. removes the unjustified GPUI structural exemptions, and
2. drives a test-first refactoring of the worst GPUI files into smaller, coherent modules.

## Hard Constraints

1. This work must be test-first.
2. The result must be genuinely better code, not cosmetic file splitting.
3. Added or changed tests must satisfy `dev-docs/goodtests.md`.
4. Structural debt must be fixed rather than hidden.
5. Honest coverage still matters: the repo’s enforced coverage gate remains `80.0%` in `xtask/src/main.rs`.
6. Review/remediation must loop with `deepthinker` and `rustreviewer` until only pedantic issues remain or five review rounds have completed.

## CI Structural Thresholds That This Plan Must Satisfy

From `.github/workflows/pr-quality-and-e2e.yml`:
- file length warning: over `750` lines
- file length failure: over `1000` lines
- lizard cyclomatic complexity failure threshold: `-C 50`
- lizard function length failure threshold: `-L 100`

These thresholds apply to GPUI views/components after the exemptions are removed.

## Current Baseline

### Coverage baseline
Latest local workspace summary from `target/llvm-cov-target/workspace-summary.json`:
- line coverage: `64.83141082519964%` (`14613 / 22540`, missed `7927`)
- enforced gate remains `80.0%`

### Largest GPUI file-length offenders
- `src/ui_gpui/views/chat_view.rs` — `2527` lines
- `src/ui_gpui/views/main_panel.rs` — `2019`
- `src/ui_gpui/views/profile_editor_view.rs` — `1875`
- `src/ui_gpui/views/mcp_add_view.rs` — `1696`
- `src/ui_gpui/views/settings_view.rs` — `1485`
- `src/ui_gpui/views/mcp_configure_view.rs` — `1471`
- `src/ui_gpui/views/model_selector_view.rs` — `1404`
- `src/ui_gpui/views/api_key_manager_view.rs` — `1200`
- `src/ui_gpui/views/history_view.rs` — `647` lines, already below file-length thresholds

### Current GPUI lizard hotspots
Collected locally with `python -m lizard -C 50 -L 100 -w src/ui_gpui/views src/ui_gpui/components`:
- `chat_view.rs::handle_command` — 393 length
- `chat_view.rs::render_top_bar` — 140 length
- `chat_view.rs::render_title_bar` — 186 length
- `chat_view.rs::render_input_bar` — 159 length
- `chat_view.rs::render` — 224 length
- `main_panel.rs::handle_command` — 647 length, `51` CCN
- `main_panel.rs::render` — 230 length
- `settings_view.rs::handle_command` — 123 length
- `settings_view.rs::render_profiles_section` — 143 length
- `settings_view.rs::render_mcp_section` — 140 length
- `api_key_manager_view.rs::render_edit_form` — 239 length
- `history_view.rs::render_conversation_card` — 116 length
- several GPUI test functions inside view files also exceed `-L 100`

### GPUI components baseline
Current component files are small. Largest measured component file is `src/ui_gpui/components/secure_text_field.rs` at `170` lines. Components are still in scope for enforcement, but they are not the main structural problem today.

## Scope

### Primary refactor targets
Ordered by structural urgency:
1. `src/ui_gpui/views/chat_view.rs`
2. `src/ui_gpui/views/main_panel.rs`
3. `src/ui_gpui/views/profile_editor_view.rs`
4. `src/ui_gpui/views/mcp_add_view.rs`
5. `src/ui_gpui/views/settings_view.rs`
6. `src/ui_gpui/views/mcp_configure_view.rs`
7. `src/ui_gpui/views/model_selector_view.rs`
8. `src/ui_gpui/views/api_key_manager_view.rs`

### Secondary / conditional targets
- `src/ui_gpui/views/history_view.rs` only for function-level lizard cleanup or if touched by shared-contract work
- GPUI components only if new structural issues appear while refactoring or if an extracted subview clearly belongs there
- `src/ui_gpui/views/mod.rs` re-exports and downstream imports that must stay stable
- tests that use `include_str!("../src/ui_gpui/views/...`)` and therefore will break or become invalid during extraction unless deliberately remediated

## Test Landscape Constraints

### Strong GPUI directions to extend
- `tests/chat_view_handle_command_tests.rs` (currently the main external `chat_view` safety-net file, with 9 `#[gpui::test]` tests that must be mapped to the required contracts before extraction)
- `tests/settings_view_tests.rs`
- `tests/settings_view_mcp_saved_state_tests.rs`
- `tests/settings_view_display_tests.rs`
- `tests/settings_panel_behavior_tests.rs`
- `tests/profile_editor_view_tests.rs`
- `tests/profile_editor_save_payload_mapping_tests.rs`
- `tests/mcp_add_view_projection_tests.rs`
- `tests/mcp_configure_view_can_save_tests.rs`
- `tests/history_view_tests.rs`
- inline `#[gpui::test]` modules inside `main_panel.rs`, `settings_view.rs`, `profile_editor_view.rs`, `mcp_add_view.rs`, `mcp_configure_view.rs`, `model_selector_view.rs`, `api_key_manager_view.rs`, and `history_view.rs`

### Structural support tests that do not count as sufficient behavioral safety nets on their own
- `route_view_command` / `CommandTargets` style tests inside `main_panel.rs`
- other routing-only or counter-only tests proving command-to-target wiring without user-visible state change

### Weak / source-text suites that must be actively managed
These suites currently use `include_str!()` against GPUI source files and cannot be ignored during refactoring:
- `tests/model_profile_flow_regression_tests.rs`
- `tests/chat_startup_scrollback_layout_regression_tests.rs`
- `tests/gpui_popup_independence_tests.rs`
- `tests/api_key_manager_ui_regression_tests.rs`
- `tests/chat_view_conversation_switch_regression_tests.rs`
- `tests/seven_bugs_regression_tests.rs`

### Empty or stub files that must not count toward safety-net minimums
- `tests/main_panel_state_tests.rs` — currently empty
- `tests/gpui_wiring_command_routing_tests.rs` — currently empty
- `tests/gpui_chat_view_tests.rs` — currently empty
- `tests/chat_view_title_visibility_tests.rs` — specification stubs using `assert!(true)` and therefore zero-value as safety nets

### Other GPUI-relevant files to classify deliberately during execution
- `tests/model_selector_presenter_tests.rs` — presenter-level, not a direct GPUI view safety net
- `tests/regression_hardening_preserved_behaviors_tests.rs`
- `tests/gpui_integration_tests.rs`
- `tests/gpui_wiring_event_flow_tests.rs`

The plan must either replace weak/source-text assertions with behavioral equivalents, narrow them to legitimate static-contract assertions, or remove them deliberately with written justification.

## Coverage Strategy

The current workspace baseline is about `64.83%`, well below the enforced `80%` gate. This plan therefore cannot honestly define success as mere non-regression. Instead:
- every implementation batch must preserve or improve behavioral safety nets while shrinking structural debt
- `cargo coverage` must be run and recorded after each stable batch checkpoint
- newly extracted substantial GPUI modules must not be left effectively untested
- if a batch does not improve coverage materially, the evidence must explain why the structural work was still necessary and what follow-on coverage work remains
- no newly extracted GPUI file may be added to the coverage-ignore regex
- this plan does not lower, disable, or bypass the enforced `80%` gate
- a batch may be checkpoint-complete from a structural/refactoring perspective while still not PR-ready if the workspace remains below `80%`
- PR readiness requires the coverage gate to pass, whether by coverage gained within the batch itself or by explicitly sequenced follow-on coverage work before merge

## Required Outcomes

- GPUI view/component structural excludes are removed from CI.
- Each refactor batch begins with pre-existing or newly created behavioral safety-net tests that pass before extraction and after extraction.
- Refactored files and extracted files comply with the active structural thresholds.
- Re-exports and consumer imports remain stable or are deliberately updated.
- Source-text tests referencing moved code are explicitly remediated, not left to surprise-fail.
- `cargo coverage` is accounted for honestly and follow-on coverage work is explicit wherever the gate is not yet satisfied.

## Non-Goals

- Random splitting that preserves the same conceptual mess.
- Replacing one god-file with several slightly smaller god-files.
- Mock-heavy routing tests that prove only wiring.
- Source-text grep tests presented as behavioral coverage.
- A broad GPUI framework redesign.

## Preferred Delivery Strategy

This work is expected to be large. Preferred batching is one major GPUI family per implementation PR or stacked branch batch:
- Batch A: `chat_view` + `main_panel`
- Batch B: settings/editor family
- Batch C: remaining MCP/profile-selector/API-key/history cleanup

If execution instead stays in one branch, the plan still requires each phase to be independently verifiable before proceeding.

## Verification Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --tests
cargo coverage
python3 -m venv .venv-lizard
. .venv-lizard/bin/activate
python -m pip install --upgrade pip
python -m pip install lizard
python -m lizard -C 50 -L 100 -w src/ --exclude "src/main_gpui.rs" --exclude "src/bin/*" --exclude "src/services/chat.rs" --exclude "src/llm/client_agent.rs"
find src -name '*.rs' -print0 | xargs -0 wc -l | sort -n
```

## Quality Standard

The desired end state is code that a strict Rust reviewer would consider maintainable, coherent, and test-protected. The plan is not done when CI merely turns green once; it is done when the structural exemptions are gone, the decomposed modules have clear ownership, and remaining review feedback is only pedantic.
