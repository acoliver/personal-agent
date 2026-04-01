# Issue #34: Tool Approval Settings UI — Implementation Plan

## Status: IN PROGRESS

823 lines added across 9 files. Plumbing is ~70% complete. Does NOT compile (1 error + 4 warnings).

## What's Done (solid quality, follows project conventions)

1. **Domain** (`tool_approval_policy.rs`): `deny_persistently()`, `remove_persistent_allow_prefix()`, `remove_persistent_deny_prefix()` + 3 async tests. [OK]
2. **Events** (`events/types.rs`): 8 new `UserEvent` variants for all tool-approval actions. [OK]
3. **ViewCommands** (`view_command.rs`): `ToolApprovalPolicyUpdated`, `RefreshToolApprovalSettings`. [OK]
4. **Presenter** (`settings_presenter.rs`): Full load-save-emit cycle for 7 handlers + startup snapshot. [OK]
5. **SettingsView state/logic** (`settings_view/mod.rs`): `ActiveField` enum, 10 state fields, field helpers, `EntityInputHandler`, key handling. [OK]
6. **Chat YOLO wiring** (`render_bars.rs`): Emits `SetToolApprovalYoloMode` instead of local toggle. [OK]
7. **MainPanel startup** (`startup.rs`): Emits `RefreshToolApprovalPolicy`. [OK]

## Remaining Work

### A. Fix compile error (settings_view/command.rs line 85)
- `YoloModeChanged { enabled }` → `YoloModeChanged { active }` (field name mismatch)

### B. Fix unused imports
- `settings_view/mod.rs`: Remove `AppContext`
- `settings_view/render.rs`: Imports will be consumed by step C

### C. Render tool-approval UI section (settings_view/render.rs)
The main visible feature — currently 0% done (only imports added):
- `render_tool_approval_section()` containing:
  - YOLO mode toggle (checkbox + warning text)
  - Auto-approve read-only tools toggle (checkbox)
  - MCP approval mode selector (Per Tool / Per Server radio-style)
  - Allowlist: scrollable list with [x] remove, text input + [Add] button
  - Denylist: same pattern as allowlist
  - Status message bar (error/success)
- IME canvas registration in main `render()` method
- Add `.child(self.render_tool_approval_section(cx))` to main render

### D. MainPanel command routing (command.rs + routing.rs)
- `command.rs`: Route `ToolApprovalPolicyUpdated` → settings_view, `YoloModeChanged` → settings_view + chat_view
- `routing.rs`: Add `tool_approval_policy_count` and `yolo_mode_changed_count` counters + match arms

### E. Tests
- Fix `start_runtime_requires_popup_window_before_emitting_refreshes` (expects only 2 `RefreshApiKeys`, now there's also `RefreshToolApprovalPolicy`)
- Add routing test for `ToolApprovalPolicyUpdated` counter
- Add routing test for `YoloModeChanged` counter
- Add `#[gpui::test]` for `handle_command` forwarding `ToolApprovalPolicyUpdated` to settings_view

### F. Verification
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --lib --tests`

### G. Commit, push, open PR with `Fixes #34`
### H. Watch CI + CodeRabbit, remediate until green
