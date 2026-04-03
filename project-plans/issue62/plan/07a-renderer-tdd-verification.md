# Phase 07a: Renderer TDD Verification

## Phase ID

`PLAN-20260402-MARKDOWN.P07a`

## Prerequisites

- Required: Phase 07 completed
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P07" src/ui_gpui/components/markdown_content.rs`
- Expected: Renderer tests added to test module

## Verification Commands

### Automated Checks

```bash
# 1. P07 markers present
grep -c "@plan:PLAN-20260402-MARKDOWN.P07" src/ui_gpui/components/markdown_content.rs
# Expected: 12+

# 2. RENDER requirement markers
grep -c "@requirement:REQ-MD-RENDER" src/ui_gpui/components/markdown_content.rs
# Expected: 12+

# 3. Compiles with test targets
cargo build --all-targets || exit 1

# 4. Parser tests still pass
cargo test --lib -- markdown_content::tests::test_parse || exit 1
cargo test --lib -- markdown_content::tests::test_safe_url || exit 1

# 5. Renderer tests fail naturally
cargo test --lib -- markdown_content::tests::test_render 2>&1 | tail -10
# Expected: Failures

# 6. No reverse testing
grep -rn "should_panic" src/ui_gpui/components/markdown_content.rs && echo "FAIL"

# 7. Count all tests
grep -c "#\[test\]\|#\[gpui::test\]" src/ui_gpui/components/markdown_content.rs
# Expected: 48+ (36 parser + 12 renderer)
```

### Structural Verification Checklist

- [ ] All previous phase markers intact
- [ ] 12+ renderer tests with P07 markers
- [ ] Tests compile
- [ ] Parser tests unbroken
- [ ] Renderer tests fail naturally
- [ ] No reverse testing

## Success Criteria

- 12+ renderer tests that compile and fail
- Parser tests unaffected
- No reverse testing patterns

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P07a.md`
