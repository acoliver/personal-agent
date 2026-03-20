# Old UI excision checklist

Generated: 2026-03-20

## What we are removing

This excision should remove **all non-GPUI UI paths**, not just `src/ui/`.

That includes:

### Old AppKit UI
- `src/ui/`
- `src/main_menubar.rs`
- `personal_agent_menubar` from `Cargo.toml`

### Old egui/eframe path
- `src/main.rs`
- `src/main_debug.rs`
- `src/main_utils.rs`
- `src/popover.rs`
- `personal_agent` binary entry from `Cargo.toml`
- now-unused `eframe` / `egui` / related dependencies, if proven dead

## Important clarification

`src/ui/` is **not** a TUI. It is an older native macOS AppKit UI. Any comments/docs calling it a "legacy TUI" should be corrected or deleted as part of this cleanup.

## Why this can stay lightweight

This does **not** need a giant multi-phase architecture plan.

This is mostly an excision task:
- identify all old UI roots
- delete them cleanly
- remove stale binary entries and dependencies
- update docs/tooling
- verify GPUI still builds and works

The only real caution is avoiding accidental deletion of macOS tray/window code still used by GPUI.

## Rip-out checklist

### 1. Remove old UI binary roots
- [ ] delete `src/main_menubar.rs`
- [ ] delete `src/main.rs`
- [ ] delete `src/main_debug.rs`
- [ ] remove `personal_agent_menubar` binary from `Cargo.toml`
- [ ] remove `personal_agent` binary from `Cargo.toml`

## 2. Remove old UI implementation trees and helpers
- [ ] delete `src/ui/`
- [ ] delete `src/main_utils.rs`
- [ ] delete `src/popover.rs`

## 3. Remove dead dependencies after code deletion
Audit and remove if unused after the excision:
- [ ] `eframe`
- [ ] `egui`
- [ ] `tray-icon`
- [ ] `image`
- [ ] `raw-window-handle`
- [ ] any other dependency only used by removed UI paths

Do **not** remove macOS dependencies still needed by GPUI/tray integration, including anything still referenced by `src/main_gpui.rs` or `src/ui_gpui/`.

## 4. Clean docs and repo messaging
- [ ] update `README.md` to document GPUI as the only UI path
- [ ] remove references to `personal_agent_menubar`
- [ ] remove references to `personal_agent` if that binary is deleted
- [ ] remove/update project structure references to `src/ui/`
- [ ] remove/update any text implying GPUI is merely optional if that is no longer true
- [ ] remove or correct any "legacy TUI" wording

## 5. Clean quality script and tooling references
Update `scripts/check-quality.sh`:
- [ ] remove `src/ui/` exclusions
- [ ] remove `src/main_menubar.rs` exclusions
- [ ] remove `src/popover.rs` exclusions if file is deleted
- [ ] remove stale commentary about legacy TUI code
- [ ] re-evaluate whether GPUI exclusions are still justified once it is the only UI

## 6. Clean stale root-level summary docs if they only describe removed UI work
Likely candidates to review/remove:
- [ ] `CHANGES_MCP_CONFIGURE.md`
- [ ] `MCP_AUTH_FIX_SUMMARY.md`
- [ ] `OAUTH_UI_IMPLEMENTATION.md`
- [ ] `SETTINGS_REDESIGN_SUMMARY.md`
- [ ] `SMITHERY_OAUTH_IMPLEMENTATION.md`

These should only stay if they still describe surviving code.

## Guardrails

Before deletion, quickly confirm GPUI covers the user-visible flows we care about:
- [ ] chat works
- [ ] settings works
- [ ] history works
- [ ] model selector works
- [ ] MCP add/configure works
- [ ] tray popup works

During deletion:
- [ ] do not touch `src/main_gpui.rs` unless needed for cleanup
- [ ] do not remove objc2/AppKit crates still used by GPUI tray/window code
- [ ] prefer deleting roots first, then unused code, then dependencies

## Verification

After the excision:
- [ ] `cargo build --bin personal_agent_gpui`
- [ ] `cargo test`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo fmt -- --check`
- [ ] `scripts/check-quality.sh`

Repo-state checks:
- [ ] no `src/ui/` directory remains
- [ ] no `src/main_menubar.rs` remains
- [ ] no `src/main.rs` remains if the egui path is removed
- [ ] no `src/main_debug.rs` remains
- [ ] no `src/main_utils.rs` remains
- [ ] no `src/popover.rs` remains
- [ ] no `personal_agent_menubar` entry remains in `Cargo.toml`
- [ ] no `personal_agent` binary entry remains in `Cargo.toml` unless intentionally retained
- [ ] repo search for `legacy TUI` returns no inaccurate references
- [ ] repo search for `personal_agent_menubar` only finds historical artifacts if any

## Exit condition

This cleanup is done when the repository has one supported UI stack:
- GPUI

and all older AppKit + egui/eframe UI codepaths, docs, and dead dependencies are removed cleanly.
