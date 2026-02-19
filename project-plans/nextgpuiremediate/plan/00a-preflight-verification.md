# Phase 0.5: Preflight Verification

## Phase ID
`PLAN-20260219-NEXTGPUIREMEDIATE.P0.5`

## Purpose
Verify ALL assumptions before writing any code.

## Dependency Verification

| Dependency | cargo tree Output | Status |
|------------|-------------------|--------|
| gpui | `cargo tree -p gpui` | OK/MISSING |
| tokio | `cargo tree -p tokio` | OK/MISSING |
| flume | `cargo tree -p flume` | OK/MISSING |
| uuid | `cargo tree -p uuid` | OK/MISSING |

## Type/Interface Verification

| Type Name | Expected Definition | Actual Definition | Match? |
|-----------|---------------------|-------------------|--------|
| `EventBus` | Broadcast-backed app event bus | [from `src/events/bus.rs`] | YES/NO |
| `UserEvent` | Contains GPUI action variants incl. `SaveProfileEditor`, `McpAddNext`, `SaveMcp` | [from `src/events/types.rs`] | YES/NO |
| `ViewCommand` | Rich command surface beyond model search | [from `src/presentation/view_command.rs`] | YES/NO |
| `MainPanel::handle_command` | Central dispatch point | [from `src/ui_gpui/views/main_panel.rs`] | YES/NO |

## Call Path Verification

| Function | Expected Caller | Actual Caller | Evidence |
|----------|-----------------|---------------|----------|
| `spawn_user_event_forwarder` | `main_gpui.rs` startup | [grep output] | file:line |
| `GpuiBridge::drain_commands` | MainPanel render loop | [grep output] | file:line |
| `ModelSelectorPresenter::new` | `main_gpui.rs` startup | [grep output] | file:line |
| `SettingsPresenter::new` | `main_gpui.rs` startup | [grep output] | file:line |

## Test Infrastructure Verification

| Component | Test File Exists? | Test Patterns Work? |
|-----------|-------------------|---------------------|
| wiring tests | YES/NO | YES/NO |
| integration tests | YES/NO | YES/NO |

## Commands to Run

```bash
cargo tree -p gpui
cargo tree -p tokio
cargo tree -p flume
cargo tree -p uuid

grep -n "pub struct EventBus" src/events/bus.rs
grep -n "pub enum UserEvent" src/events/types.rs
grep -n "pub enum ViewCommand" src/presentation/view_command.rs
grep -n "fn handle_command" src/ui_gpui/views/main_panel.rs

grep -n "spawn_user_event_forwarder" src/main_gpui.rs
grep -n "drain_commands" src/ui_gpui/views/main_panel.rs
grep -n "ModelSelectorPresenter::new" src/main_gpui.rs
grep -n "SettingsPresenter::new" src/main_gpui.rs

cargo build --bin personal_agent_gpui
cargo test -- --list
```

## Blocking Issues Found

- [ ] [Issue 1]
- [ ] [Issue 2]

## Verification Gate

- [ ] All dependencies verified
- [ ] All types match expectations
- [ ] All call paths are possible
- [ ] Test infrastructure ready

IF ANY CHECKBOX IS UNCHECKED: STOP and update plan before proceeding.
