# Phase 11a: Integration Implementation Verification

## Phase ID

`PLAN-20260402-MARKDOWN.P11a`

## Prerequisites

- Required: Phase 11 completed
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P11" src/`
- Expected: Full integration complete, all tests passing

## Verification Commands

### Automated Checks

```bash
# 1. ALL tests pass — the full project
cargo test --lib --tests || exit 1

# 2. P11 markers present
grep -r "@plan:PLAN-20260402-MARKDOWN.P11" src/ | wc -l
# Expected: 3+

# 3. No todo!/unimplemented! in production code
grep -rn "todo!\|unimplemented!" src/ui_gpui/components/message_bubble.rs src/ui_gpui/views/chat_view/render.rs | grep -v "mod tests"
# Expected: 0 matches

# 4. No hardcoded colors
grep -rn "rgb(0x" src/ui_gpui/components/message_bubble.rs src/ui_gpui/components/markdown_content.rs | grep -v "mod tests"
# Expected: 0 matches

# 5. No debug code
grep -rn "println!\|dbg!" src/ui_gpui/components/message_bubble.rs src/ui_gpui/views/chat_view/render.rs src/ui_gpui/components/markdown_content.rs
# Expected: 0 matches

# 6. Clippy clean
cargo clippy --all-targets -- -D warnings || exit 1

# 7. Formatted
cargo fmt --all -- --check || exit 1

# 8. Store not modified
git diff HEAD -- src/ui_gpui/app_store.rs | wc -l
# Expected: 0

# 9. No mdstream references
grep -rn "mdstream" src/ Cargo.toml | grep -v "mod tests" | grep -v ".md"
# Expected: 0 matches

# 10. Full test count
grep -rc "#\[test\]\|#\[gpui::test\]" src/ui_gpui/components/markdown_content.rs src/ui_gpui/components/message_bubble.rs 2>/dev/null
# Expected: 51+ total (36 parser + 12 renderer + 15 integration)
```

### Structural Verification Checklist

- [ ] All phase markers intact (P03 through P11)
- [ ] All tests pass
- [ ] No todo!/unimplemented! in production code
- [ ] No hardcoded colors
- [ ] No debug code
- [ ] Clippy clean
- [ ] Store/presenter layers clean
- [ ] No mdstream dependency

### Semantic Verification: Full Feature Walkthrough

1. **Parse pipeline works**:
   - [ ] `parse_markdown_blocks("# Hello\n\n**bold**")` returns [Heading, Paragraph]
   - [ ] Verified by P04 tests passing

2. **Render pipeline works**:
   - [ ] `render_markdown("# Hello")` returns non-empty elements
   - [ ] Verified by P07 tests passing

3. **Integration works**:
   - [ ] `AssistantBubble::new("**bold**").into_any_element()` renders markdown
   - [ ] `render_assistant_message(msg, false)` delegates to AssistantBubble
   - [ ] Click-to-copy behavior correct per link content
   - [ ] Verified by P09 tests passing

4. **Table-cell link detection works (REQ-MD-INTEGRATE-024)**:
   - [ ] `has_any_links` detects links in Paragraph/Heading blocks via `links` field
   - [ ] `has_any_links` recurses into BlockQuote children
   - [ ] `has_any_links` recurses into List items
   - [ ] `has_any_links` inspects Table header and body cell spans for `link_url`
   - [ ] Both `links` field (block-level) and `link_url` (span-level in tables) are checked

5. **Old rendering paths eliminated**:
   - [ ] `render_assistant_message()` no longer has raw `.child(content)` calls
   - [ ] `AssistantBubble::into_element()` no longer has raw `.child(content_text)` calls
   - [ ] Single rendering path through markdown pipeline

6. **Feature reachable by users**:
   - [ ] Chat view → render_chat_area → render_assistant_message → AssistantBubble → markdown
   - [ ] Chat view → streaming → AssistantBubble → markdown

### End-to-End Integration Points Verified

- [ ] `markdown_content.rs` → `message_bubble.rs` (import/call verified)
- [ ] `message_bubble.rs` → `render.rs` (AssistantBubble used by render_assistant_message)
- [ ] `render.rs` → `mod.rs` (render_assistant_message called by render_chat_area)
- [ ] `mod.rs` → GPUI view (elements displayed in chat scroll view)

## Success Criteria

- All tests pass (full project)
- Integration complete end-to-end
- No stubs remaining
- Old rendering paths eliminated
- Clippy and fmt clean
- Store/presenter/mdstream isolation verified
- Table-cell recursive link detection verified

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P11a.md`
