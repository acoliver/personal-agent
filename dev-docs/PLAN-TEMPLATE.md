# Plan Template for Multi-Phase Features

## Plan Header

```markdown
# Plan: [FEATURE NAME]

Plan ID: PLAN-YYYYMMDD-[FEATURE]
Generated: YYYY-MM-DD
Total Phases: [N]
Requirements: [List of REQ-IDs this plan implements]

## Critical Reminders

Before implementing ANY phase, ensure you have:

1. Completed preflight verification (Phase 0.5)
2. Defined integration contracts for multi-component features
3. Written integration tests BEFORE unit tests
4. Verified all dependencies and types exist as assumed
```

---

## Phase Template

Each phase MUST follow this structure:

````markdown
# Phase [NN]: [Phase Title]

## Phase ID

`PLAN-YYYYMMDD-[FEATURE].P[NN]`

## Prerequisites

- Required: Phase [NN-1] completed
- Verification: `grep -r "@plan:PLAN-YYYYMMDD-[FEATURE].P[NN-1]" .`
- Expected files from previous phase: [list]
- Preflight verification: Phase 0.5 MUST be completed before any implementation phase

## Requirements Implemented (Expanded)

For EACH requirement this phase implements, provide:

### REQ-XXX: [Requirement Title]

**Full Text**: [Copy the complete requirement text here - DO NOT just reference]
**Behavior**:

- GIVEN: [precondition]
- WHEN: [action]
- THEN: [expected outcome]
  
**Why This Matters**: [1-2 sentences explaining the user value]

## Implementation Tasks

### Files to Create

- `src/path/to/file.rs` - [description]
  - MUST include: `/// @plan:PLAN-YYYYMMDD-[FEATURE].P[NN]`
  - MUST include: `/// @requirement:REQ-XXX`

### Files to Modify

- `src/path/to/existing.rs`
  - Line [N]: [change description]
  - ADD comment: `/// @plan:PLAN-YYYYMMDD-[FEATURE].P[NN]`
  - Implements: `/// @requirement:REQ-XXX`

### Required Code Markers

Every function/struct/test created in this phase MUST include:

```rust
/// @plan PLAN-YYYYMMDD-[FEATURE].P[NN]
/// @requirement REQ-XXX
/// @pseudocode lines X-Y (if applicable)
pub fn my_function() {
    // implementation
}
```
````

## Verification Commands

### Automated Checks (Structural)

```bash
# Check plan markers exist
grep -r "@plan:PLAN-YYYYMMDD-[FEATURE].P[NN]" . | wc -l
# Expected: [N] occurrences

# Check requirements covered
grep -r "@requirement:REQ-XXX" . | wc -l
# Expected: [N] occurrences

# Compile and run phase-specific tests
cargo build || exit 1
cargo test <phase_specific_tests> || exit 1
```

### Structural Verification Checklist

- [ ] Previous phase markers present
- [ ] No skipped phases (P[NN-1] exists)
- [ ] All listed files created/modified
- [ ] Plan markers added to all changes
- [ ] Tests pass for this phase
- [ ] No `todo!()` or `unimplemented!()` in phase code (except stub phases)

### Deferred Implementation Detection (MANDATORY after impl phases)

```bash
# Run ALL of these checks - if ANY match, phase FAILS:

# Check for todo!/unimplemented! left in implementation
grep -rn "todo!\|unimplemented!" [modified-files] --include="*.rs"
# Expected: No matches in implementation code

# Check for "cop-out" comments
grep -rn -E "(// TODO|// FIXME|// HACK|placeholder|not yet)" [modified-files] --include="*.rs"
# Expected: No matches

# Check for empty implementations
grep -rn "fn .* \{\s*\}" [modified-files] --include="*.rs"
# Expected: No matches in implementation code
```

### Semantic Verification Checklist (MANDATORY)

**Go beyond markers. Actually verify the behavior exists.**

#### Behavioral Verification Questions (answer ALL before proceeding)

1. **Does the code DO what the requirement says?**
   - [ ] I read the requirement text
   - [ ] I read the implementation code (not just checked file exists)
   - [ ] I can explain HOW the requirement is fulfilled

2. **Is this REAL implementation, not placeholder?**
   - [ ] Deferred implementation detection passed
   - [ ] No empty function bodies in implementation
   - [ ] No "will be implemented" comments

3. **Would the test FAIL if implementation was removed?**
   - [ ] Test verifies actual outputs, not just that code ran
   - [ ] Test would catch a broken implementation

4. **Is the feature REACHABLE by users?**
   - [ ] Code is called from existing code paths
   - [ ] There's a path from UI/CLI to this code

5. **What's MISSING?** (list gaps that need fixing before proceeding)
   - [ ] [gap 1]
   - [ ] [gap 2]

#### Feature Actually Works

```bash
# Manual test command (RUN THIS and paste actual output):
cargo run --bin personal_agent_menubar
# Then perform: [describe manual test steps]
# Expected behavior: [describe what should happen]
# Actual behavior: [paste what actually happens]
```

#### Integration Points Verified

- [ ] Caller passes correct data type to callee (verified by reading both files)
- [ ] Callee processes data correctly (verified by tracing execution)
- [ ] Return value used correctly by caller (verified by checking usage site)
- [ ] Error handling works at component boundaries (verified by inducing error)

#### Edge Cases Verified

- [ ] Empty/None input handled
- [ ] Invalid input rejected with clear error
- [ ] Boundary values work correctly
- [ ] Resource cleanup on failure

## Success Criteria

- All verification commands return expected results
- No phases skipped in sequence
- Plan markers traceable in codebase

## Failure Recovery

If this phase fails:

1. Rollback commands: `git checkout -- src/<modified_files>`
2. Files to revert: [list]
3. Cannot proceed to Phase [NN+1] until fixed

## Phase Completion Marker

Create: `project-plans/[feature]/.completed/P[NN].md`
Contents:

```markdown
Phase: P[NN]
Completed: YYYY-MM-DD HH:MM
Files Created: [list with line counts]
Files Modified: [list with diff stats]
Tests Added: [count]
Verification: [paste of verification command outputs]

## Holistic Functionality Assessment

### What was implemented?
[Describe in your own words what the code actually does]

### Does it satisfy the requirements?
[For each requirement, explain HOW the implementation satisfies it]

### What is the data flow?
[Trace one complete path: input -> processing -> output]

### What could go wrong?
[Identify edge cases, error conditions, or integration risks]

### Verdict
[PASS/FAIL with explanation]
```

---

## Preflight Verification Phase Template (Phase 0.5)

Before implementation begins, create this mandatory phase:

```markdown
# Phase 0.5: Preflight Verification

## Purpose
Verify ALL assumptions before writing any code.

## Dependency Verification
| Dependency | cargo tree Output | Status |
|------------|-------------------|--------|
| [crate1] | [paste output] | OK/MISSING |
| [crate2] | [paste output] | OK/MISSING |

## Type/Interface Verification
| Type Name | Expected Definition | Actual Definition | Match? |
|-----------|---------------------|-------------------|--------|
| [Type1] | [what plan assumes] | [what code shows] | YES/NO |

## Call Path Verification
| Function | Expected Caller | Actual Caller | Evidence |
|----------|-----------------|---------------|----------|
| [func1] | [where plan says] | [grep output] | [file:line] |

## Test Infrastructure Verification
| Component | Test File Exists? | Test Patterns Work? |
|-----------|-------------------|---------------------|
| [comp1] | YES/NO | YES/NO |

## Blocking Issues Found
[List any issues that MUST be resolved before proceeding]

## Verification Gate
- [ ] All dependencies verified
- [ ] All types match expectations
- [ ] All call paths are possible
- [ ] Test infrastructure ready

IF ANY CHECKBOX IS UNCHECKED: STOP and update plan before proceeding.
```

---

## Inline Requirement Expansion Template

When referencing requirements, ALWAYS expand them inline:

```markdown
### Scenario: Message Persistence

**Requirement ID**: REQ-CONV-001.1
**Requirement Text**: Messages MUST be persisted to conversation storage after each exchange
**Behavior Specification**:
- GIVEN: User sends a message in an active conversation
- WHEN: Assistant responds and streaming completes
- THEN: Both messages are saved to ~/Library/Application Support/PersonalAgent/conversations/{id}.json

**Why This Matters**: Without this, conversation history is lost on app restart

**Test Case**:
```rust
#[test]
fn test_messages_persisted_after_exchange() {
    let storage = ConversationStorage::new(temp_dir());
    let conversation = storage.create_conversation();
    
    // Simulate message exchange
    conversation.add_message(Message::user("Hello"));
    conversation.add_message(Message::assistant("Hi there!"));
    conversation.save().unwrap();
    
    // Verify persistence
    let loaded = storage.load_conversation(conversation.id).unwrap();
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[0].content, "Hello");
    assert_eq!(loaded.messages[1].content, "Hi there!");
}
```
```

---

## Plan Execution Tracking

At the start of the plan, create:

```markdown
# project-plans/[feature]/execution-tracker.md

## Execution Status

| Phase | ID | Status | Started | Completed | Verified | Semantic? | Notes |
|-------|-----|--------|---------|-----------|----------|-----------|-------|
| 0.5 | P0.5 | [ ] | - | - | - | N/A | Preflight verification |
| 03 | P03 | [ ] | - | - | - | [ ] | Create stub |
| 04 | P04 | [ ] | - | - | - | [ ] | Write TDD tests |
| 05 | P05 | [ ] | - | - | - | [ ] | Implementation |
| 06 | P06 | [ ] | - | - | - | [ ] | Integration stub |
| 07 | P07 | [ ] | - | - | - | [ ] | Integration TDD |
| 08 | P08 | [ ] | - | - | - | [ ] | Integration impl |
| 09 | P09 | [ ] | - | - | - | [ ] | Migration |
| 10 | P10 | [ ] | - | - | - | [ ] | Deprecation |

Note: "Semantic?" column tracks whether semantic verification (feature actually works) was performed.

## Completion Markers

- [ ] All phases have @plan markers in code
- [ ] All requirements have @requirement markers
- [ ] Verification script passes
- [ ] No phases skipped
```

This must be updated after EACH phase.

---

## Example Phase (Filled Out)

```markdown
# Phase 07: Chat Streaming Integration TDD

## Phase ID
`PLAN-20250126-CHATSTREAM.P07`

## Prerequisites
- Required: Phase 06 completed
- Verification: `grep -r "@plan:PLAN-20250126-CHATSTREAM.P06" .`
- Expected files from previous phase:
  - `src/llm/streaming.rs`
  - `tests/streaming_stub_tests.rs`
- Preflight verification: Phase 0.5 completed

## Requirements Implemented (Expanded)

### REQ-CHAT-003.1: Stream Token Display
**Full Text**: Tokens MUST appear in chat view within 200ms of receipt from LLM
**Behavior**:
- GIVEN: User sends a message and streaming begins
- WHEN: LLM sends first token
- THEN: Token appears in chat bubble within 200ms
**Why This Matters**: Users expect responsive real-time typing experience

## Implementation Tasks

### Files to Create
- `tests/streaming_integration_tests.rs`
  - MUST include: `/// @plan:PLAN-20250126-CHATSTREAM.P07`
  - MUST include: `/// @requirement:REQ-CHAT-003.1`
  - Test: Tokens appear in UI within timing window
  - Test: Stream cancellation stops token flow
  - Test: Error during stream shows error state

### Files to Modify
- `tests/mod.rs`
  - Line 15: Add `mod streaming_integration_tests;`
  - ADD comment: `/// @plan:PLAN-20250126-CHATSTREAM.P07`

### Required Code Markers
Every test MUST include:
```rust
/// @plan PLAN-20250126-CHATSTREAM.P07
/// @requirement REQ-CHAT-003.1
#[test]
fn test_tokens_appear_within_timing_window() {
    // test implementation
}
```

## Verification Commands

### Automated Checks

```bash
# Check plan markers exist
grep -r "@plan:PLAN-20250126-CHATSTREAM.P07" . | wc -l
# Expected: 8+ occurrences

# Check requirements covered
grep -r "@requirement:REQ-CHAT-003.1" tests/ | wc -l
# Expected: 3+ occurrences

# Compile (will fail until P08)
cargo build
# Expected: Compiles but tests fail
```

### Manual Verification Checklist

- [ ] Phase 06 markers present
- [ ] Test file created for streaming integration
- [ ] Tests follow behavioral pattern
- [ ] Tests will fail naturally until implementation
- [ ] All tests tagged with plan and requirement IDs

## Success Criteria

- 8+ tests created for streaming functionality
- All tests tagged with P07 marker
- Tests fail with assertion errors, not compile errors

## Failure Recovery

If this phase fails:

1. `git checkout -- tests/streaming_integration_tests.rs`
2. `git checkout -- tests/mod.rs`
3. Re-run Phase 07 with corrected requirements

## Phase Completion Marker

Create: `project-plans/chatstream/.completed/P07.md`
```
