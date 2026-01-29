# Phase 02a: Verify HistoryPresenter Event Wiring

**Phase ID**: P02a
**Type**: Verification
**Status**: Pending
**Prerequisites**: P02 completion marker exists

## Objective

Verify that HistoryPresenter correctly subscribes to event bus and handles all required conversation lifecycle events per dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md section "Presentation Layer Isolation".

## Verification Protocol

Per dev-docs/COORDINATING.md, this is a SKEPTICAL AUDITOR phase. Assume nothing works until you have EVIDENCE it works.

## Prerequisite Check

Before running verification, confirm:

```bash
# Does P02 completion marker exist?
ls project-plans/wire-presenters/plan/.completed/P02.md

# Does P01a have PASS verdict?
grep "^## Verdict: PASS" project-plans/wire-presenters/plan/.completed/P01A.md

# Does evidence show no placeholders?
grep "unimplemented!" project-plans/wire-presenters/plan/.completed/P01A.md
# Expected: (no output)
```

If any check fails: DO NOT PROCEED. Remediate failed phase first.

## Structural Checks

### 1. Files Exist

```bash
# Check that HistoryPresenter was modified
git diff --stat src/presentation/history_presenter.rs
# Should show meaningful changes
```

Expected: HistoryPresenter has event subscription code

### 2. Markers Present

```bash
# Check for plan markers
grep -c "@plan PLAN-20250128-PRESENTERS.P02" src/presentation/history_presenter.rs
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
grep -rn "unimplemented!" src/presentation/history_presenter.rs
# Expected: (no output)

# Check 2: todo! macro
grep -rn "todo!" src/presentation/history_presenter.rs
# Expected: (no output)

# Check 3: TODO/FIXME comments
grep -rn "// TODO\|// FIXME\|// HACK\|// STUB" src/presentation/history_presenter.rs
# Expected: (no output)

# Check 4: Placeholder strings
grep -rn "placeholder\|not yet implemented\|will be implemented" src/presentation/history_presenter.rs
# Expected: (no output)
```

**IF ANY GREP RETURNS MATCHES: STOP. VERDICT IS FAIL.**

## Semantic Verification

### 1. Event Subscription Code

```bash
# Check that EventBus subscription exists
grep -A 5 "subscribe_to_events\|event_bus.subscribe" src/presentation/history_presenter.rs
# Expected: Subscription code for UserEvent and ConversationEvent
```

What to verify:
- [ ] HistoryPresenter has `event_bus` field
- [ ] Constructor calls `subscribe_to_events()`
- [ ] Subscription filters for UserEvent::SelectConversation and related
- [ ] Subscription filters for all ConversationEvent variants

### 2. Event Handler Implementation

```bash
# Check EventHandler trait implementation
grep -A 3 "impl EventHandler for HistoryPresenter" src/presentation/history_presenter.rs
# Expected: Match statement on AppEvent
```

What to verify:
- [ ] `impl EventHandler for HistoryPresenter` exists
- [ ] `handle_event` method exists
- [ ] Match statement handles at least 5 ConversationEvent variants
- [ ] Each handler calls a private method

### 3. ViewCommand Emission

```bash
# Check that handlers emit ViewCommands
grep -c "emit_view_command\|ViewCommand::" src/presentation/history_presenter.rs
# Expected: >= 5 (one per event handler)
```

What to verify:
- [ ] Each event handler emits a ViewCommand
- [ ] ViewCommand variants match event types (AppendConversationItem, RemoveConversationItem, etc.)
- [ ] No direct service calls in handlers

### 4. Specific Event Handlers

Verify these specific handlers exist:

```bash
# Check for required handlers
grep -c "fn on_conversation_created\|fn on_conversation_deleted\|fn on_title_updated" src/presentation/history_presenter.rs
# Expected: >= 3

grep -c "fn on_select_conversation\|fn on_start_rename\|fn on_confirm_rename" src/presentation/history_presenter.rs
# Expected: >= 3

grep -c "fn on_list_refreshed\|fn on_conversation_activated\|fn on_conversation_deactivated" src/presentation/history_presenter.rs
# Expected: >= 3
```

Expected: At least 9 event handler methods

## Code Tracing

Pick ONE event and trace the full flow:

**Example Trace: `AppEvent::Conversation(ConversationEvent::Created { id, title })`**

1. Read the event handler:
```bash
sed -n '/ConversationEvent::Created/,/^[[:space:]]*}/p' src/presentation/history_presenter.rs
```

2. Verify:
- [ ] Handler extracts id and title from event
- [ ] Handler constructs ViewCommand::AppendConversationItem or similar
- [ ] Handler calls `emit_view_command(cmd)`

## Comparison to ChatPresenter

```bash
# Verify similar pattern to ChatPresenter
grep -c "impl EventHandler" src/presentation/history_presenter.rs
# Expected: >= 1 (same pattern as ChatPresenter)

# Check for similar field names
grep -c "event_bus:\|Arc<EventBus>" src/presentation/history_presenter.rs
# Expected: >= 1
```

## Inputs

### Files to Read
- `src/presentation/history_presenter.rs` - Full implementation
- `project-plans/wire-presenters/plan/.completed/P02.md` - Phase completion evidence
- `src/events/types.rs` - ConversationEvent definitions
- `project-plans/wire-presenters/plan/.completed/P01A.md` - Reference implementation

### Evidence Required
- Exact grep outputs from all checks above
- File:line references for event subscription
- Function names for all event handlers
- At least one full code trace

## Outputs

### Evidence File
Create: `project-plans/wire-presenters/plan/.completed/P02A.md`

Must contain:

```markdown
# Phase 02a Verification Results

## Verdict: [PASS|FAIL]

## Structural Checks
- Files exist: [YES/NO with git diff --stat output]
- Markers present: [YES/NO with grep count]
- Build: [PASS/FAIL with output]

## Placeholder Detection
Command: grep -rn "unimplemented!" src/presentation/history_presenter.rs
Output: [exact output or "no matches"]

Command: grep -rn "todo!" src/presentation/history_presenter.rs
Output: [exact output or "no matches"]

Command: grep -rn "// TODO\|// FIXME" src/presentation/history_presenter.rs
Output: [exact output or "no matches"]

Command: grep -rn "placeholder" src/presentation/history_presenter.rs
Output: [exact output or "no matches"]

## Semantic Verification
- Event subscription exists: [YES/NO with grep output]
- EventHandler trait implemented: [YES/NO with grep output]
- ViewCommands emitted: [YES/NO with count]
- Required handlers exist: [list handlers found]

## Code Trace Example
[Pick one event and show the full flow]

## Evidence Summary
[Bullet points of what was verified]

## Blocking Issues (if FAIL)
[List exactly what must be fixed]
```

## PASS/FAIL Criteria

### PASS Conditions
- Exit code 0 from `cargo build --all-targets`
- ALL placeholder grep commands return no matches
- Event subscription code exists
- EventHandler trait implemented
- At least 9 event handler methods exist (all ConversationEvent variants)
- ViewCommands emitted in handlers
- Evidence file created with all grep outputs

### FAIL Conditions
- Build fails
- ANY placeholder found
- Missing event subscription
- Missing EventHandler implementation
- Handlers don't emit ViewCommands
- Evidence file missing or incomplete

## Related Requirements

- COORDINATING.md: Binary outcomes only (PASS or FAIL, no conditional)
- ARCHITECTURE_IMPROVEMENTS.md: Presenters must not call services directly
- presentation.md: HistoryPresenter must react to ConversationEvent
- P01A pattern: Follow same structure as ChatPresenter
