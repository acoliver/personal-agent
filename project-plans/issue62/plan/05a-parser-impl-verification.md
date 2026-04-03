# Phase 05a: Parser Implementation Verification

## Phase ID

`PLAN-20260402-MARKDOWN.P05a`

## Prerequisites

- Required: Phase 05 completed
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P05" src/ui_gpui/components/markdown_content.rs`
- Expected: All P04 tests passing, implementation code with pseudocode references

## Verification Commands

### Automated Checks

```bash
# 1. All tests pass
cargo test --lib -- markdown_content || exit 1

# 2. P05 markers present
grep -c "@plan:PLAN-20260402-MARKDOWN.P05" src/ui_gpui/components/markdown_content.rs
# Expected: 5+

# 3. Pseudocode references present
grep -c "@pseudocode" src/ui_gpui/components/markdown_content.rs
# Expected: 5+

# 4. No todo!/unimplemented! in production code
grep -rn "todo!\|unimplemented!" src/ui_gpui/components/markdown_content.rs | grep -v "mod tests" | grep -v "#\[cfg(test)\]"
# Expected: 2 remaining (blocks_to_elements and render_markdown — not yet implemented)

# 5. No debug code
grep -rn "println!\|dbg!" src/ui_gpui/components/markdown_content.rs
# Expected: No matches

# 6. Formatted
cargo fmt --all -- --check || exit 1

# 7. Clippy snapshot (non-gating in P05a per deferred lint policy)
# Strict clippy gate is enforced in P08a.
cargo clippy --all-targets -- -D warnings || true

# 8. No test modifications (compare P04 test count with current)
grep -c "#\[test\]" src/ui_gpui/components/markdown_content.rs
# Expected: Same count as P04
```

### Structural Verification Checklist

- [ ] All P03, P04 markers still present
- [ ] P05 markers on implementation functions
- [ ] Pseudocode line references on implementation functions
- [ ] All tests pass
- [ ] No tests modified
- [ ] No debug code
- [ ] fmt passes
- [ ] Clippy output captured for visibility (non-gating in P05a)
- [ ] `parse_markdown_blocks()` fully implemented (no todo!())
- [ ] `is_safe_url()` fully implemented (no todo!())
- [ ] `blocks_to_elements()` still `todo!()` (Phase 08)
- [ ] `render_markdown()` still `todo!()` (Phase 08)

### Semantic Verification

- [ ] `parse_markdown_blocks("# Hello")` returns `Heading { level: 1, ... }` (verified by test)
- [ ] `parse_markdown_blocks("**bold**")` returns spans with `bold: true` (verified by test)
- [ ] `parse_markdown_blocks("")` returns empty vec (verified by test)
- [ ] `is_safe_url("https://example.com")` returns true (verified by test)
- [ ] `is_safe_url("javascript:alert(1)")` returns false (verified by test)
- [ ] Implementation follows pseudocode structure (manual review)

## Success Criteria

- All parser tests pass
- Implementation follows pseudocode
- No remaining `todo!()` in parse_markdown_blocks or is_safe_url
- fmt clean
- Strict clippy gate deferred to P08a

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P05a.md`
