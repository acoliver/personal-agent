# Phase 09a: Integration TDD Verification

## Phase ID

`PLAN-20260402-MARKDOWN.P09a`

## Prerequisites

- Required: Phase 09 completed
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P09" src/`
- Expected: Integration test module added to message_bubble.rs (or appropriate file)

## Verification Commands

### Automated Checks

```bash
# 1. P09 markers present
grep -r "@plan:PLAN-20260402-MARKDOWN.P09" src/ | wc -l
# Expected: 15+

# 2. Integration requirement markers
grep -r "@requirement:REQ-MD-INTEGRATE" src/ | wc -l
# Expected: 15+

# 3. Compiles with test targets
cargo build --all-targets || exit 1

# 4. Parser + renderer tests still pass
cargo test --lib -- markdown_content::tests || exit 1

# 5. No reverse testing
grep -rn "should_panic" src/ui_gpui/components/message_bubble.rs && echo "FAIL"

# 6. Count integration tests
grep -c "#\[test\]\|#\[gpui::test\]" src/ui_gpui/components/message_bubble.rs
# Expected: 15+

# 7. Isolation tests present
grep "mdstream\|app_store\|presenter" src/ui_gpui/components/message_bubble.rs | grep "test" | head -5
# Expected: test references for isolation checks
```

### Structural Verification Checklist

- [ ] All previous phase markers intact
- [ ] P09 markers on all integration tests
- [ ] 15+ tests created
- [ ] Tests compile
- [ ] No reverse testing
- [ ] Click-to-copy tests present (3 variants: no-links, has-links, streaming)
- [ ] Store/presenter isolation tests present
- [ ] mdstream isolation test present
- [ ] Production code in `message_bubble.rs` and `render.rs` NOT modified (only test module added)

## Success Criteria

- 15+ integration tests that compile
- Previous tests unaffected
- Click precedence gate tests present
- No production code changes yet (tests only)

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P09a.md`
