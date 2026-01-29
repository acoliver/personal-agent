# Phase 01: Preflight Verification

## Phase ID

`PLAN-20250125-REFACTOR.P01`

## Prerequisites

- Required: None (first phase)
- Verification: None
- Expected files from previous phase: None
- Preflight verification: This IS the preflight verification phase

## Purpose

Verify ALL assumptions before writing any implementation code. This phase ensures that:

1. All required dependencies are available in the project
2. All types and interfaces we plan to use actually exist
3. All module paths are valid
4. Test infrastructure is ready
5. Build and toolchain work correctly

## Requirements Implemented

None - This is a verification phase only.

## Implementation Tasks

### Files to Create

- `project-plans/refactor/preflight-report.md`
  - MUST include: `@plan:PLAN-20250125-REFACTOR.P01`
  - Complete dependency verification report
  - Type verification tables
  - Module path verification
  - Test infrastructure verification
  - Blocking issues list
  - Verification gate checklist

### Verification Commands to Execute

#### Dependency Verification

```bash
# Check Tokio dependency
cargo tree | grep tokio
# Expected: tokio with features: rt-multi-thread, macros, sync

# Check Serde dependency
cargo tree | grep serde
# Expected: serde with features: derive

# Check Thiserror dependency
cargo tree | grep thiserror
# Expected: thiserror present

# Check Tracing dependency
cargo tree | grep tracing
# Expected: tracing and tracing-subscriber present

# Check Anyhow dependency (if used)
cargo tree | grep anyhow
# Expected: anyhow present (if used in project)
```

#### Type/Interface Verification

```bash
# Check GlobalRuntime exists
grep -r "pub struct GlobalRuntime" src/
# Expected: Found in agent/runtime.rs

# Check SecretsManager exists
grep -r "pub struct SecretsManager" src/
# Expected: Found in mcp/secrets.rs

# Check Settings exists
grep -r "pub struct Settings" src/
# Expected: Found in config/settings.rs

# Check ModelProfile exists
grep -r "pub struct ModelProfile" src/
# Expected: Found in models/mod.rs

# Check HttpClient patterns
grep -r "reqwest::Client" src/
# Expected: Found in registry/ module
```

#### Module Path Verification

```bash
# Check agent module structure
ls -la src/agent/
# Expected: runtime.rs exists

# Check mcp module structure
ls -la src/mcp/
# Expected: service.rs, manager.rs, secrets.rs exist

# Check llm module structure
ls -la src/llm/
# Expected: client.rs exists

# Check config module structure
ls -la src/config/
# Expected: settings.rs exists

# Check registry module structure
ls -la src/registry/
# Expected: module exists with cache/fetch code
```

#### Test Infrastructure Verification

```bash
# Check if tests directory exists
ls -la tests/
# Expected: Directory exists

# Check if project compiles
cargo build --all-targets
# Expected: Builds successfully

# Check if tests compile
cargo test --no-run
# Expected: Tests compile successfully

# Check for test helpers
ls -la src/*/tests.rs
# Expected: Some test files exist
```

#### Build and Toolchain Verification

```bash
# Rust version
rustc --version
# Expected: 1.70+ or current stable

# Cargo version
cargo --version
# Expected: Current version

# Check for .rustfmt.toml
ls -la .rustfmt.toml
# Expected: File exists

# Check for clippy configuration
grep clippy Cargo.toml
# Expected: Some clippy configuration or defaults

# Format check
cargo fmt --check
# Expected: All code formatted
```

## Verification Commands

### Automated Checks

```bash
# Compile project
cargo build --all-targets 2>&1 | tee /tmp/preflight-build.log
# Expected: Exit code 0

# Run tests (if any)
cargo test --no-run 2>&1 | tee /tmp/preflight-test-compile.log
# Expected: Exit code 0

# Check formatting
cargo fmt --check 2>&1 | tee /tmp/preflight-fmt.log
# Expected: Exit code 0

# Run clippy
cargo clippy --all-targets -- -D warnings 2>&1 | tee /tmp/preflight-clippy.log
# Expected: No errors (warnings acceptable)
```

### Manual Verification Checklist

- [ ] All dependencies verified (cargo tree output matches expectations)
- [ ] All types match expectations (GlobalRuntime, SecretsManager, etc. exist)
- [ ] All module paths are valid (files exist in expected locations)
- [ ] Test infrastructure ready (tests compile, test directory exists)
- [ ] Build and toolchain work correctly (cargo build, fmt, clippy pass)

## Blocking Issues Found

[List any issues discovered during verification]

_If ANY blocking issue is found, the refactor CANNOT proceed until resolved._

## Verification Gate

Before proceeding to Phase 02a (Analysis Verification), ensure:

- [ ] All dependencies verified and present
- [ ] All types match expectations
- [ ] All module paths are valid
- [ ] Test infrastructure ready
- [ ] Build succeeds without errors
- [ ] Format check passes
- [ ] Clippy produces no errors (warnings acceptable)

**IF ANY CHECKBOX IS UNCHECKED: STOP and update plan before proceeding.**

## Success Criteria

- Complete preflight report created
- All dependencies verified
- All types and interfaces confirmed to exist
- All module paths validated
- Test infrastructure confirmed ready
- Build and toolchain verified working
- No blocking issues found

## Failure Recovery

If this phase fails (blocking issues found):

1. Document the blocking issue in detail
2. Update the specification or plan to address the issue
3. Re-run verification until all checks pass
4. Cannot proceed to Phase 02a until all checks pass

## Phase Completion Marker

Create: `project-plans/refactor/plan/.completed/P01.md`

Contents:

```markdown
Phase: P01
Completed: [YYYY-MM-DD HH:MM]
Files Created: preflight-report.md
Files Modified: None
Tests Added: None (verification phase)
Verification: [Paste outputs of all verification commands]
Blocking Issues: None / [List issues if found]
```

## Next Steps

After successful completion of this phase:

1. Proceed to Phase 01a: Preflight Verification Checklist
2. Verify all preflight checks are complete and documented
3. Then proceed to Phase 02a: Analysis Verification
