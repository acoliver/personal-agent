# Phase 01 Artifact: Public API and Static-Test Remediation Plan

## Public API / re-export inventory

Current exports from `src/ui_gpui/views/mod.rs` that execution must protect or update deliberately:
- `ApiKeyManagerView`
- `ChatState`, `ChatView`
- `ConversationItem`, `HistoryState`, `HistoryView`
- `MainPanel`
- `McpAddState`, `McpAddView`, `McpRegistry`, `McpSearchResult`, `SearchState`
- `ConfigField`, `McpAuthMethod`, `McpConfigureData`, `McpConfigureState`, `McpConfigureView`, `OAuthStatus`
- `ModelInfo`, `ModelSelectorState`, `ModelSelectorView`, `ProviderInfo`
- `ApiType`, `AuthMethod`, `ProfileEditorData`, `ProfileEditorState`, `ProfileEditorView`
- `McpItem`, `McpStatus`, `ProfileItem`, `SettingsState`, `SettingsView`

## Re-export rule

When internals are extracted into submodules, prefer this pattern:
- keep the original top-level module file (`chat_view.rs`, `settings_view.rs`, etc.) as the stable public entry point
- re-export moved types/functions from that top-level module where needed
- only force downstream import changes if there is a clear maintainability win and all consumers are updated in the same batch

## Source-text test inventory to remediate

These tests currently reference GPUI source files via `include_str!()` and cannot be ignored during decomposition:
- `tests/model_profile_flow_regression_tests.rs`
- `tests/chat_startup_scrollback_layout_regression_tests.rs`
- `tests/gpui_popup_independence_tests.rs`
- `tests/api_key_manager_ui_regression_tests.rs`
- `tests/chat_view_conversation_switch_regression_tests.rs`
- `tests/seven_bugs_regression_tests.rs`

## Pre-classified expected dispositions

### Replace with behavioral tests
These are implementation-text assertions that should become behavior tests wherever feasible:
- `api_key_manager_ui_regression_tests.rs`
  - input handler registration/editability assertions
  - Cmd+V paste support assertion
  - mask toggle assertion
  - edit-mode tab behavior assertion
- `chat_view_conversation_switch_regression_tests.rs`
  - transcript replacement ordering assertions
  - selection path assertions tied to source ordering or source snippets
- `seven_bugs_regression_tests.rs`
  - bug 2 chat dropdown overlay source assertions
  - bug 3 profile dropdown root-overlay source assertions
  - bug 5 finalize-stream source assertion
  - bug 6 history/chat source assertions
  - bug 7 model-editable source assertion

### Narrow to legitimate static-contract assertions only if still valuable
These can remain static only if rewritten to assert a stable public/module contract rather than source text placement:
- `chat_startup_scrollback_layout_regression_tests.rs`
  - `ChatView` export/type visibility assertion may remain
  - layout-specific `min_h_0` source-text assertions should normally become behavioral render/state tests if feasible
- `gpui_popup_independence_tests.rs`
  - popup independence should ideally become a behavior/integration assertion; if any static assertion remains, it must target a stable architectural contract rather than raw source fragments
- `model_profile_flow_regression_tests.rs`
  - source assertions about shared provider defaults / keychain behavior should be replaced by behavioral or direct functional tests where possible; only keep static assertions if the contract is truly architectural and not merely implementation text

### Remove with written justification if no worthwhile behavioral/static contract exists
- any leftover `include_str!()` assertion whose only value is proving a specific literal, helper name, or implementation ordering that is not itself a user-visible or architecture-stable contract

## Remediation rule for each source-text test

For each affected assertion, execution must record one of these dispositions in phase evidence:
1. replaced with a behavioral test proving the same user-visible or state-visible contract
2. narrowed to a legitimate static contract that still belongs in the surviving module path
3. removed with explicit justification because it only asserted implementation text and had no valid contract worth preserving

## Out-of-scope note

`include_str!()` assertions against `src/main_gpui.rs` are outside this plan's source-text-remediation scope. Track them separately if they need cleanup.

## Forbidden disposition

- leaving a source-text assertion broken and simply updating the path without confirming the assertion still reflects a valid contract
- changing only the `include_str!()` path with no explanation of the surviving contract
