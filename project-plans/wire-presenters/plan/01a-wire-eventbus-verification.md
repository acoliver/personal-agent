# Phase 01a: Verify ChatPresenter Event Wiring

**Phase ID**: P01a
**Type**: Verification
**Status**: Pending
**Prerequisites**: P01 completion marker exists

## Objective

Verify that ChatPresenter correctly subscribes to event bus and handles all required events per dev-docs/architecture/ARCHITECTURE_IMPROVEMENTS.md section "Presentation Layer Isolation".

## Verification Protocol

Per dev-docs/COORDINATING.md, this is a SKEPTICAL AUDITOR phase. Assume nothing works until you have EVIDENCE it works.

## Prerequisite Check

Before running verification, confirm:

```bash
# Does P01 completion marker exist?
ls project-plans/wire-presenters/plan/.completed/P01.md

# Does it contain evidence?
wc -l project-plans/wire-presenters/plan/.completed/P01.md
# Should have > 0 lines
```

If either check fails: DO NOT PROCEED. Phase P01 must be completed first.

## Structural Checks

### 1. Files Exist

```bash
# Check that ChatPresenter was modified
git diff --stat src/presentation/chat_presenter.rs
# Should show meaningful changes
```

Expected: ChatPresenter has event subscription code

### 2. Markers Present

```bash
# Check for plan markers in implementation
grep -c "@plan PLAN-20250128-PRESENTERS.P01" src/presentation/chat_presenter.rs
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
grep -rn "unimplemented!" src/presentation/chat_presenter.rs
# Expected: (no output)

# Check 2: todo! macro
grep -rn "todo!" src/presentation/chat_presenter.rs
# Expected: (no output)

# Check 3: TODO/FIXME comments
grep -rn "// TODO\|// FIXME\|// HACK\|// STUB" src/presentation/chat_presenter.rs
# Expected: (no output)

# Check 4: Placeholder strings
grep -rn "placeholder\|not yet implemented\|will be implemented" src/presentation/chat_presenter.rs
# Expected: (no output)
```

**IF ANY GREP RETURNS MATCHES: STOP. VERDICT IS FAIL.**

## Semantic Verification

### 1. Event Subscription Code

```bash
# Check that EventBus subscription exists
grep -A 5 "subscribe_to_events\|event_bus.subscribe" src/presentation/chat_presenter.rs
# Expected: Subscription code matching AppEvent patterns
```

What to verify:
- [ ] ChatPresenter has `event_bus` field
- [ ] Constructor calls `subscribe_to_events()` or similar
- [ ] Subscription filters for relevant events (UserEvent, ChatEvent)

### 2. Event Handler Implementation

```bash
# Check EventHandler trait implementation
grep -A 3 "impl EventHandler for ChatPresenter" src/presentation/chat_presenter.rs
# Expected: Match statement on AppEvent
```

What to verify:
- [ ] `impl EventHandler for ChatPresenter` exists
- [ ] `handle_event` method exists
- [ ] Match statement handles at least 5 ChatEvent variants
- [ ] Each handler calls a private method (e.g., `on_stream_started`)

### 3. ViewCommand Emission

```bash
# Check that handlers emit ViewCommands
grep -c "emit_view_command\|ViewCommand::" src/presentation/chat_presenter.rs
# Expected: >= 5 (one per event handler)
```

What to verify:
- [ ] Each event handler emits a ViewCommand
- [ ] ViewCommand variants match event types
- [ ] No direct service calls in handlers

### 4. Specific Event Handlers

Verify these specific handlers exist (grep for function names):

```bash
# Check for required handlers
grep -c "fn on_stream_started\|fn on_text_delta\|fn on_thinking_delta" src/presentation/chat_presenter.rs
# Expected: >= 3

grep -c "fn on_tool_call_started\|fn on_tool_call_completed" src/presentation/chat_presenter.rs
# Expected: >= 2

grep -c "fn on_stream_completed\|fn on_stream_error\|fn on_stream_cancelled" src/presentation/chat_presenter.rs
# Expected: >= 3
```

Expected: At least 8 event handler methods

## Code Tracing

Pick ONE event and trace the full flow:

**Example Trace: `AppEvent::Chat(ChatEvent::TextDelta { text })`**

1. Read the event handler:
```bash
sed -n '/TextDelta.*text/,/^[[:space:]]*}/p' src/presentation/chat_presenter.rs
```

2. Verify:
- [ ] Handler extracts text from event
- [ ] Handler constructs ViewCommand::AppendMessageContent or similar
- [ ] Handler calls `emit_view_command(cmd)`

## Inputs

### Files to Read
- `src/presentation/chat_presenter.rs` - Full implementation
- `project-plans/wire-presenters/plan/.completed/P01.md` - Phase completion evidence
- `src/events/types.rs` - Event definitions to verify handlers match

### Evidence Required
- Exact grep outputs from all checks above
- File:line references for event subscription
- Function names for all event handlers
- At least one full code trace

## Outputs

### Evidence File
Create: `project-plans/wire-presenters/plan/.completed/P01A.md`

Must contain:

```markdown
# Phase 01a Verification Results

## Verdict: [PASS|FAIL]

## Structural Checks
- Files exist: [YES/NO with git diff --stat output]
- Markers present: [YES/NO with grep count]
- Build: [PASS/FAIL with output]

## Placeholder Detection
Command: grep -rn "unimplemented!" src/presentation/chat_presenter.rs
Output: [exact output or "no matches"]

Command: grep -rn "todo!" src/presentation/chat_presenter.rs
Output: [exact output or "no matches"]

Command: grep -rn "// TODO\|// FIXME\|// HACK" src/presentation/chat_presenter.rs
Output: [exact output or "no matches"]

Command: grep -rn "placeholder" src/presentation/chat_presenter.rs
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
- Event subscription code exists (grep finds "subscribe_to_events")
- EventHandler trait implemented
- At least 8 event handler methods exist
- ViewCommands emitted in handlers
- Evidence file created with all grep outputs

### FAIL Conditions
- Build fails
- ANY placeholder found (unimplemented!, todo!, etc.)
- Missing event subscription
- Missing EventHandler implementation
- Handlers don't emit ViewCommands
- Evidence file missing or incomplete

## Related Requirements

- COORDINATING.md: Binary outcomes only (PASS or FAIL, no conditional)
- ARCHITECTURE_IMPROVEMENTS.md: Presenters must not call services directly
- presentation.md: ChatPresenter must react to UserEvent and ChatEvent
