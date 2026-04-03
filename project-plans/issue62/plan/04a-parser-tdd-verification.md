# Phase 04a: Parser TDD Verification

## Phase ID

`PLAN-20260402-MARKDOWN.P04a`

## Prerequisites

- Required: Phase 04 completed
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P04" src/ui_gpui/components/markdown_content.rs`
- Expected files from previous phase: Test module added to `markdown_content.rs`

## Verification Commands

### Automated Checks

```bash
# 1. Plan markers for P04 exist
grep -c "@plan:PLAN-20260402-MARKDOWN.P04" src/ui_gpui/components/markdown_content.rs
# Expected: 36+

# 2. P03 markers still present
grep -c "@plan:PLAN-20260402-MARKDOWN.P03" src/ui_gpui/components/markdown_content.rs
# Expected: 6+

# 3. Compiles with test targets
cargo build --all-targets || exit 1

# 4. No reverse testing
grep -rn "should_panic" src/ui_gpui/components/markdown_content.rs && echo "FAIL"
# Expected: No matches

# 5. Tests exist and fail naturally
cargo test --lib -- markdown_content 2>&1 | tail -20
# Expected: Multiple test failures (from todo!() panic)

# 6. Count tests
grep -c "#\[test\]" src/ui_gpui/components/markdown_content.rs
# Expected: 36+

# 7. Every test has a requirement marker
# Count tests vs requirement markers — should be roughly equal
TESTS=$(grep -c "#\[test\]" src/ui_gpui/components/markdown_content.rs)
REQS=$(grep -c "@requirement:" src/ui_gpui/components/markdown_content.rs)
echo "Tests: $TESTS, Requirement markers: $REQS"
# Expected: REQS >= TESTS (some items from P03 also have markers)
```

### Structural Verification Checklist

- [ ] P03 markers intact
- [ ] P04 markers present on all new tests
- [ ] 36+ tests created (including coverage for previously-missing SHOULD/COULD requirements)
- [ ] No `#[should_panic]` found
- [ ] `cargo build --all-targets` succeeds
- [ ] Tests fail with todo!() panic (not compile errors)
- [ ] Test names are descriptive and behavioral

### Semantic Verification

- [ ] At least one test per block-level construct (Paragraph, Heading, CodeBlock, BlockQuote, List, Table, ThematicBreak)
- [ ] At least one test per inline style (bold, italic, bold-italic, strikethrough, code, link, task list markers)
- [ ] At least one test per fallback (image, footnote definition, footnote reference, HTML block, inline HTML, script stripping, inline math, display math, metadata skip, malformed HTML, definition list)
- [ ] URL validation tests cover: valid http/https, dangerous schemes, malformed input, edge cases
- [ ] Empty/whitespace input tests exist
- [ ] Nested structure tests exist (list of lists, blockquote with list)
- [ ] All tests assert concrete expected values (not just `is_some()` or `is_ok()`)
- [ ] REQ-MD-PARSE-026 (task list marker) test present
- [ ] REQ-MD-PARSE-041 (footnote definition fallback) test present
- [ ] REQ-MD-PARSE-042 (footnote reference literal) test present
- [ ] REQ-MD-PARSE-047 (display math to code block) test present
- [ ] REQ-MD-PARSE-049 (metadata block skip) test present
- [ ] REQ-MD-PARSE-051 (definition list fallback) test present

## Success Criteria

- 36+ tests that compile and fail naturally
- Complete requirement coverage for REQ-MD-PARSE and REQ-MD-SEC groups (including all SHOULD/COULD)
- No reverse testing

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P04a.md`
