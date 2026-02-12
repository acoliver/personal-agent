# Phase 01: Preflight Verification

## Phase ID

`PLAN-20250128-GPUI.P01`

## Prerequisites

- None (first phase)

## Purpose

Verify ALL assumptions before writing any code:
1. GPUI can be added as a dependency
2. `flume` can be added for cross-runtime channels
3. Existing presenter/event architecture is intact
4. NSStatusItem code can be reused
5. ExactoBar patterns are applicable

---

## Verification Tasks

### 1. GPUI Dependency Test

Add GPUI to Cargo.toml and verify it compiles:

```bash
# In Cargo.toml, add:
# gpui = { git = "https://github.com/zed-industries/zed", branch = "main", package = "gpui" }

# Then run:
cargo check
```

**Expected:** Compiles (may have warnings, but no errors)
**If fails:** Document the error, may need to pin specific commit

### 2. flume Dependency Test

Add flume for runtime-agnostic channels:

```bash
# In Cargo.toml, add:
# flume = "0.11"

# Then run:
cargo check
```

**Expected:** Compiles without issues

### 3. Existing Architecture Verification

Verify presenter layer exists and is functional:

```bash
# Check presenters exist
ls -la src/presentation/

# Check EventBus exists
ls -la src/events/

# Check services exist  
ls -la src/services/

# Verify current build still works
cargo build
cargo test --lib
```

**Expected:**
- `src/presentation/chat_presenter.rs` exists
- `src/presentation/history_presenter.rs` exists
- `src/presentation/settings_presenter.rs` exists
- `src/events/bus.rs` exists
- `src/services/` has service implementations
- Build and tests pass

### 4. NSStatusItem Code Location

Identify code that will be reused for tray integration:

```bash
# Find NSStatusItem setup
grep -rn "NSStatusItem\|NSStatusBar" src/ --include="*.rs"

# Find icon loading
grep -rn "MenuBarIcon\|load_image" src/ --include="*.rs"

# Find popover toggle
grep -rn "togglePopover" src/ --include="*.rs"
```

**Expected:**
- `src/main_menubar.rs` contains NSStatusItem setup
- Icon path: `assets/MenuBarIcon.imageset/icon-32.png`
- Toggle function exists for show/hide

### 5. ExactoBar Pattern Applicability

Verify ExactoBar research is available:

```bash
# Check ExactoBar was cloned
ls -la research/exactobar/

# Key files to reference
ls research/exactobar/exactobar-app/src/tray.rs
ls research/exactobar/exactobar-app/src/menu/
ls research/exactobar/exactobar-app/src/components/
```

**Expected:**
- `research/exactobar/` exists with source code
- Key patterns available: tray.rs, components/, menu/

### 6. ViewCommand/UserEvent Types

Verify event types are defined and usable:

```bash
# Check ViewCommand enum
grep -A 50 "pub enum ViewCommand" src/presentation/view_command.rs | head -60

# Check UserEvent enum
grep -A 50 "pub enum UserEvent" src/events/types.rs | head -60

# Check ChatEvent enum
grep -A 30 "pub enum ChatEvent" src/events/types.rs | head -40
```

**Expected:**
- ViewCommand has variants for UI updates (AppendStream, ShowThinking, ShowError, etc.)
- UserEvent has variants for user actions (SendMessage, NewConversation, etc.)
- ChatEvent has streaming variants (TextDelta, ThinkingDelta, etc.)

### 7. Theme Values

Extract current theme values for GPUI parity:

```bash
# Get current theme colors
cat src/ui/theme.rs
```

**Expected:**
- Background colors defined (BG_DARKEST, BG_DARKER, etc.)
- Text colors defined (TEXT_PRIMARY, TEXT_SECONDARY, etc.)
- Values in RGB tuple format

---

## Deliverables

Create evidence file: `project-plans/gpui-migration/plan/.completed/P01.md`

Contents:
```markdown
# Phase 01: Preflight Verification Evidence

## GPUI Dependency
- Added to Cargo.toml: [YES/NO]
- cargo check result: [PASS/FAIL]
- If failed, error: [paste error]

## flume Dependency
- Added to Cargo.toml: [YES/NO]
- cargo check result: [PASS/FAIL]

## Existing Architecture
- Presenters exist: [YES/NO with list]
- EventBus exists: [YES/NO]
- Services exist: [YES/NO with list]
- cargo build: [PASS/FAIL]
- cargo test: [PASS/FAIL with count]

## NSStatusItem Code
- Location: [file:line]
- Icon path: [path]
- Toggle function: [file:line]

## ExactoBar Reference
- Directory exists: [YES/NO]
- Key files present: [list]

## Event Types
- ViewCommand variants: [count and key examples]
- UserEvent variants: [count and key examples]
- ChatEvent variants: [count and key examples]

## Theme Values
- Background colors: [list with hex/RGB]
- Text colors: [list with hex/RGB]

## Blocking Issues
[List any issues that must be resolved before proceeding, or "None"]

## Verdict
[PASS if all checks pass, FAIL if any blocking issues]
```

---

## Success Criteria

- [ ] GPUI dependency can be added (compiles)
- [ ] `flume` dependency can be added (compiles)
- [ ] Existing presenter architecture intact
- [ ] NSStatusItem code identified and reusable
- [ ] ExactoBar reference available
- [ ] Event types defined and suitable (ViewCommand, UserEvent, ChatEvent)
- [ ] Theme values documented

**If any check fails with blocking issue: STOP and resolve before proceeding.**

---

## Failure Recovery

If GPUI dependency fails:
1. Try pinning to specific zed commit
2. Check for missing system dependencies
3. Document macOS version requirements

If architecture incomplete:
1. Complete presenter wiring first (PLAN-20250128-PRESENTERS)
2. Document what's missing

---

## Next Phase

After P01 completes with PASS:
→ P01a: Preflight Verification Checklist (coordinator validates evidence)
