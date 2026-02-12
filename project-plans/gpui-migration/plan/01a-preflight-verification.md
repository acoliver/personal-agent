# Phase 01a: Preflight Verification Checklist

## Phase ID

`PLAN-20250128-GPUI.P01a`

## Prerequisites

- Phase 01 evidence file exists: `project-plans/gpui-migration/plan/.completed/P01.md`
- Phase 01 shows no blocking issues

---

## Coordinator Verification Protocol

This phase is for the **coordinator** to verify Phase 01 evidence.

### Step 1: Evidence File Exists

```bash
ls project-plans/gpui-migration/plan/.completed/P01.md
cat project-plans/gpui-migration/plan/.completed/P01.md
```

**Check:** File exists and contains all required sections.

### Step 2: GPUI Dependency Verified

In the evidence file, verify:
- [ ] "Added to Cargo.toml: YES"
- [ ] "cargo check result: PASS" (or documented workaround)

If cargo check failed:
- [ ] Specific commit pinned?
- [ ] Error documented?
- [ ] Workaround identified?

### Step 3: Architecture Intact

In the evidence file, verify:
- [ ] All presenters listed (ChatPresenter, HistoryPresenter, SettingsPresenter)
- [ ] EventBus confirmed
- [ ] Services listed
- [ ] "cargo build: PASS"
- [ ] "cargo test: PASS"

### Step 4: NSStatusItem Code Located

In the evidence file, verify:
- [ ] File location documented (should be `src/main_menubar.rs`)
- [ ] Icon path documented (should be `assets/MenuBarIcon.imageset/icon-32.png`)
- [ ] Toggle function identified

### Step 5: ExactoBar Reference Available

In the evidence file, verify:
- [ ] "Directory exists: YES"
- [ ] Key files listed (tray.rs, components/, menu/)

### Step 6: Event Types Documented

In the evidence file, verify:
- [ ] ViewCommand variants listed with count
- [ ] UserEvent variants listed with count
- [ ] ChatEvent variants listed with count

### Step 7: Theme Values Documented

In the evidence file, verify:
- [ ] Background colors listed with values
- [ ] Text colors listed with values

### Step 8: No Blocking Issues

In the evidence file, verify:
- [ ] "Blocking Issues: None" OR issues have documented resolutions

---

## Verdict Decision

### PASS Criteria (ALL must be true)

1. Evidence file exists with all sections
2. GPUI dependency works (or has documented workaround)
3. All presenters and EventBus confirmed
4. NSStatusItem code located
5. ExactoBar reference available
6. Event types documented
7. Theme values documented
8. No unresolved blocking issues

### FAIL Criteria (ANY triggers FAIL)

1. Evidence file missing or incomplete
2. GPUI dependency fails with no workaround
3. Critical architecture components missing
4. Unable to locate NSStatusItem code
5. ExactoBar reference not available
6. Blocking issues without resolution

---

## Evidence Recording

Create: `project-plans/gpui-migration/plan/.completed/P01A.md`

```markdown
# Phase 01a: Preflight Verification Results

## Verdict: [PASS|FAIL]

## Evidence File Review

### P01 Evidence File
- Exists: [YES/NO]
- Complete: [YES/NO]
- Sections missing: [list or "None"]

### GPUI Dependency
- Status: [VERIFIED/NEEDS_WORKAROUND/BLOCKED]
- Notes: [any relevant details]

### Architecture
- ChatPresenter: [FOUND/MISSING]
- HistoryPresenter: [FOUND/MISSING]
- SettingsPresenter: [FOUND/MISSING]
- EventBus: [FOUND/MISSING]
- Build status: [PASS/FAIL]
- Test status: [PASS/FAIL]

### NSStatusItem Integration
- Code located: [YES/NO]
- File: [path]
- Reusable: [YES/NO/PARTIAL]

### ExactoBar Reference
- Available: [YES/NO]
- Key patterns: [list]

### Event Types
- ViewCommand: [count] variants
- UserEvent: [count] variants  
- ChatEvent: [count] variants

### Theme
- Colors documented: [YES/NO]

### Blocking Issues
- Resolved: [list or "N/A"]
- Unresolved: [list or "None"]

## Verdict Justification
[Explain why PASS or FAIL]

## Next Steps
[If PASS: Proceed to Phase 02]
[If FAIL: List required remediation]
```

---

## Failure Recovery

If verification fails:

1. Document specific failures
2. Return to Phase 01 for remediation
3. Do NOT proceed to Phase 02

---

## Next Phase

After P01a completes with PASS:
→ P02: Analysis Phase
