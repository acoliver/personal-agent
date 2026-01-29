# Plan Coordination Guide

This document defines how to coordinate multi-phase plan execution using subagents with strict verification and remediation loops.

**VERSION**: 1.0
**CRITICAL**: There is NO conditional pass. Every phase either PASSES or FAILS. Failures MUST be remediated before proceeding.

---

## Core Principles

### 1. Binary Outcomes Only

Every verification has exactly two outcomes:
- **PASS**: All criteria met, evidence recorded, proceed to next phase
- **FAIL**: One or more criteria not met, enter remediation loop

**There is NO "conditional pass", NO "pass with warnings", NO "partial pass".**

If you find yourself wanting to write "conditional pass" or "mostly complete", the phase has **FAILED**.

### 2. Evidence-Based Verification

Every pass MUST include:
- Exact commands run and their output
- Specific file:line references proving implementation
- Count of tests/functions/etc. created
- Explicit statement of what was verified and how

"Trust but verify" is wrong. **Verify, don't trust.**

### 3. Implementation Phases Have Zero Tolerance for Placeholders

In any phase marked as "Implementation" (not "Stub"):
- `unimplemented!()` = **FAIL**
- `todo!()` = **FAIL**
- `// TODO:` = **FAIL**
- `panic!("not yet")` = **FAIL**
- Empty function bodies returning defaults = **FAIL** (unless spec explicitly allows)
- Placeholder strings like `"placeholder response"` = **FAIL**

### 4. Prerequisite Chain is Mandatory

Phase N cannot start until:
- Phase N-1 completion marker exists
- Phase N-1 verification passed (not conditional)
- Phase N-1 evidence file is complete

If prerequisites are not met, **do not start the phase**. Remediate the previous phase first.

---

## Todo List Structure

When coordinating a plan, create todos with this exact structure:

```json
[
  { 
    "id": "p01", 
    "content": "Phase 01: [Name]", 
    "status": "pending", 
    "priority": "high",
    "subtasks": [
      { "id": "p01-exec", "content": "Execute phase [rustexpert]" },
      { "id": "p01-verify", "content": "Verify evidence file exists" }
    ]
  },
  { 
    "id": "p01a", 
    "content": "Phase 01a: [Name] Verification", 
    "status": "pending", 
    "priority": "high",
    "subtasks": [
      { "id": "p01a-prereq", "content": "Check P01 evidence exists with PASS" },
      { "id": "p01a-exec", "content": "Execute verification [rustexpert]" },
      { "id": "p01a-record", "content": "Record verdict in evidence file" }
    ]
  }
]
```

### Subagent Selection Guide

Specify subagent in subtask content using `[subagent-name]` suffix:

| Task Type | Subagent | When to Use |
|-----------|----------|-------------|
| Rust implementation | `[rustexpert]` | Any Rust code writing, cargo commands, Rust testing |
| Documentation | `[default]` | Markdown files, analysis, specifications |
| Verification | `[rustexpert]` | Placeholder detection, build checks, test execution |
| Web/Frontend | `[webexpert]` | If applicable to project |
| Coordination | `[default]` | Todo management, prerequisite checks |

### Status Transitions

```
pending --> in_progress --> completed  (if verification returns PASS)
pending --> in_progress --> pending    (if verification returns FAIL, enter remediation)
```

**CRITICAL RULES:**
- NEVER mark `completed` if verification returned FAIL
- NEVER start Phase N if Phase N-1 evidence file is missing
- NEVER start Phase N if Phase N-1 evidence shows FAIL verdict
- If stuck in remediation loop (3 attempts), escalate to human

### Prerequisite Check Template

Before dispatching any phase, run:

```bash
# Check 1: Does previous completion evidence exist?
PREV_PHASE=$((CURRENT_PHASE - 1))
ls project-plans/[feature]/plan/.completed/P${PREV_PHASE}.md 2>/dev/null
ls project-plans/[feature]/plan/.completed/P${PREV_PHASE}A.md 2>/dev/null

# Check 2: Does evidence show PASS verdict?
grep "^## Verdict: PASS" project-plans/[feature]/plan/.completed/P${PREV_PHASE}A.md

# If either check fails: DO NOT DISPATCH. Remediate previous phase first.
```

---

## Phase Execution Protocol

### Step 1: Check Prerequisites

Before dispatching any phase:

```bash
# Does previous completion marker exist?
ls project-plans/[feature]/plan/.completed/P[N-1].md
ls project-plans/[feature]/plan/.completed/P[N-1]A.md

# Does evidence file exist and contain PASS?
grep "VERDICT: PASS" project-plans/[feature]/plan/.completed/P[N-1]A.md
```

If either check fails: **DO NOT PROCEED**. Remediate phase N-1.

### Step 2: Dispatch to Subagent

Use this prompt template for IMPLEMENTATION phases:

```
Execute Phase [NN]: [Phase Name] for plan [PLAN-ID].

=== READ FIRST ===
- project-plans/[feature]/plan/[NN]-[name].md (phase requirements)
- [any pseudocode or spec files referenced in phase]

=== IMPLEMENTATION REQUIREMENTS ===
[Paste specific requirements from the phase file]

=== CRITICAL: ZERO TOLERANCE FOR PLACEHOLDERS ===
THIS IS AN IMPLEMENTATION PHASE. The following are COMPLETE AND UTTER FAILURE:

- `unimplemented!()` anywhere in delivered code = FAIL
- `todo!()` anywhere in delivered code = FAIL
- `// TODO:` comments = FAIL
- `// FIXME:` comments = FAIL
- Placeholder strings like "not yet implemented" = FAIL
- Empty function bodies that should do real work = FAIL

You MUST actually implement the functionality. "Stub it for now" is NOT acceptable.

=== VERIFICATION YOU MUST PERFORM BEFORE REPORTING DONE ===

1. Build check:
   cargo build --all-targets
   # Must pass with 0 errors

2. Test check:
   cargo test [relevant-test-pattern]
   # Must pass with 0 failures

3. Placeholder detection (MOST IMPORTANT):
   grep -rn "unimplemented!" src/[relevant-paths]
   grep -rn "todo!" src/[relevant-paths]
   grep -rn "// TODO\|// FIXME" src/[relevant-paths]
   grep -rn "placeholder\|not yet implemented" src/[relevant-paths]
   # ALL must return NO MATCHES

If ANY grep returns matches, your implementation is INCOMPLETE. Fix it before reporting.

=== DELIVERABLES ===
1. Working implementation with NO placeholders
2. All specified tests passing
3. Evidence file at: project-plans/[feature]/plan/.completed/P[NN].md

=== REPORT FORMAT ===
Report one of:
- PASS: [summary of what was implemented, grep outputs showing no placeholders]
- FAIL: [exactly what failed and why]

DO NOT report "conditional pass" or "mostly done". Those are FAIL.
```

Use this prompt template for STUB phases (where placeholders ARE allowed):

```
Execute Phase [NN]: [Phase Name] (STUB PHASE) for plan [PLAN-ID].

=== READ FIRST ===
- project-plans/[feature]/plan/[NN]-[name].md

=== STUB REQUIREMENTS ===
Create code skeleton that COMPILES but may not fully function yet.

In stub phases, the following ARE ALLOWED:
- `unimplemented!("description")` for methods to be implemented later
- Returning empty/default values (Vec::new(), None, etc.)

=== VERIFICATION ===
cargo build --all-targets
# Must compile with 0 errors

=== DELIVERABLES ===
1. Compiling code skeleton
2. Evidence file at: project-plans/[feature]/plan/.completed/P[NN].md
```

### Step 3: Verification Phase

After implementation, dispatch verification:

```
Execute Phase [NN]a: [Phase Name] Verification for plan [PLAN-ID].

=== YOUR ROLE ===
You are a SKEPTICAL AUDITOR. Your job is to VERIFY, not implement.
Assume nothing works until you have EVIDENCE it works.

=== VERIFICATION PROTOCOL ===
1. Run every check command and RECORD EXACT OUTPUT (not summaries)
2. READ the actual code (not just check file existence)
3. TRACE at least one complete code path through the implementation
4. VERIFY tests would fail if implementation was removed/broken

=== MANDATORY PLACEHOLDER DETECTION (FOR IMPLEMENTATION PHASES) ===
Run ALL of these and paste EXACT output:

```bash
# Check 1: unimplemented! macro
$ grep -rn "unimplemented!" src/[relevant-paths]
[PASTE EXACT OUTPUT HERE]
# Expected: (no output) -- if ANY output, FAIL

# Check 2: todo! macro
$ grep -rn "todo!" src/[relevant-paths]
[PASTE EXACT OUTPUT HERE]
# Expected: (no output) -- if ANY output, FAIL

# Check 3: TODO/FIXME comments
$ grep -rn "// TODO\|// FIXME\|// HACK\|// STUB" src/[relevant-paths]
[PASTE EXACT OUTPUT HERE]
# Expected: (no output) -- if ANY output, FAIL

# Check 4: Placeholder strings
$ grep -rn "placeholder\|not yet implemented\|will be implemented" src/[relevant-paths]
[PASTE EXACT OUTPUT HERE]
# Expected: (no output) -- if ANY output, FAIL
```

**IF ANY GREP RETURNS MATCHES: STOP. VERDICT IS FAIL. DO NOT PROCEED TO OTHER CHECKS.**

=== SEMANTIC VERIFICATION ===
After placeholder detection passes, verify the code actually works:

1. Build verification:
   $ cargo build --all-targets 2>&1 | tail -5
   [PASTE OUTPUT]

2. Test verification:
   $ cargo test [relevant-tests] 2>&1 | grep -E "^test|passed|failed"
   [PASTE OUTPUT]

3. Code inspection (describe what you found):
   - File: [path]
   - Function: [name]
   - What it does: [your description after reading the code]
   - Does it satisfy requirement [REQ-XXX]? [YES/NO with explanation]

=== VERDICT RULES ===
- If ALL checks pass with recorded evidence: VERDICT: PASS
- If ANY check fails: VERDICT: FAIL (list which checks failed and why)

**THERE IS NO "CONDITIONAL PASS". THERE IS NO "PARTIAL PASS". THERE IS NO "PASS WITH WARNINGS".**
**IF YOU WANT TO WRITE ANY OF THOSE, THE VERDICT IS FAIL.**

=== CREATE EVIDENCE FILE ===
Create: project-plans/[feature]/plan/.completed/P[NN]A.md

The evidence file MUST contain the exact template from PLAN-TEMPLATE.md with all command outputs filled in.
```
Execute Phase [NN]a: [Phase Name] Verification for plan [PLAN-ID].

YOUR JOB IS TO VERIFY, NOT IMPLEMENT.

VERIFICATION CHECKLIST:
1. STRUCTURAL CHECKS:
   - [ ] Files exist: [list expected files]
   - [ ] Markers present: grep -c "@plan [PLAN-ID].P[NN]" [files]
   - [ ] Build passes: cargo build --all-targets

2. PLACEHOLDER DETECTION (MANDATORY):
   Run these commands and report EXACT output:
   ```
   grep -rn "unimplemented!" src/[paths]
   grep -rn "todo!" src/[paths]
   grep -rn "// TODO\|// FIXME" src/[paths]
   grep -rn "placeholder\|not yet implemented" src/[paths]
   ```
   If ANY match found in implementation code: FAIL

3. SEMANTIC CHECKS:
   - [ ] Read the actual code in [files]
   - [ ] Verify it does what the requirement says
   - [ ] Trace data flow: input → processing → output
   - [ ] Run tests and verify they exercise real behavior

4. EVIDENCE COLLECTION:
   For each check, record:
   - Command run
   - Exact output (truncated if >20 lines, but include totals)
   - Your interpretation

VERDICT RULES:
- If ALL checks pass with evidence: VERDICT: PASS
- If ANY check fails: VERDICT: FAIL (explain which and why)
- There is NO conditional pass. PASS or FAIL only.

Create evidence file: project-plans/[feature]/plan/.completed/P[NN]A.md

The evidence file MUST contain:
```markdown
# Phase [NN]a Verification Results

## Verdict: [PASS|FAIL]

## Structural Checks
- Files exist: [YES/NO with list]
- Markers present: [count and locations]
- Build: [PASS/FAIL with any errors]

## Placeholder Detection
Command: grep -rn "unimplemented!" src/[paths]
Output: [exact output or "no matches"]

Command: grep -rn "todo!" src/[paths]
Output: [exact output or "no matches"]

[repeat for all grep commands]

## Semantic Verification
- Code does what requirement says: [YES/NO with explanation]
- Data flow traced: [description]
- Tests exercise real behavior: [YES/NO with evidence]

## Evidence Summary
[Bullet points of what was verified and how]

## Blocking Issues (if FAIL)
[List exactly what must be fixed]
```
```

### Step 4: Remediation Loop

If verification returns FAIL:

1. **DO NOT mark phase complete**
2. **DO NOT proceed to next phase**
3. Record the failure in execution tracker
4. Dispatch remediation task:

```
REMEDIATION REQUIRED for Phase [NN]: [Phase Name]

Previous verification FAILED with these issues:
[paste blocking issues from verification]

Your task:
1. Fix ALL listed issues
2. Do NOT introduce new issues
3. Run verification checks yourself before reporting done
4. Report what you fixed and evidence it's fixed

This is attempt [N] of maximum 3 remediation attempts.
```

5. After remediation, re-run verification
6. Maximum 3 remediation attempts before escalating

### Step 5: Coordinator Inspection of Results

**Before accepting a subagent's result, the coordinator MUST:**

1. **Check for equivocation words:**
   - Search result for: "conditional", "partial", "mostly", "almost", "nearly", "expected failure"
   - If ANY found: **Reject the result, enter remediation**

2. **Verify evidence is concrete:**
   - Are there actual command outputs (not just "I ran X")?
   - Are there file:line references (not just "the file exists")?
   - Are there counts/metrics (not just "tests pass")?
   - If evidence is vague: **Reject the result, demand specifics**

3. **For implementation phases, verify no placeholders:**
   - Check that grep outputs for `unimplemented!`, `todo!`, etc. are shown
   - Check that those outputs are empty (no matches)
   - If placeholders were found but subagent said PASS: **Override to FAIL**

4. **Read the verdict:**
   - Must be exactly "PASS" or "FAIL"
   - "Conditional pass" = Coordinator overrides to FAIL
   - "Pass with warnings" = Coordinator overrides to FAIL
   - Missing verdict = FAIL

### Step 6: Record Completion

Only after verification PASSES AND coordinator inspection passes:

1. Verify evidence file exists at `project-plans/[feature]/plan/.completed/P[NN]A.md`
2. Verify evidence file contains "Verdict: PASS" (not conditional)
3. Update todo status to `completed`
4. Update execution tracker
5. Proceed to next phase

**If any step fails, do NOT mark completed. Enter remediation.**

---

## Execution Tracker Format

Maintain `project-plans/[feature]/execution-tracker.md`:

```markdown
# Execution Tracker: [PLAN-ID]

## Status Summary
- Total Phases: [N]
- Completed: [N]
- In Progress: [N]  
- Remaining: [N]
- Current Phase: P[NN]

## Phase Status

| Phase | Status | Attempts | Completed | Verified | Evidence |
|-------|--------|----------|-----------|----------|----------|
| P01 | [OK] PASS | 1 | 2025-01-28 | 2025-01-28 | P01A.md |
| P01a | [OK] PASS | 1 | 2025-01-28 | N/A | N/A |
| P02 | [ERROR] FAIL | 2 | - | - | - |
| P02a |  PENDING | - | - | - | - |

## Remediation Log

### P02 Attempt 1 (2025-01-28)
- Issue: `unimplemented!()` found in chat_service.rs
- Action: Subagent instructed to implement
- Result: Still had placeholder response string

### P02 Attempt 2 (2025-01-28)
- Issue: Placeholder string "not yet implemented"
- Action: Subagent instructed to use real implementation
- Result: PENDING VERIFICATION

## Blocking Issues
- P02 has failed verification twice
- Root cause: Subagent not understanding "no placeholders" requirement
```

---

## Prompt Templates for Common Situations

### Implementation Phase Prompt

```
Execute Phase [NN]: [Name] - IMPLEMENTATION PHASE

CRITICAL UNDERSTANDING:
This is an IMPLEMENTATION phase, not a stub phase. 
The code you write MUST actually work. 
Placeholders are COMPLETE AND UTTER FAILURE.

Specifically:
- `unimplemented!()` = FAILURE, DO NOT USE
- `todo!()` = FAILURE, DO NOT USE
- `// TODO:` = FAILURE, DO NOT USE
- Returning placeholder strings = FAILURE
- Empty implementations = FAILURE

READ: project-plans/[feature]/plan/[NN]-[name].md

IMPLEMENT:
[specific tasks]

BEFORE REPORTING DONE:
1. Run: cargo build --all-targets (must pass)
2. Run: cargo test [tests] (must pass)  
3. Run: grep -rn "unimplemented!\|todo!" src/[path] (must return NOTHING)

If grep finds ANYTHING, your implementation is incomplete. Fix it.

Create: project-plans/[feature]/plan/.completed/P[NN].md
```

### Verification Phase Prompt

```
Execute Phase [NN]a: Verification

YOUR ROLE: Skeptical auditor. Assume nothing works until proven.

VERIFICATION PROTOCOL:
1. Run every check command and record exact output
2. Read the actual code, don't just check file existence
3. Trace at least one complete code path
4. Verify tests would fail if implementation removed

MANDATORY CHECKS:
[list specific checks for this phase]

PLACEHOLDER DETECTION (run all, report exact output):
```bash
grep -rn "unimplemented!" src/[relevant-paths]
grep -rn "todo!" src/[relevant-paths]
grep -rn "// TODO\|// FIXME\|// HACK" src/[relevant-paths]
grep -rn "placeholder\|not yet" src/[relevant-paths]
```

If ANY of these return matches in implementation code: FAIL

VERDICT:
- PASS: All checks pass with recorded evidence
- FAIL: Any check fails (specify which)

NO CONDITIONAL PASS EXISTS. DO NOT INVENT ONE.

Create: project-plans/[feature]/plan/.completed/P[NN]A.md with full evidence
```

### Remediation Phase Prompt

```
REMEDIATION REQUIRED: Phase [NN] failed verification

FAILURE REASONS:
[paste from verification]

YOUR TASK:
1. Fix each listed issue
2. Verify your fix with the same checks that failed
3. Do not introduce new issues
4. Report exactly what you changed

EVIDENCE REQUIRED:
For each issue fixed, show:
- What the problem was
- What you changed (file:line)
- Verification that it's fixed (command + output)

This is remediation attempt [N/3].
```

---

## Content Inspection Checklist

**Coordinators MUST inspect actual deliverables, not just accept subagent claims.**

### For Implementation Phases

After subagent reports completion, coordinator should:

```bash
# 1. Verify files changed (not just created)
git diff --stat src/[relevant-paths]
# Should show meaningful changes, not empty files

# 2. Spot-check for placeholders yourself
grep -rn "unimplemented!\|todo!\|// TODO\|placeholder" src/[relevant-paths]
# Should return nothing

# 3. Verify tests exist and run
cargo test [relevant-pattern] --no-run
cargo test [relevant-pattern] 2>&1 | head -20
# Should compile and run

# 4. Read at least ONE key function
# Open src/[path]/[file].rs
# Find the main function that should do work
# Does it actually DO something or just return defaults?
```

### Red Flags in Code Inspection

**Hollow implementations (code exists but does nothing useful):**
```rust
// RED FLAG: Returns placeholder string
fn send_message(&self, _text: &str) -> String {
    "This is a placeholder response".to_string()  // FAIL
}

// RED FLAG: Returns empty collection
fn get_available_tools(&self) -> Vec<Tool> {
    Vec::new()  // FAIL if tools should actually be fetched
}

// RED FLAG: Ignores inputs
fn process(&self, input: Data) -> Result<Output, Error> {
    let _ = input;  // Ignoring input = not processing it
    Ok(Output::default())  // FAIL
}

// RED FLAG: Deferred with comment
fn important_function(&self) -> Result<(), Error> {
    // TODO: Actually implement this
    Ok(())  // FAIL
}
```

**What REAL implementation looks like:**
```rust
// REAL: Actually uses the input
fn send_message(&self, text: &str) -> Result<String, Error> {
    let request = self.build_request(text)?;  // Uses text
    let response = self.client.send(request).await?;  // Does work
    Ok(response.content)  // Returns real result
}

// REAL: Actually fetches tools
fn get_available_tools(&self) -> Result<Vec<Tool>, Error> {
    let tools = self.mcp_client.list_tools().await?;  // Fetches
    Ok(tools.into_iter().map(|t| t.into()).collect())  // Transforms
}
```

---

## Anti-Patterns to Reject

### "Conditional Pass"
```
VERDICT: CONDITIONAL PASS - tests pass but 3 have unimplemented stubs
```
**WRONG.** This is a FAIL. Fix the stubs.

### "Pass with Warnings"
```
VERDICT: PASS (with warnings about missing error handling)
```
**WRONG.** Either the error handling is required (FAIL) or it's not (true PASS). Decide.

### "Mostly Complete"
```
Implementation is 90% done, just needs LLM integration
```
**WRONG.** 90% done = not done = FAIL. The 10% is often the actual feature.

### "Expected Failures"
```
63 tests fail but that's expected because they test stubs
```
**WRONG in implementation phases.** If tests fail, either:
- Tests are wrong (fix tests), or
- Implementation is incomplete (fix implementation)

"Expected failure" is only valid in TDD phases where tests are written before implementation.

### Skipping Prerequisites
```
Starting P09 even though P08 verification hasn't completed
```
**WRONG.** Never skip. Fix P08 first.

### Hollow Implementation
```
Code exists and compiles, but functions return defaults/placeholders
```
**WRONG.** Coordinator must inspect code content, not just verify it compiles.

---

## Escalation Protocol

After 3 failed remediation attempts:

1. Stop automated execution
2. Document the root cause
3. Escalate to human with:
   - What was attempted
   - Why it keeps failing
   - Suggested plan modification

Do NOT keep retrying the same failing approach.

---

## Completion Checklist

Plan execution is complete when:

- [ ] All phases have status PASS in tracker (not conditional, not partial)
- [ ] All verification evidence files exist at `project-plans/[feature]/plan/.completed/`
- [ ] All evidence files contain "Verdict: PASS" (grep to verify)
- [ ] `cargo build --all-targets` passes with 0 errors
- [ ] `cargo test` passes with 0 failures
- [ ] `cargo clippy` passes with 0 warnings (or explicitly allowed)
- [ ] `grep -rn "unimplemented!\|todo!" src/` returns NO MATCHES
- [ ] Feature actually works when manually tested (not just compiles)
- [ ] Code inspection confirms real implementation (not hollow/placeholder)

**If ANY checkbox is unchecked, the plan is NOT COMPLETE.**

---

## Quick Reference: The Rules

### The Only Two Verdicts
- **PASS**: All criteria met, evidence recorded, no placeholders, code works
- **FAIL**: Anything else

### Forbidden Phrases (auto-FAIL if seen)
- "Conditional pass"
- "Pass with warnings"
- "Partial pass"
- "Mostly complete"
- "Expected failures" (in impl phases)
- "Will be implemented"
- "Placeholder for now"

### Coordinator Must-Do Before Accepting Result
1. Check for equivocation words in result
2. Verify grep outputs for placeholders are EMPTY
3. Verify evidence file exists with PASS verdict
4. Spot-check at least one function for hollow implementation

### When to Override Subagent to FAIL
- Subagent says "conditional pass" -> FAIL
- Subagent says "pass" but grep found placeholders -> FAIL
- Subagent says "pass" but code inspection shows hollow impl -> FAIL
- Subagent says "pass" but tests actually failed -> FAIL
- Evidence file is missing or incomplete -> FAIL

---

## Related Documents

- [PLAN.md](./PLAN.md) - How to create plans
- [PLAN-TEMPLATE.md](./PLAN-TEMPLATE.md) - Phase structure template
- [goodtests.md](./goodtests.md) - How to write proper tests
