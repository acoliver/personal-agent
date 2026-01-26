# Autonomous Plan-Creation Guide for LLxprt Workers

This document defines how to create foolproof implementation plans that prevent LLM fraud and ensure valid TDD implementations through autonomous worker execution.

**CRITICAL**: When executing plans, use the [PLAN-TEMPLATE.md](./PLAN-TEMPLATE.md) for generating plans.

---

## Core Principles

1. **TDD is MANDATORY** - Every line of production code must be written in response to a failing test
2. **Worker Isolation** - Each phase executed by fresh subagent instance with clean context
3. **Architect-First** - All plans begin with architect-written specification
4. **Analysis Before Code** - Mandatory analysis/pseudocode phases before implementation
5. **Aggressive Verification** - Multi-layered fraud detection at every step
6. **No Reverse Testing** - Tests NEVER check for NotYetImplemented or stub behavior
7. **Modify, Don't Duplicate** - Always UPDATE existing files, never create parallel versions
8. **NO ISOLATED FEATURES** - Every feature MUST be integrated into the existing system, not built in isolation
9. **Integration-First Testing** - Integration tests written BEFORE unit tests to verify component contracts
10. **Preflight Verification** - All assumptions verified BEFORE implementation begins
11. **Semantic over Structural** - Verify features WORK, not just that files/markers exist

---

## CRITICAL: Phase Numbering and Execution

### Sequential Execution is MANDATORY

**PROBLEM**: Coordinators skip phase numbers (executing 03, 06, 09, 16 instead of 03, 04, 05, 06...).

**SOLUTION**:

1. **NEVER SKIP NUMBERS** - Phases must be executed in exact numerical sequence
2. **USE PLAN IDS** - Every plan gets `PLAN-YYYYMMDD-FEATURE` ID
3. **TAG EVERYTHING** - Every implementation must include `@plan:PLAN-ID.P##` markers

### Required Plan Structure

```markdown
Plan ID: PLAN-20250126-FEATURE
Phases: 03, 04, 05, 06, 07, 08, 09, 10, 11, 12, 13, 14, 15, 16

Execution MUST be:
 P03 -> Verify -> P04 -> Verify -> P05 -> Verify -> P06 -> Verify -> P07...
 P03 -> P06 -> P09 -> P16 (WRONG - skipped phases)
```

### Code Traceability Requirements

Every function, test, and struct MUST include:

```rust
/// @plan PLAN-20250126-FEATURE.P07
/// @requirement REQ-003.1
/// @pseudocode lines 42-74
```

### Phase Verification Before Proceeding

Before starting Phase N, coordinator MUST verify:

```bash
# Check previous phase exists
grep -r "@plan:PLAN-ID.P$((N-1))" . || exit 1

# Cannot skip from P06 to P09
# Must do P07, P08 first
```

---

## CRITICAL: Pseudocode Usage Requirements

### Pseudocode MUST Be Used

**PROBLEM**: LLMs frequently create pseudocode then ignore it completely during implementation.

**SOLUTION**: Implementation phases MUST explicitly reference pseudocode line numbers:

```bash
# WRONG - Pseudocode ignored
"Implement the update_settings method based on requirements"

# CORRECT - Pseudocode enforced
"Implement update_settings method (from pseudocode lines 23-34):
- Line 23: VALIDATE changes with provider validator
- Line 24: BEGIN transaction
- Line 25: CLONE current settings
..."
```

---

## CRITICAL: Integration Requirements - STOP BUILDING ISOLATED FEATURES

### The Problem LLMs Keep Repeating

**PROBLEM**: LLMs constantly build perfect features in isolation that don't actually solve the problem because they're never connected to the existing system.

**EXAMPLES OF THIS MISTAKE**:

- Building SettingsService but not replacing Config's scattered settings
- Creating authentication system but not connecting it to existing endpoints
- Writing perfect tool handlers but not registering them with the tool registry
- Implementing caching layer but not using it in actual API calls

### MANDATORY Integration Analysis Phase

Every plan MUST include an integration analysis that answers:

1. **What existing code will USE this feature?**
   - List specific files and functions that need to call the new code
   - If answer is "nothing", the feature is useless

2. **What existing code needs to be REPLACED?**
   - Identify the buggy/scattered code being fixed
   - Plan how to deprecate and remove old implementation

3. **How will users ACCESS this feature?**
   - What commands/APIs/UI will invoke it?
   - If users can't reach it, it doesn't exist

4. **What needs to be MIGRATED?**
   - Existing data that needs conversion
   - Existing configs that need updating
   - Existing tests that need modification

5. **Integration Test Requirements**
   - Tests that verify the feature works WITH the existing system
   - Not just unit tests of the feature in isolation

### Required Integration Phases

Every plan MUST include these phases AFTER implementation:

```
06-integration-stub.md         # Wire feature into existing system
06a-integration-stub-verification.md
07-integration-tdd.md          # Tests that feature works IN CONTEXT
07a-integration-tdd-verification.md
08-integration-impl.md         # Actually connect to existing code
08a-integration-impl-verification.md
09-migration.md                # Migrate existing data/config
09a-migration-verification.md
10-deprecation.md              # Remove old implementation
10a-deprecation-verification.md
```

### Integration Checklist

Before ANY implementation starts, verify:

- [ ] Identified all touch points with existing system
- [ ] Listed specific files that will import/use the feature
- [ ] Identified old code to be replaced/removed
- [ ] Planned migration path for existing data
- [ ] Created integration tests that verify end-to-end flow
- [ ] User can actually access the feature through existing UI/CLI

**RED FLAG**: If the feature can be completely implemented without modifying ANY existing files except adding exports, it's probably built in isolation and won't solve the actual problem.

---

## Plan Structure

```
project-plans/<feature-slug>/
  specification.md           <- Architect-written specification
  analysis/                  <- Analysis artifacts
    domain-model.md
    pseudocode/
      component-001.md      <- MUST be referenced in implementation
      component-002.md      <- Line numbers cited in phases
  plan/
    00-overview.md          <- Generated from specification
    01-analysis.md          <- Domain analysis phase
    01a-analysis-verification.md
    02-pseudocode.md        <- Pseudocode development
    02a-pseudocode-verification.md
    03-<feature>-stub.md    <- Feature implementation phases
    03a-<feature>-stub-verification.md
    04-<feature>-tdd.md
    04a-<feature>-tdd-verification.md
    05-<feature>-impl.md    <- MUST reference pseudocode lines
    05a-<feature>-impl-verification.md
    ...
```

---

## Phase 0: Architect Specification (specification.md)

Written by architect worker BEFORE any implementation planning.

### Required Sections:

```markdown
# Feature Specification: <Name>

## Purpose

Clear statement of why this feature exists and what problem it solves.

## Architectural Decisions

- **Pattern**: (e.g., Service Layer, Repository, Event-Driven)
- **Technology Stack**: Rust, objc2/AppKit, tokio async runtime
- **Data Flow**: How data moves through the system
- **Integration Points**: Existing services/modules to connect with

## Project Structure

```
src/
  <module>/
    mod.rs          # Module exports
    types.rs        # Type definitions
    service.rs      # Business logic
    repository.rs   # Data access
tests/
  <module>_tests.rs # Behavioral tests
```

## Technical Environment
- **Type**: macOS Menu Bar App
- **Runtime**: Native macOS with tokio async
- **UI Framework**: AppKit via objc2
- **Dependencies**: List with exact versions from Cargo.toml

## Integration Points (MANDATORY SECTION)

### Existing Code That Will Use This Feature
- `src/ui/chat_view.rs` - Will call new streaming handler
- `src/llm/client.rs` - Will use new provider abstraction
- `src/config.rs` - Will use profile service

### Existing Code To Be Replaced
- `src/old_module.rs` - Legacy implementation to be removed
- Scattered logic in multiple files - Consolidate into service
- Direct storage access - Replace with repository calls

### User Access Points
- UI: Which view/button triggers this feature
- Menu bar: Which menu item
- Hotkey: Which keyboard shortcut

### Migration Requirements
- Existing conversations need migration to new format
- Config files need schema update
- Cached data needs invalidation

## Formal Requirements
[REQ-001] Feature Name
  [REQ-001.1] Specific behavior with acceptance criteria
  [REQ-001.2] Another specific behavior
  [REQ-001.3] Error handling requirements
[REQ-INT-001] Integration Requirements
  [REQ-INT-001.1] Replace existing implementation in all callers
  [REQ-INT-001.2] Update UI to use new service
  [REQ-INT-001.3] Migrate existing data without data loss

## Data Schemas

```rust
// Core entity
pub struct MyEntity {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

// API request/response
pub struct MyRequest {
    pub field: String,
}
```

## Example Data

```json
{
  "validInput": {
    "field": "value"
  },
  "invalidInput": {
    "field": ""
  }
}
```

## Constraints

- No blocking operations on main thread
- All async operations must use tokio runtime
- UI updates must be on main thread via MainThreadMarker
- Follow existing error handling patterns

## Performance Requirements

- UI response time: <100ms
- Streaming latency: <200ms to first token
- Memory usage: <100MB baseline
```

---

## Phase 0.5: Preflight Verification (MANDATORY)

**PURPOSE**: Verify ALL assumptions before writing any code.

### Required Verifications

#### 1. Dependency Verification
```bash
# For each crate referenced in the plan:
cargo tree -p <crate-name>    # Must show installed version
grep "<crate>" Cargo.toml     # Must find entry
```

**If any dependency is missing**: STOP. Update the plan.

#### 2. Type/Interface Verification
```bash
# For each type referenced in the plan, verify it exists:
grep -r "struct <TypeName>" src/

# Verify trait implementations exist:
grep -r "impl <TraitName> for" src/
```

**If types don't match plan assumptions**: STOP. Update the plan.

#### 3. Call Path Verification
```bash
# Verify the code paths described in the plan actually exist:
grep -r "<function_name>" src/ --include="*.rs"
```

**If call paths are impossible**: STOP. Redesign the plan.

#### 4. Test Infrastructure Verification
```bash
# Verify test files exist for components being modified:
ls tests/<module>_tests.rs

# Verify test patterns work:
cargo test --test <test_name> -- --list
```

**If test infrastructure is missing**: Add a phase to create it BEFORE implementation.

### Preflight Verification Checklist

Create `plan/00a-preflight-verification.md` with:

```markdown
# Preflight Verification Results

## Dependencies Verified
- [ ] `<dep1>`: `cargo tree -p <dep1>` output: [paste]

## Types Verified
- [ ] `<TypeName>`: Actual definition matches plan? [yes/no]
  - Expected: [what plan says]
  - Actual: [what code shows]

## Call Paths Verified
- [ ] `<function>`: Can be called from `<caller>`? [yes/no]
  - Evidence: [grep output]

## Test Infrastructure Verified
- [ ] Test file exists: `tests/<file>_tests.rs`
- [ ] Test patterns work: [sample output]

## Blocking Issues Found
[List any issues that require plan modification before proceeding]
```

---

## Phase 3+: Implementation Cycles

Each feature follows strict 3-phase TDD cycle:

### A. Stub Phase

**Goal**: Create minimal skeleton that compiles

**CRITICAL RULES**:

- Stubs can return `todo!()` or `unimplemented!()` OR return default values
- Tests MUST NOT expect panic from `todo!()` (no reverse testing)
- Tests will fail naturally when stub panics or returns wrong values
- NEVER create `ServiceV2` or `ServiceNew` - UPDATE existing files

**Worker Prompt**:

```bash
llxprt_task --subagent deepthinker --goal "
Implement stub for <feature> based on:
- specification.md section <X>
- analysis/pseudocode/<component>.md

Requirements:
1. UPDATE existing files (do not create new versions)
2. Methods can either:
   - Use todo!() macro
   - OR return default values of correct type
3. If returning default values:
   - Structs: return Default::default() or empty struct
   - Vecs: return vec![]
   - Options: return None
   - Results: return Ok(default) or Err(todo!())
4. Maximum 100 lines total
5. Must compile with cargo build

FORBIDDEN:
- Creating ServiceV2 or parallel versions
- TODO comments in production code (use todo!() macro instead)

Output status to workers/phase-03.json
"
```

**Verification MUST**:

```bash
# Check for TODO comments (todo!() macro is OK in stubs)
grep -r "// TODO" src/
[ $? -eq 0 ] && echo "FAIL: TODO comments found"

# Check for version duplication
find src -name "*_v2*" -o -name "*_new*" -o -name "*_copy*"
[ $? -eq 0 ] && echo "FAIL: Duplicate versions created"

# Verify Rust compiles
cargo build || exit 1

# Verify tests don't EXPECT panic (reverse testing)
grep -r "should_panic\|#\[should_panic\]" tests/
[ $? -eq 0 ] && echo "FAIL: Tests expecting panic (reverse testing)"
```

### B. TDD Phase

**CRITICAL**: This phase determines success/failure of implementation

**MANDATORY RULES**:

- Tests expect REAL BEHAVIOR that doesn't exist yet
- NO testing for panic/todo!()
- NO reverse tests
- Tests naturally fail with assertion errors

**Worker Prompt**:

```bash
llxprt_task --subagent deepthinker --goal "
Write comprehensive BEHAVIORAL tests for <feature> based on:
- specification.md requirements [REQ-X]
- analysis/pseudocode/<component>.md
- Example data from specification

MANDATORY RULES:
1. Test ACTUAL BEHAVIOR with real data flows
2. NEVER test for panic or todo!() behavior
3. Each test must transform INPUT -> OUTPUT based on requirements
4. NO tests that just verify mocks were called
5. NO tests that only check struct fields exist
6. NO reverse tests
7. Each test must have doc comment:
   /// @requirement REQ-001.1
   /// @scenario Valid user login
   /// @given { email: 'user@example.com', password: 'Valid123!' }
   /// @when login_user() is called
   /// @then Returns Ok(Session) with valid token

FORBIDDEN PATTERNS:
- #[should_panic]
- assert!(result.is_err()) without checking error type
- Tests that pass with empty implementations

Create 15-20 BEHAVIORAL tests covering:
- Input -> Output transformations for each requirement
- State changes and side effects
- Error conditions with specific error types/messages
- Integration between components (real, not mocked)

Output status to workers/phase-04.json
"
```

**Verification MUST**:

```bash
# Check for reverse testing
grep -r "should_panic" tests/ && echo "FAIL: Reverse testing found"

# Run tests - should fail naturally
cargo test 2>&1 | head -20
# Should see: assertion failed or todo!() panic
# NOT: "test passed"
```

### C. Implementation Phase

**CRITICAL**: Must follow pseudocode line-by-line

**Worker Prompt**:

```bash
llxprt_task --subagent deepthinker --goal "
Implement <feature> to make ALL tests pass.

UPDATE src/<feature>.rs
(MODIFY existing file - do not create new)

MANDATORY: Follow pseudocode EXACTLY from analysis/pseudocode/<component>.md:

Example for update_settings method:
- Line 11: VALIDATE changes against schema
  -> Use validator: validate(&changes)?
- Line 14: BEGIN TRANSACTION
  -> let backup = self.settings.clone()
- Line 15: CLONE current settings
  -> let mut new_settings = self.settings.clone()
- Line 16: MERGE changes into clone
  -> new_settings.merge(changes)
- Line 17: PERSIST to repository
  -> self.repository.save(&new_settings).await?
- Line 19-21: ON ERROR ROLLBACK
  -> catch: self.settings = backup; return Err(e)

Requirements:
1. Do NOT modify any existing tests
2. UPDATE existing files (no new versions)
3. Implement EXACTLY what pseudocode specifies
4. Reference pseudocode line numbers in comments
5. All tests must pass
6. No println! or debug code
7. No TODO comments

Run 'cargo test' and ensure all pass.
Output status to workers/phase-05.json
"
```

**Verification MUST**:

```bash
# All tests pass
cargo test || exit 1

# No test modifications
git diff tests/ | grep -E "^[+-]" | grep -v "^[+-]{3}" && echo "FAIL: Tests modified"

# No debug code
grep -r "println!\|dbg!\|todo!\|unimplemented!" src/ --include="*.rs" | grep -v "// " && echo "FAIL: Debug code found"

# No duplicate files
find src -name "*_v2*" -o -name "*_copy*" && echo "FAIL: Duplicate versions found"
```

---

## Worker Execution Protocol

### Using LLxprt Subagents

```bash
# Launch analysis workers via llxprt_task
llxprt_task --subagent deepthinker --goal "Analyze domain..." &
llxprt_task --subagent deepthinker --goal "Create pseudocode for auth..." &
```

### Sequential Implementation

```bash
# Each 3-phase cycle must complete before next
./execute-phase.sh 03 03a  # auth-stub + verification
./execute-phase.sh 04 04a  # auth-tdd + verification
./execute-phase.sh 05 05a  # auth-impl + verification

# Only then move to next feature
./execute-phase.sh 06 06a  # user-stub + verification
```

---

## Verification Patterns for Rust

### 1. Compilation Check

```bash
cargo build --all-targets || exit 1
cargo clippy -- -D warnings || exit 1
```

### 2. Test Coverage

```bash
cargo tarpaulin --out Html --output-dir coverage/
# Check coverage report
```

### 3. Integration Coherence

```bash
# Verify components work together
cargo test --test integration_tests || exit 1
```

### 4. Deferred Implementation Detection

```bash
# MANDATORY: Run this check after every implementation phase
grep -rn -E "(todo!|unimplemented!|TODO|FIXME|HACK)" src/ --include="*.rs"
# Expected: No matches in implementation code (stubs are OK in stub phases)

# Detect "placeholder" patterns
grep -rn -E "(placeholder|not yet|will be)" src/ --include="*.rs"
# Expected: No matches
```

---

## Plan Evaluation Checklist

After creating a plan, evaluate it for:

### 1. Integration Analysis (MOST CRITICAL)

- [ ] **Lists specific existing files that will use the feature**
- [ ] **Identifies exact code to be replaced/removed**
- [ ] **Shows how users will access the feature**
- [ ] **Includes migration plan for existing data**
- [ ] **Has integration test phases**
- [ ] **Feature CANNOT work without modifying existing files**

### 2. Pseudocode Compliance

- [ ] Pseudocode files have numbered lines
- [ ] Implementation phases reference line numbers
- [ ] Verification checks pseudocode was followed
- [ ] No unused pseudocode files

### 3. TDD Phase

- [ ] Tests expect real behavior
- [ ] No testing for panic/todo!()
- [ ] No reverse tests
- [ ] Behavioral assertions

### 4. Implementation Phase

- [ ] References pseudocode line numbers
- [ ] Updates existing files (no V2 versions)
- [ ] Verification compares to pseudocode
- [ ] No test modifications allowed

### 5. Anti-Patterns Detected

- [ ] No `ServiceV2` or `ServiceNew` files
- [ ] No parallel implementations
- [ ] No test modifications during implementation

---

## Success Metrics

A well-executed plan will have:

1. **Zero test modifications** between TDD and implementation phases
2. **>80% code coverage** from behavioral tests
3. **All REQ tags** covered by behavioral tests with actual assertions
4. **Pseudocode match** - every line traceable between design and implementation
5. **Clean worker execution** with no context overflow
6. **No duplicate versions** - all updates to existing files
7. **No reverse testing** - tests never check for panic/todo!()

**Red Flags of Fraudulent Implementation**:

- Tests that pass with empty implementations
- ServiceV2 or parallel versions created
- Tests checking for panic behavior
- Test modifications during implementation
- Pseudocode ignored during implementation

Remember: Pseudocode is not optional decoration - it's the blueprint that implementation MUST follow line by line.
