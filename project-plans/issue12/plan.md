# Issue 12 Plan: Themes + Settings Dropdown + Mac Native Option

## Goal

Implement GitHub issue #12 by replacing hardcoded GPUI theme colors with a file-driven theme system, adding theme selection in Settings, and adding a **Mac Native** option that follows the user’s macOS appearance/colors.

This plan assumes:
- Theme files are copied from `tmp/llxprt-code/packages/ui/themes` and become first-party assets in this repo.
- `green-screen` is no longer special-cased in code; it is loaded from its theme file like all other file-backed themes.
- Work is executed test-first (TDD) with separate implementation and verification subagents.

---

## Scope

### In Scope
- Copy theme JSON files from llxprt into this repo and own them here.
- Parse/load theme files into GPUI-consumable theme data.
- Refactor `src/ui_gpui/theme.rs` to runtime theme selection.
- Add Settings dropdown for theme selection.
- Persist selected theme using existing `AppSettingsService`.
- Add `mac-native` option that derives palette from macOS semantic colors / appearance.
- Add/extend UI automation tests and capture screenshots into workspace artifacts.
- Execute through PR creation, CI watch loops, CodeRabbit review/remediation loops.

### Out of Scope
- User-imported external theme files.
- Theme editor UI.
- Non-macOS native-theme implementation.

---

## Repository Touchpoints (Expected)

- `src/ui_gpui/theme.rs`
- `src/ui_gpui/views/settings_view/mod.rs`
- `src/ui_gpui/views/settings_view/render.rs`
- `src/ui_gpui/views/settings_view/command.rs`
- `src/events/types.rs`
- `src/presentation/view_command.rs`
- `src/main_gpui.rs`
- `src/main_gpui/startup.rs`
- `src/services/app_settings.rs`
- `src/services/app_settings_impl.rs`
- `assets/themes/*.json` (new, copied and owned)
- `tests/*theme*`
- `tests/ui_automation_tests.rs` (and/or new `tests/ui_theme_automation_tests.rs`)

---

## Subagent Execution Model

### Implementation Lane (Subagent: `rustcoder`)
- Owns RED->GREEN->REFACTOR implementation per phase.
- Produces code, tests, and artifacts.
- Runs required local verification commands before handing back.

### Verification Lane (Subagent: `rustreviewer`)
- Independent verification of behavior, architecture fit, and test sufficiency.
- Validates CI/workflow readiness and catches regressions.
- No rubber-stamp reviews; full clean review each pass.

### Orchestrator (Main agent)
- Sequences phases.
- Enforces TDD gates and checklists.
- Handles git flow, PR creation, CI loops, CodeRabbit remediation loop.

---

## TDD Phase Plan

## Phase 0 — Preflight + Baseline Contracts

### RED
- Add failing tests/checks that codify baseline expectations:
  - Theme assets location exists and is readable (`assets/themes`).
  - Existing `Theme` is currently hardcoded (documented baseline test/assertion).
  - Existing settings persistence roundtrip for `theme` is validated.

### GREEN
- Create issue workspace folders:
  - `project-plans/issue12/`
  - `artifacts/issue12/`
- Record baseline behavior and constraints in tests/docs.

### REFACTOR
- Normalize helper utilities for theme fixture loading in tests.

---

## Phase 1 — Theme Assets + Schema + Loader

### RED
Create failing tests first:
- `tests/theme_loader_tests.rs`
  - Loads all copied theme files from `assets/themes`.
  - Validates required fields (`name`, `slug`, `kind`, required color groups).
  - Confirms `green-screen` exists and parses.
  - Confirms malformed JSON yields explicit error.

### GREEN
- Copy theme files from `tmp/llxprt-code/packages/ui/themes/*.json` into `assets/themes/`.
- Implement loader/schema modules under `src/ui_gpui/theme/` or adjacent module structure consistent with repo style.
- Implement slug-indexed registry and list APIs.

### REFACTOR
- Extract shared parsing/validation helpers.
- Improve loader error messages with file path + field context.

---

## Phase 2 — Runtime Theme Engine Refactor (`theme.rs`)

### RED
Create failing tests first:
- `Theme` accessors return values from active theme palette (not constants).
- Switching active slug changes returned colors.
- Unknown slug falls back to default slug deterministically.
- `green-screen` rendered values come from JSON-backed palette.

### GREEN
- Refactor `src/ui_gpui/theme.rs` from static constants to runtime-backed accessors.
- Keep existing call-site API stable as much as possible (`Theme::bg_dark()`, etc.) to minimize churn.
- Add runtime theme state holder (thread-safe and GPUI-safe for current architecture).

### REFACTOR
- Reduce conversion duplication (hex -> GPUI colors).
- Keep token mapping explicit and documented.

---

## Phase 3 — Settings Dropdown + Event/Command Wiring

### RED
Create failing tests first:
- Settings view receives and renders theme list.
- Selecting theme emits expected user event.
- Presenter/command path persists theme and updates runtime theme state.
- Current theme is selected when Settings opens.

### GREEN
- Add theme dropdown to settings UI.
- Wire events/commands through existing architecture (`UserEvent`, `ViewCommand`, presenter/main panel wiring).
- Persist selected theme via `AppSettingsService::set_theme`.
- On startup, apply saved theme (`get_theme`) before first render.

### REFACTOR
- Keep settings view logic modular and consistent with current `settings_view` decomposition.
- Eliminate duplicated mapping code between view/presenter/runtime.

---

## Phase 4 — Mac Native Theme (`mac-native` option)

### RED
Create failing tests first:
- `mac-native` slug appears in selectable theme options.
- Appearance resolver maps light/dark modes to expected semantic token categories.
- If appearance query fails, fallback is deterministic and logged.
- Non-macOS compile path remains valid (cfg-gated fallback behavior).

### GREEN
- Implement macOS native theme resolver (cfg target macOS), using AppKit semantic/system colors and/or appearance detection.
- Add `mac-native` pseudo-theme option in registry/listing.
- When selected, resolve colors dynamically from OS appearance and apply.

### REFACTOR
- Isolate platform bridge behind small trait/module boundary for testability.
- Centralize fallback behavior and legacy `dark/light/auto` mapping.

---

## Phase 5 — Integration, Migration Mapping, and Regression Guards

### RED
Create failing tests first:
- Legacy settings values map correctly:
  - `dark` -> chosen default dark slug
  - `light` -> chosen default light slug
  - `auto` -> `mac-native`
- Theme persists across restart path.
- All GPUI views continue rendering with runtime theme accessors.

### GREEN
- Implement settings migration mapping in startup/settings loading path.
- Add integration tests for full select-save-restart-restore flow.

### REFACTOR
- Simplify migration map implementation and add explicit tests for edge values.

---

## Phase 6 — UI Automation + Screenshot Evidence

### RED
Create failing (ignored/manual) automation assertions first:
- Theme-switch scenario test expects screenshot artifacts to exist in workspace and be non-empty.
- Theme-switch scenario asserts config persisted slug changed.

### GREEN
Add/extend UI automation test(s):
- Reuse existing AppleScript helpers and GPUI launch env flags:
  - `PA_AUTO_OPEN_POPUP=1`
  - `PA_TEST_POPUP_ONSCREEN=1`
- Navigate to settings, switch themes, capture screenshots after each switch.

### Screenshot Artifact Paths (workspace-only)
- `artifacts/issue12/theme-default.png`
- `artifacts/issue12/theme-green-screen.png`
- `artifacts/issue12/theme-dracula.png`
- `artifacts/issue12/theme-mac-native-light.png`
- `artifacts/issue12/theme-mac-native-dark.png`

### REFACTOR
- Add deterministic helper for screenshot capture and path creation.
- Keep tests ignored by default if they require local Accessibility permissions.

---

## Required Verification Commands (Local)

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --lib --tests

python3 -m venv .venv-lizard
. .venv-lizard/bin/activate
python -m pip install --upgrade pip
python -m pip install lizard
python -m lizard -C 50 -L 100 -w src/
```

Optional focused runs during iteration:
```bash
cargo test --test ui_automation_tests -- --ignored --test-threads=1
cargo test --test ui_theme_automation_tests -- --ignored --test-threads=1
```

---

## Git + PR + CI + CodeRabbit Workflow

### 1) Branch and implementation flow
- Branch already created: `issue12`.
- Use atomic commits per phase.

### 2) Pre-commit review commands
```bash
git status
git diff HEAD
git log -n 3
```

### 3) PR creation
- Push branch.
- Create PR with title containing issue fix reference:
  - `Implement theme system + settings dropdown + mac-native option (Fixes #12)`

### 4) CI watch loop
- After PR creation, run:
```bash
gh pr checks NUM --watch --interval 300
```
- If checks are incomplete/failing, loop (max 5 loops), printing timestamp each loop.

### 5) CodeRabbit remediation loop
For every CodeRabbit issue:
- Evaluate correctness against source.
- Fix if valid.
- If invalid/out-of-scope, explain why with evidence.
- Add response comment describing action taken.
- Resolve issue when addressed/invalid.
- Re-run local verification suite and push.
- Repeat CI watch loop until all checks pass and review issues are resolved.

### 6) Completion gate
- All CI checks green.
- No unresolved actionable CodeRabbit issues.
- Theme screenshots and test evidence present in workspace artifacts.

---

## Risks and Mitigations

1. **Theme schema drift from llxprt source**
   - Mitigation: strict schema tests against all copied files.
2. **Token mismatch between JSON schema and GPUI token needs**
   - Mitigation: explicit mapping table + defaults + tests.
3. **Mac-native color extraction complexity**
   - Mitigation: small platform adapter + fallback path + cfg-gated tests.
4. **UI automation flakiness in headless/permission-constrained environments**
   - Mitigation: keep deterministic unit/integration tests as primary gate; keep automation tests ignored/manual with artifact checks.
5. **Regression risk from `Theme` API refactor across many call sites**
   - Mitigation: preserve method signatures and behavior contract tests.

---

## Open Questions to Confirm During Implementation

1. Canonical default dark slug (`default` vs another) for fallback.
2. Canonical light slug for `light` migration mapping.
3. Whether to auto-refresh `mac-native` on appearance change notification only, or also on app activation.
4. Whether to expose `mac-native` label as `Mac Native` or `System` in the dropdown.

---

## Definition of Done

- Theme JSON files are copied into repo-owned `assets/themes/` and fully parsed.
- `green-screen` is file-driven (no hardcoded special path).
- Settings screen supports theme selection and persistence.
- `mac-native` option follows macOS appearance/colors with safe fallback.
- Full local verification suite passes.
- UI automation theme scenario(s) and screenshots are produced in workspace artifacts.
- PR created, CI green, CodeRabbit issues remediated/resolved, ready for merge.
