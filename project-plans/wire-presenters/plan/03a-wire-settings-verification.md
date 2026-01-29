# Phase 03a: Verify SettingsPresenter Event Wiring

**Phase ID**: P03a
**Type**: Verification
**Status**: Pending
**Prerequisites**: P03 completion marker exists

## Objective

Verify that SettingsPresenter correctly subscribes to event bus and handles all required profile, MCP, and system events per dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md section "Presentation Layer Isolation".

## Verification Protocol

Per dev-docs/COORDINATING.md, this is a SKEPTICAL AUDITOR phase. Assume nothing works until you have EVIDENCE it works.

## Prerequisite Check

Before running verification, confirm:

```bash
# Does P03 completion marker exist?
ls project-plans/wire-presenters/plan/.completed/P03.md

# Does P02a have PASS verdict?
grep "^## Verdict: PASS" project-plans/wire-presenters/plan/.completed/P02A.md

# Does evidence show no placeholders?
grep "unimplemented!" project-plans/wire-presenters/plan/.completed/P02A.md
# Expected: (no output)
```

If any check fails: DO NOT PROCEED. Remediate failed phase first.

## Structural Checks

### 1. Files Exist

```bash
# Check that SettingsPresenter was modified
git diff --stat src/presentation/settings_presenter.rs
# Should show meaningful changes
```

Expected: SettingsPresenter has event subscription code

### 2. Markers Present

```bash
# Check for plan markers
grep -c "@plan PLAN-20250128-PRESENTERS.P03" src/presentation/settings_presenter.rs
# Expected: >= 1
```

### 3. Build Verification

```bash
cargo build --all-targets 2>&1 | tail -20
# Expected: exit code 0, "Finished" message
```

## Placeholder Detection (MANDATORY)

Run ALL of these commands and record EXACT output:

```bash
# Check 1: unimplemented! macro
grep -rn "unimplemented!" src/presentation/settings_presenter.rs
# Expected: (no output)

# Check 2: todo! macro
grep -rn "todo!" src/presentation/settings_presenter.rs
# Expected: (no output)

# Check 3: TODO/FIXME comments
grep -rn "// TODO\|// FIXME\|// HACK\|// STUB" src/presentation/settings_presenter.rs
# Expected: (no output)

# Check 4: Placeholder strings
grep -rn "placeholder\|not yet implemented\|will be implemented" src/presentation/settings_presenter.rs
# Expected: (no output)
```

**IF ANY GREP RETURNS MATCHES: STOP. VERDICT IS FAIL.**

## Semantic Verification

### 1. Event Subscription Code

```bash
# Check that EventBus subscription exists
grep -A 5 "subscribe_to_events\|event_bus.subscribe" src/presentation/settings_presenter.rs
# Expected: Multiple subscriptions for UserEvent, ProfileEvent, McpEvent, SystemEvent
```

What to verify:
- [ ] SettingsPresenter has `event_bus` field
- [ ] Constructor calls `subscribe_to_events()`
- [ ] Subscription filters for UserEvent (profile and MCP actions)
- [ ] Subscription filters for all ProfileEvent variants
- [ ] Subscription filters for all McpEvent variants
- [ ] Subscription filters for SystemEvent::ConfigLoaded, ConfigSaved, ModelsRegistryRefreshed

### 2. Event Handler Implementation

```bash
# Check EventHandler trait implementation
grep -A 3 "impl EventHandler for SettingsPresenter" src/presentation/settings_presenter.rs
# Expected: Match statement on AppEvent
```

What to verify:
- [ ] `impl EventHandler for SettingsPresenter` exists
- [ ] `handle_event` method exists
- [ ] Match statement handles all three event categories (Profile, Mcp, System)
- [ ] Each handler calls a private method

### 3. ViewCommand Emission

```bash
# Check that handlers emit ViewCommands
grep -c "emit_view_command\|ViewCommand::" src/presentation/settings_presenter.rs
# Expected: >= 10 (one per event handler category)
```

What to verify:
- [ ] Each event handler emits a ViewCommand
- [ ] ViewCommand variants match event types
- [ ] No direct service calls in handlers

### 4. Profile Event Handlers

```bash
# Check for profile handlers
grep -c "fn on_profile_created\|fn on_profile_updated\|fn on_profile_deleted" src/presentation/settings_presenter.rs
# Expected: >= 3

grep -c "fn on_default_profile_changed\|fn on_profile_test_started\|fn on_profile_test_completed" src/presentation/settings_presenter.rs
# Expected: >= 3

grep -c "fn on_select_profile\|fn on_edit_profile\|fn on_delete_profile" src/presentation/settings_presenter.rs
# Expected: >= 3
```

Expected: At least 9 profile-related handlers

### 5. MCP Event Handlers

```bash
# Check for MCP handlers
grep -c "fn on_mcp_starting\|fn on_mcp_started\|fn on_mcp_start_failed" src/presentation/settings_presenter.rs
# Expected: >= 3

grep -c "fn on_mcp_stopped\|fn on_mcp_unhealthy\|fn on_mcp_recovered" src/presentation/settings_presenter.rs
# Expected: >= 3

grep -c "fn on_toggle_mcp\|fn on_configure_mcp\|fn on_save_mcp_config" src/presentation/settings_presenter.rs
# Expected: >= 3
```

Expected: At least 9 MCP-related handlers

### 6. System Event Handlers

```bash
# Check for system handlers
grep -c "fn on_config_loaded\|fn on_config_saved\|fn on_models_refreshed" src/presentation/settings_presenter.rs
# Expected: >= 3
```

## Code Tracing

Pick ONE event from each category and trace the full flow:

**Trace 1: Profile Event**
`AppEvent::Profile(ProfileEvent::TestCompleted { id, success, response_time_ms, error })`

1. Read the event handler:
```bash
sed -n '/ProfileEvent::TestCompleted/,/^[[:space:]]*}/p' src/presentation/settings_presenter.rs
```

2. Verify:
- [ ] Handler extracts all fields from event
- [ ] Handler constructs appropriate ViewCommand (success or error)
- [ ] Handler calls `emit_view_command(cmd)`

**Trace 2: MCP Event**
`AppEvent::Mcp(McpEvent::Unhealthy { id, name, error })`

1. Read the event handler:
```bash
sed -n '/McpEvent::Unhealthy/,/^[[:space:]]*}/p' src/presentation/settings_presenter.rs
```

2. Verify:
- [ ] Handler extracts error from event
- [ ] Handler constructs ViewCommand::ShowMcpError or similar
- [ ] Handler calls `emit_view_command(cmd)`

## Comparison to Previous Presenters

```bash
# Verify similar pattern to ChatPresenter and HistoryPresenter
grep -c "impl EventHandler" src/presentation/settings_presenter.rs
# Expected: >= 1 (same pattern as other presenters)

# Check for similar field names
grep -c "event_bus:\|Arc<EventBus>" src/presentation/settings_presenter.rs
# Expected: >= 1
```

## Inputs

### Files to Read
- `src/presentation/settings_presenter.rs` - Full implementation
- `project-plans/wire-presenters/plan/.completed/P03.md` - Phase completion evidence
- `src/events/types.rs` - ProfileEvent, McpEvent, SystemEvent definitions
- `project-plans/wire-presenters/plan/.completed/P01A.md` - Reference implementation

### Evidence Required
- Exact grep outputs from all checks above
- File:line references for event subscription
- Function names for all event handlers (profile, MCP, system)
- At least two full code traces (one profile, one MCP)

## Outputs

### Evidence File
Create: `project-plans/wire-presenters/plan/.completed/P03A.md`

Must contain:

```markdown
# Phase 03a Verification Results

## Verdict: [PASS|FAIL]

## Structural Checks
- Files exist: [YES/NO with git diff --stat output]
- Markers present: [YES/NO with grep count]
- Build: [PASS/FAIL with output]

## Placeholder Detection
Command: grep -rn "unimplemented!" src/presentation/settings_presenter.rs
Output: [exact output or "no matches"]

Command: grep -rn "todo!" src/presentation/settings_presenter.rs
Output: [exact output or "no matches"]

Command: grep -rn "// TODO\|// FIXME" src/presentation/settings_presenter.rs
Output: [exact output or "no matches"]

Command: grep -rn "placeholder" src/presentation/settings_presenter.rs
Output: [exact output or "no matches"]

## Semantic Verification
- Event subscription exists: [YES/NO with grep output]
- EventHandler trait implemented: [YES/NO with grep output]
- ViewCommands emitted: [YES/NO with count]
- Profile handlers: [count and list]
- MCP handlers: [count and list]
- System handlers: [count and list]

## Code Trace Examples
[Pick one profile event and one MCP event, show full flow for each]

## Evidence Summary
[Bullet points of what was verified]

## Blocking Issues (if FAIL)
[List exactly what must be fixed]
```

## PASS/FAIL Criteria

### PASS Conditions
- Exit code 0 from `cargo build --all-targets`
- ALL placeholder grep commands return no matches
- Event subscription code exists (all three categories)
- EventHandler trait implemented
- At least 9 profile event handlers exist
- At least 9 MCP event handlers exist
- At least 3 system event handlers exist
- ViewCommands emitted in handlers
- Evidence file created with all grep outputs

### FAIL Conditions
- Build fails
- ANY placeholder found
- Missing event subscription for any category
- Missing EventHandler implementation
- Handlers don't emit ViewCommands
- Evidence file missing or incomplete

## Related Requirements

- COORDINATING.md: Binary outcomes only (PASS or FAIL, no conditional)
- ARCHITECTURE_IMPROVEMENTS.md: Presenters must not call services directly
- presentation.md: SettingsPresenter must react to ProfileEvent and McpEvent
- P01A, P02A pattern: Follow same structure as ChatPresenter and HistoryPresenter
