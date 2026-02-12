# Phase 03a: Bridge Stub Verification

## Phase ID

`PLAN-20250128-GPUI.P03a`

## Prerequisites

- Phase 03 evidence file exists: `project-plans/gpui-migration/plan/.completed/P03.md`

---

## Verification Protocol

### 1. File Structure

```bash
ls -la src/ui_gpui/bridge/
```

Expected files:
- [ ] `mod.rs`
- [ ] `gpui_bridge.rs`
- [ ] `view_command_sink.rs`
- [ ] `user_event_forwarder.rs`

### 2. Plan Markers

```bash
grep -c "@plan PLAN-20250128-GPUI.P03" src/ui_gpui/bridge/*.rs
```

Expected: 10+ total occurrences

### 3. Requirement Markers

```bash
grep -c "@requirement REQ-GPUI-006" src/ui_gpui/bridge/*.rs
```

Expected: 6+ occurrences covering:
- REQ-GPUI-006.1 (flume channels)
- REQ-GPUI-006.2 (try_send)
- REQ-GPUI-006.3 (try_recv drain)
- REQ-GPUI-006.4 (ViewCommandSink)
- REQ-GPUI-006.5 (notifier)

### 4. flume Dependency

```bash
grep "flume" Cargo.toml
```

Expected: `flume = "0.11"` or similar

### 5. Module Export

```bash
grep "pub mod bridge" src/ui_gpui/mod.rs
grep "pub use bridge" src/ui_gpui/mod.rs
```

Expected: Both present

### 6. Compilation

```bash
cargo build
```

Expected: Success (warnings OK, errors NOT OK)

### 7. Type Definitions Present

```bash
grep "pub struct GpuiBridge" src/ui_gpui/bridge/gpui_bridge.rs
grep "pub struct ViewCommandSink" src/ui_gpui/bridge/view_command_sink.rs
grep "pub fn spawn_user_event_forwarder" src/ui_gpui/bridge/user_event_forwarder.rs
```

Expected: All three present

---

## Stub Phase Allowances

In stub phases, these are ALLOWED:
- `unimplemented!("description")`
- Empty struct bodies that compile

NOT allowed:
- `// TODO` comments
- Missing @plan markers
- Compilation errors

---

## Evidence File

Create: `project-plans/gpui-migration/plan/.completed/P03A.md`

```markdown
# Phase 03a: Bridge Stub Verification Results

## Verdict: [PASS|FAIL]

## File Structure
```bash
$ ls -la src/ui_gpui/bridge/
[paste output]
```

## Plan Markers
```bash
$ grep -c "@plan PLAN-20250128-GPUI.P03" src/ui_gpui/bridge/*.rs
[paste output]
```
Total: [N] (expected: 10+)

## Requirement Markers
```bash
$ grep -c "@requirement REQ-GPUI-006" src/ui_gpui/bridge/*.rs
[paste output]
```
Total: [N] (expected: 6+)

## flume Dependency
```bash
$ grep "flume" Cargo.toml
[paste output]
```

## Module Export
```bash
$ grep -E "pub (mod|use) bridge" src/ui_gpui/mod.rs
[paste output]
```

## Compilation
```bash
$ cargo build 2>&1 | tail -5
[paste output]
```

## Type Definitions
- GpuiBridge: [FOUND/NOT FOUND]
- ViewCommandSink: [FOUND/NOT FOUND]
- spawn_user_event_forwarder: [FOUND/NOT FOUND]

## Verdict Justification
[Explain why PASS or FAIL]
```

---

## Next Phase

After P03a completes with PASS:
--> P04: Bridge TDD
