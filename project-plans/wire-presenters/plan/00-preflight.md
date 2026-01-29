# Phase 00: Preflight Verification

**Plan ID**: PLAN-20250128-PRESENTERS
**Phase ID**: P00
**Type**: Preflight (REQUIRED before any implementation)
**Status**: Pending

## Objective

Verify all assumptions about existing code before proceeding with implementation. This phase gathers evidence that the EventBus API, event types, and presenter structures match our plan assumptions.

## Preflight Checks

### 1. EventBus API Verification

**Expected**: EventBus uses tokio::sync::broadcast with simple subscribe/publish API

```bash
# Verify EventBus structure
grep -n "pub fn" src/events/bus.rs
# Expected output shows: new(), publish(), subscribe(), subscriber_count()

# Verify subscribe returns broadcast::Receiver
grep -n "broadcast::Receiver" src/events/bus.rs
# Expected: subscribe() returns broadcast::Receiver<AppEvent>
```

**Evidence to capture**: `evidence/PLAN-20250128-PRESENTERS/preflight/eventbus-api.txt`

### 2. Event Type Verification

**Expected**: AppEvent enum contains User, Chat, Mcp, Profile, Conversation, Navigation, System variants

```bash
# List all event variants
grep -n "^pub enum\|^    [A-Z]" src/events/types.rs | head -100
# Expected: Shows all event enums and their variants

# Verify ChatEvent variants we need
grep -n "StreamStarted\|TextDelta\|ThinkingDelta\|ToolCallStarted\|ToolCallCompleted\|StreamCompleted\|StreamCancelled\|StreamError" src/events/types.rs
# Expected: All 8 ChatEvent variants exist
```

**Evidence to capture**: `evidence/PLAN-20250128-PRESENTERS/preflight/event-types.txt`

### 3. Presenter Structure Verification

**Expected**: Presenters exist in src/presentation/ with impl blocks

```bash
# Check presenter files exist
ls -la src/presentation/*.rs

# Check presenter struct definitions
grep -rn "^pub struct.*Presenter" src/presentation/
# Expected: ChatPresenter, HistoryPresenter, SettingsPresenter, ErrorPresenter

# Check if presenters already have event_bus field
grep -rn "event_bus\|EventBus" src/presentation/
# May or may not exist - we need to know current state
```

**Evidence to capture**: `evidence/PLAN-20250128-PRESENTERS/preflight/presenter-structure.txt`

### 4. ViewCommand Verification

**Expected**: ViewCommand enum exists for presenter -> UI communication

```bash
# Check ViewCommand exists
grep -rn "^pub enum ViewCommand" src/presentation/
# Expected: ViewCommand enum definition

# List ViewCommand variants
grep -n "^    [A-Z]" src/presentation/view_command.rs | head -50 || echo "ViewCommand may be in different file"
```

**Evidence to capture**: `evidence/PLAN-20250128-PRESENTERS/preflight/viewcommand.txt`

### 5. Build Verification

```bash
# Verify project builds
cargo build --all-targets 2>&1 | tail -20
# Expected: "Finished" message, exit code 0

# Verify tests pass
cargo test 2>&1 | tail -30
# Expected: All tests pass
```

**Evidence to capture**: `evidence/PLAN-20250128-PRESENTERS/preflight/build-status.txt`

### 6. Configuration Verification

```bash
# Check synthetic profile exists
ls -la ~/.llxprt/profiles/synthetic.json
cat ~/.llxprt/profiles/synthetic.json | head -10

# Check API key file exists
ls -la ~/.synthetic_key
# DO NOT cat the key file
```

**Evidence to capture**: `evidence/PLAN-20250128-PRESENTERS/preflight/config-status.txt`

## Inputs

- `src/events/bus.rs` - EventBus implementation
- `src/events/types.rs` - Event type definitions
- `src/presentation/*.rs` - Presenter implementations
- `~/.llxprt/profiles/synthetic.json` - LLM profile
- `~/.synthetic_key` - API key file

## Outputs

- Evidence files in `evidence/PLAN-20250128-PRESENTERS/preflight/`
- Determination of whether plan assumptions are correct
- List of any required plan adjustments

## PASS/FAIL Criteria

### PASS Conditions
- EventBus has `subscribe()` returning `broadcast::Receiver<AppEvent>`
- All expected ChatEvent variants exist (8 variants)
- Presenter structs exist in `src/presentation/`
- `cargo build --all-targets` exits 0
- `cargo test` exits 0
- Synthetic profile file exists

### FAIL Conditions
- EventBus API differs from expected (different subscribe signature)
- Missing event variants that plan depends on
- Presenter files don't exist
- Build fails
- Synthetic profile missing

## If Preflight FAILS

1. Document exact discrepancies
2. Update plan phases to match actual code structure
3. Re-run preflight verification
4. Only proceed when preflight passes
