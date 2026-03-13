# Phase 00a: Preflight Verification

## Phase ID

`PLAN-20260304-GPUIREMEDIATE.P00a`

## Purpose

Verify the diagnosed failure, code paths, test inventory, and verification tooling assumptions before implementation planning begins.

## Dependency / Tooling Verification

| Item | Verification Command | Expected Result |
|------|----------------------|-----------------|
| Rust formatting | `cargo fmt --all --check` | command available; formatting status known |
| Rust typecheck | `cargo check -q` | command available; current baseline known |
| Clippy | `cargo clippy --all-targets -- -D warnings` | command available; baseline known |
| Quality helper baseline | `bash scripts/check-quality.sh` | helper runs and baseline pass/fail status is recorded explicitly before implementation |

## Type / Interface Verification

| Item | Expectation | Evidence Command |
|------|-------------|------------------|
| `build_startup_view_commands` | startup bootstrap path exists | `grep -n "fn build_startup_view_commands" src/main_gpui.rs` |
| `MainPanelAppState.startup_commands` | startup commands stored globally | `grep -n "startup_commands" src/ui_gpui/views/main_panel.rs` |
| `MainPanel::apply_startup_commands` | synchronous startup application exists | `grep -n "fn apply_startup_commands" src/ui_gpui/views/main_panel.rs` |
| `MainPanel::ensure_bridge_polling` | popup-owned command polling exists | `grep -n "fn ensure_bridge_polling" src/ui_gpui/views/main_panel.rs` |
| `ChatPresenter::handle_select_conversation` | presenter replays selected transcript | `grep -n "handle_select_conversation" src/presentation/chat_presenter.rs` |
| `ConversationMessagesLoaded` | bulk transcript replacement command exists | `grep -n "ConversationMessagesLoaded" src/presentation/view_command.rs src/ui_gpui/views/chat_view.rs src/ui_gpui/views/main_panel.rs` |
| `ConversationActivated` shape | current protocol shape is known before migration | `grep -n "ConversationActivated" src/presentation/view_command.rs src/presentation/chat_presenter.rs` |
| transcript payload type | concrete transcript payload/storage boundary is known before store pseudocode is implemented | `grep -n "ConversationMessagesLoaded\|ConversationMessagePayload" src/presentation/view_command.rs src/presentation/chat_presenter.rs src/ui_gpui/views/chat_view.rs src/main_gpui.rs` |
| `ViewCommand` serialization | protocol migration burden is grounded in the real serialized contract | `grep -n "derive(Debug, Clone, PartialEq, Serialize, Deserialize)" src/presentation/view_command.rs` |
| bridge emit failure semantics | selection transport failure handling is grounded in current code | `grep -n "pub fn emit\|try_send" src/ui_gpui/bridge/gpui_bridge.rs src/ui_gpui/bridge/view_command_sink.rs` |
| bridge drain ownership baseline | current and target drainer callsites are inventoried before migration | `grep -rn "drain_commands" src tests --include="*.rs"` |
| variant/match inventory | all protocol construction and match sites are inventoried before migration | `grep -rn -l "ConversationActivated\|ConversationMessagesLoaded" src tests --include="*.rs"` |
| `src/ui_gpui/mod.rs` export | app-store module registration need is known before Phase 04 | `grep -n "app_store" src/ui_gpui/mod.rs || true` |

## Call Path Verification

| Call Path | Why It Matters | Evidence Command |
|-----------|----------------|------------------|
| `ChatView` / `HistoryView` emit `SelectConversation` | proves manual selection enters presenter path | `grep -n "SelectConversation" src/ui_gpui/views/chat_view.rs src/ui_gpui/views/history_view.rs` |
| `ChatPresenter` emits `ConversationActivated` then `ConversationMessagesLoaded` | proves data replay already exists | `grep -n "ConversationActivated\|ConversationMessagesLoaded" src/presentation/chat_presenter.rs` |
| `ChatView` filters transcript replay by active conversation state | proves ordering/state-ownership seam matters | `grep -n "ConversationMessagesLoaded\|active_conversation_id" src/ui_gpui/views/chat_view.rs` |
| `MainPanel` forwards transcript replay into mounted chat view | proves popup forwarding is in current delivery path | `grep -n "ConversationMessagesLoaded\|handle_command\|chat_view" src/ui_gpui/views/main_panel.rs` |


## Test Infrastructure Verification

| Test Target | Verification Command | Expected Result |
|-------------|----------------------|-----------------|
| presenter/settings tests | `cargo test --test presenter_selection_and_settings_tests -- --list` | test binary exists |
| regression tests | `cargo test --test seven_bugs_regression_tests -- --list` | test binary exists |
| startup/layout tests | `cargo test --test chat_startup_scrollback_layout_regression_tests -- --list` | test binary exists |
| conversation switch tests | `cargo test --test chat_view_conversation_switch_regression_tests -- --list` | test binary exists |
| helper/client tests | `cargo test --test llm_client_helpers_tests -- --list` | test binary exists |
| GPUI integration tests | `cargo test --test gpui_integration_tests -- --list` | test binary exists |
| GPUI bridge tests | `cargo test --test gpui_bridge_tests -- --list` | test binary exists |
| GPUI chat view tests | `cargo test --test gpui_chat_view_tests -- --list` | test binary exists |
| GPUI components tests | `cargo test --test gpui_components_tests -- --list` | test binary exists |
| GPUI wiring event-flow tests | `cargo test --test gpui_wiring_event_flow_tests -- --list` | test binary exists |
| GPUI wiring command-routing tests | `cargo test --test gpui_wiring_command_routing_tests -- --list` | test binary exists |
| provider quirks tests | `cargo test --test kimi_provider_quirks_integration_tests -- --list` | test binary exists |

## Commands To Run

```bash
grep -n "fn build_startup_view_commands" src/main_gpui.rs
grep -n "startup_commands" src/ui_gpui/views/main_panel.rs
grep -n "fn apply_startup_commands" src/ui_gpui/views/main_panel.rs
grep -n "fn ensure_bridge_polling" src/ui_gpui/views/main_panel.rs
grep -n "SelectConversation" src/ui_gpui/views/chat_view.rs src/ui_gpui/views/history_view.rs
grep -n "handle_select_conversation" src/presentation/chat_presenter.rs
grep -n "ConversationActivated\|ConversationMessagesLoaded" src/presentation/chat_presenter.rs
grep -n "ConversationMessagesLoaded\|active_conversation_id\|handle_command\|chat_view" src/presentation/view_command.rs src/ui_gpui/views/chat_view.rs src/ui_gpui/views/main_panel.rs
grep -n "ConversationActivated" src/presentation/view_command.rs src/presentation/chat_presenter.rs
grep -n "ConversationMessagesLoaded\|ConversationMessagePayload" src/presentation/view_command.rs src/presentation/chat_presenter.rs src/ui_gpui/views/chat_view.rs src/main_gpui.rs
grep -n "derive(Debug, Clone, PartialEq, Serialize, Deserialize)" src/presentation/view_command.rs
grep -n "pub fn emit\|try_send" src/ui_gpui/bridge/gpui_bridge.rs src/ui_gpui/bridge/view_command_sink.rs
grep -rn "drain_commands" src tests --include="*.rs"
grep -rn -l "ConversationActivated\|ConversationMessagesLoaded" src tests --include="*.rs"
grep -n "app_store" src/ui_gpui/mod.rs || true

bash scripts/check-quality.sh
cargo test --test presenter_selection_and_settings_tests -- --list
cargo test --test seven_bugs_regression_tests -- --list
cargo test --test chat_startup_scrollback_layout_regression_tests -- --list
cargo test --test chat_view_conversation_switch_regression_tests -- --list
cargo test --test llm_client_helpers_tests -- --list
cargo test --test gpui_integration_tests -- --list
cargo test --test gpui_bridge_tests -- --list
cargo test --test gpui_chat_view_tests -- --list
cargo test --test gpui_components_tests -- --list
cargo test --test gpui_wiring_event_flow_tests -- --list
cargo test --test gpui_wiring_command_routing_tests -- --list
cargo test --test kimi_provider_quirks_integration_tests -- --list
```

## Quality Helper Baseline Recording

- Record the actual exit status and any notable output from `bash scripts/check-quality.sh` in the Phase 00a evidence artifact.
- Decision tree:
  1. if the helper passes cleanly on the untouched baseline, record PASS evidence and proceed;
  2. if it fails on issues inside this plan's execution scope, stop and remediate those issues before proceeding;
  3. if it fails on clearly out-of-scope baseline issues, create an explicit, evidence-backed baseline exception rule in the Phase 00a evidence artifact before any implementation phase begins.
- A valid baseline exception rule must include:
  - exact command run and exit status
  - exact representative failing file:line evidence from helper output
  - explicit statement that the failures are outside this plan's execution scope at preflight time
  - a no-regression contract for later phases: implementation may not introduce new `scripts/check-quality.sh` failures inside files touched by this plan, and final evidence must show either full helper green or file-scoped proof that touched recovery files did not add to the baseline exception set
- If any named test target or verification command shape discovered in Phase 00a differs from downstream phase assumptions, Phase 00a must update every downstream phase command that depends on that target before implementation begins.
- Later phases may not hand-wave a failing quality helper as pre-existing; they must either satisfy the helper fully or prove no regression beyond the explicit Phase 00a baseline exception scope.

## Blocking Issues Found

- [ ] Any cited code path missing or materially different from diagnosis
- [ ] Any required test target absent or renamed
- [ ] Transcript payload/match-site inventory materially differs from plan assumptions
- [ ] `scripts/check-quality.sh` unavailable, non-executable, or not runnable
- [ ] `scripts/check-quality.sh` fails on the baseline without a completed evidence-backed exception rule or in-scope remediation

## Verification Gate

- [ ] Startup/bootstrap path verified
- [ ] Runtime presenter path verified
- [ ] Failure seam confirmed by file evidence
- [ ] Required test targets confirmed
- [ ] Quality helper baseline recorded and either passing or explicitly exception-scoped with a no-regression contract

If any checkbox is unchecked: stop and update the plan before implementation.
