# Phase 11: Integration Implementation

## Phase ID

`PLAN-20260402-MARKDOWN.P11`

## Prerequisites

- Required: Phase 10a completed (integration stub verified)
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P10" src/`
- Expected files from previous phase: Integration tests from P09, stubs from P10
- Preflight verification: Phase 0.5 completed

## Requirements Implemented (Expanded)

This phase completes the integration to make all P09 tests pass. The P10 stubs are refined into full integration code.

### REQ-MD-INTEGRATE-001: Canonical Rendering Owner

**Full Text**: The system shall route all assistant content — both completed messages and streaming messages — through a single canonical markdown rendering owner.
**Behavior**:
- GIVEN: Any assistant message (completed or streaming)
- WHEN: Rendered
- THEN: Goes through `AssistantBubble::into_element()` → `parse_markdown_blocks()` → `blocks_to_elements()`
**Why This Matters**: Eliminates dual rendering paths.

### REQ-MD-INTEGRATE-002: Markdown Rendering in AssistantBubble

**Full Text**: WHEN the assistant bubble renders, the system shall parse markdown from the content text and produce GPUI elements.
**Behavior**: (See P10 for full expansion)

### REQ-MD-INTEGRATE-010: Delegation from render_assistant_message

**Full Text**: The completed-message rendering path shall delegate to `AssistantBubble`.
**Behavior**: (See P10 for full expansion)

### REQ-MD-INTEGRATE-012: Visual Baseline Normalization

**Full Text**: WHEN the refactored rendering path produces output through the assistant bubble, the system shall normalize width, padding, gap, text size, and cursor style to match the existing completed-message visual baseline.
**Behavior**:
- GIVEN: Current `render_assistant_message()` uses `max_w(px(300.0))`, specific padding, bg colors
- WHEN: Refactored to delegate to AssistantBubble
- THEN: AssistantBubble's styling matches the old visual baseline (width, padding, colors, gap)
**Why This Matters**: Prevents visual regressions.

### REQ-MD-INTEGRATE-020 through -024: Click-to-Copy

(See P09 and P10 for full expansions. Implementation makes click-to-copy tests pass.)

### REQ-MD-INTEGRATE-023: Raw Markdown Source in Clipboard

**Full Text**: The click-to-copy handler shall copy the raw markdown source string (not the rendered text) to the system clipboard.
**Behavior**:
- GIVEN: A message with content `"**bold**"`
- WHEN: Clicked (no links, not streaming)
- THEN: Clipboard receives `"**bold**"` (raw source), not `"bold"` (rendered text)

### REQ-MD-INTEGRATE-024: Recursive Link Detection

**Full Text**: The link detection shall recursively inspect the parsed `MarkdownBlock` IR for non-empty `links` fields across any descendant block, including table cells.
**Behavior**:
- GIVEN: A message with links nested inside a list item
- WHEN: Link detection runs
- THEN: `has_links` is true (detection recurses through List items, BlockQuote children, Table cells)
- GIVEN: A message with links inside a table cell
- WHEN: Link detection runs
- THEN: `has_links` is true (detection recurses into Table header cells and body row cells)

## Implementation Tasks

### Files to Modify

#### `src/ui_gpui/components/message_bubble.rs`

- REFINE `AssistantBubble::into_element()` (from P10 stub):
  - Add `/// @plan:PLAN-20260402-MARKDOWN.P11`
  - Implement full markdown pipeline:
    1. Append streaming cursor if `self.is_streaming`
    2. Parse: `let blocks = parse_markdown_blocks(&content_text);`
    3. Link detection: recursive check across all block variants including table cells
    4. Render: `let elements = blocks_to_elements(&blocks);`
    5. Build container div with existing styling (width, padding, bg, rounded corners)
    6. Add `.children(elements)` to the content div
    7. Conditionally add `.on_click()` if `!has_links && !self.is_streaming`
    8. Conditionally add `.cursor_pointer()` same condition

  - Implement recursive `has_any_links()` helper that properly handles ALL block variants including Table cells (REQ-MD-INTEGRATE-024):

    ```rust
    /// @plan:PLAN-20260402-MARKDOWN.P11
    /// @requirement:REQ-MD-INTEGRATE-024
    fn has_any_links(blocks: &[MarkdownBlock]) -> bool {
        blocks.iter().any(|b| match b {
            MarkdownBlock::Paragraph { links, .. }
            | MarkdownBlock::Heading { links, .. } => !links.is_empty(),
            MarkdownBlock::BlockQuote { children } => has_any_links(children),
            MarkdownBlock::List { items, .. } => items.iter().any(|item| has_any_links(item)),
            MarkdownBlock::Table { header, rows, .. } => {
                // REQ-MD-INTEGRATE-024: Recursively inspect table cell content for links.
                // Table cells contain Vec<MarkdownInline> — check each cell's spans
                // for link_url presence.
                let header_has_links = header.iter().any(|cell_spans| {
                    cell_spans.iter().any(|span| span.link_url.is_some())
                });
                let body_has_links = rows.iter().any(|row| {
                    row.iter().any(|cell_spans| {
                        cell_spans.iter().any(|span| span.link_url.is_some())
                    })
                });
                header_has_links || body_has_links
            },
            _ => false,
        })
    }
    ```

    **Key design note on Table cell link detection**: The `MarkdownBlock::Table` variant stores cells as `Vec<MarkdownInline>` (flat span lists), not as nested `MarkdownBlock` trees. Therefore, link detection for table cells must inspect `MarkdownInline::link_url` on each span rather than checking a `links` field. This is different from `Paragraph`/`Heading` which carry a top-level `links: Vec<(Range<usize>, String)>` field. The `has_any_links` helper must handle both patterns.

  - Visual baseline normalization:
    - Compare the width/padding/bg from the old `render_assistant_message()` (was: `max_w(px(300.0))`, `bg(Theme::assistant_bubble())` or `bg(Theme::bg_darker())`, etc.)
    - Ensure `AssistantBubble`'s container matches these values
    - Gate streaming-specific styles on `self.is_streaming`

#### `src/ui_gpui/views/chat_view/render.rs`

- REFINE `render_assistant_message()` function (from P10 stub):
  - Add `/// @plan:PLAN-20260402-MARKDOWN.P11`
  - Ensure model_id fallback to "Assistant"
  - Ensure thinking content passed through
  - Remove all old raw-text rendering code
  - Clean implementation:
    ```rust
    /// @plan:PLAN-20260402-MARKDOWN.P11
    /// @requirement:REQ-MD-INTEGRATE-010
    pub(super) fn render_assistant_message(msg: &ChatMessage, show_thinking: bool) -> gpui::AnyElement {
        let mut bubble = AssistantBubble::new(msg.content.clone());
        if let Some(ref model_id) = msg.model_id {
            bubble = bubble.model_id(model_id.clone());
        } else {
            bubble = bubble.model_id("Assistant");
        }
        if show_thinking {
            if let Some(ref thinking) = msg.thinking {
                bubble = bubble.thinking(thinking.clone()).show_thinking(true);
            }
        }
        bubble.into_any_element()
    }
    ```

### Required Code Markers

```rust
/// @plan:PLAN-20260402-MARKDOWN.P11
/// @requirement:REQ-MD-INTEGRATE-002
/// @pseudocode render-markdown.md lines 1-10
```

### FORBIDDEN

- Modifying any tests (P04, P07, P09)
- Creating duplicate files
- `println!()` or `dbg!()` in production code
- `// TODO` comments
- Hardcoded colors (must use Theme::*)
- Modifying store or presenter layers
- Adding mdstream dependency

## Verification Commands

### Automated Checks (Structural)

```bash
# 1. P11 markers present
grep -r "@plan:PLAN-20260402-MARKDOWN.P11" src/ | wc -l
# Expected: 3+

# 2. All tests pass (parser + renderer + integration)
cargo test --lib || exit 1

# 3. No test modifications
git diff HEAD -- src/ui_gpui/components/markdown_content.rs | grep -E "^[-+].*#\[test\]" | head -5
# Expected: No test changes

# 4. Clippy clean
cargo clippy --all-targets -- -D warnings || exit 1

# 5. Formatted
cargo fmt --all -- --check || exit 1

# 6. No hardcoded colors
grep -rn "rgb(0x" src/ui_gpui/components/message_bubble.rs | grep -v "mod tests" | grep -v "// " && echo "FAIL"

# 7. No debug code
grep -rn "println!\|dbg!" src/ui_gpui/components/message_bubble.rs src/ui_gpui/views/chat_view/render.rs && echo "FAIL"

# 8. Store/presenter not modified
git diff HEAD -- src/ui_gpui/app_store.rs | wc -l
# Expected: 0
```

### Deferred Implementation Detection (MANDATORY)

```bash
# No todo!/unimplemented! in modified files
grep -rn "todo!\|unimplemented!" src/ui_gpui/components/message_bubble.rs | grep -v "mod tests"
# Expected: No matches

grep -rn "todo!\|unimplemented!" src/ui_gpui/views/chat_view/render.rs | grep -v "mod tests"
# Expected: No matches

# No placeholder comments
grep -rn -E "(// TODO|// FIXME|placeholder|not yet)" src/ui_gpui/components/message_bubble.rs src/ui_gpui/views/chat_view/render.rs
# Expected: No matches
```

### Semantic Verification Checklist

1. **Does the code DO what the requirements say?**
   - [ ] AssistantBubble calls parse_markdown_blocks + blocks_to_elements (REQ-MD-INTEGRATE-002)
   - [ ] render_assistant_message delegates to AssistantBubble (REQ-MD-INTEGRATE-010)
   - [ ] Click-to-copy attached when no links, not streaming (REQ-MD-INTEGRATE-020)
   - [ ] Click-to-copy NOT attached when has links (REQ-MD-INTEGRATE-021)
   - [ ] Click-to-copy NOT attached during streaming (REQ-MD-INTEGRATE-022)
   - [ ] Raw markdown copied to clipboard, not rendered text (REQ-MD-INTEGRATE-023)
   - [ ] Link detection is recursive AND inspects table cells (REQ-MD-INTEGRATE-024)
   - [ ] Streaming cursor appended (REQ-MD-INTEGRATE-040)
   - [ ] Model label falls back to "Assistant" (REQ-MD-INTEGRATE-015)

2. **Is table-cell link detection correct (REQ-MD-INTEGRATE-024)?**
   - [ ] `has_any_links` checks `Paragraph.links` and `Heading.links` fields
   - [ ] `has_any_links` recurses into `BlockQuote.children`
   - [ ] `has_any_links` recurses into `List.items` (each item is Vec<MarkdownBlock>)
   - [ ] `has_any_links` inspects `Table.header` cells for `MarkdownInline.link_url`
   - [ ] `has_any_links` inspects `Table.rows` cells for `MarkdownInline.link_url`
   - [ ] Both span-level links (link_url) and block-level links (links field) are detected

3. **Is this REAL implementation?**
   - [ ] Deferred implementation detection passed
   - [ ] All code paths are complete — no stubs remaining

4. **Is the feature REACHABLE?**
   - [ ] render_assistant_message → AssistantBubble → markdown pipeline
   - [ ] Streaming path → AssistantBubble → markdown pipeline
   - [ ] User can see markdown rendered in the actual app

5. **Visual baseline preserved?**
   - [ ] Width, padding, background color match pre-change values
   - [ ] Streaming cursor visible at end of content
   - [ ] Model label and thinking block still render

6. **What's MISSING?**
   - [ ] [List gaps]

#### Feature Actually Works

```bash
# Build and run the app
cargo run --bin personal_agent_gpui
# Then:
# 1. Send a message to the assistant
# 2. Observe that **bold**, *italic*, `code`, headings render correctly
# 3. Click on a completed message (no links) → content copied to clipboard
# 4. If message has a link → clicking link opens browser, clicking elsewhere does nothing
# 5. During streaming → cursor visible, no click-to-copy
# Expected: All markdown constructs render visually correct
# Actual: [paste what actually happens]
```

## Success Criteria

- **ALL P09 integration tests now PASS** — this is the phase that makes them pass. Every test that was expected to fail in P09 and P10 must now succeed. If any P09 test still fails, this phase is not complete.
- All P04 (parser) + P07 (renderer) tests continue to pass
- No tests modified
- No `todo!()` or `unimplemented!()` in production code
- No hardcoded colors
- Store/presenter layers untouched
- Clippy and fmt clean
- Click-to-copy conditional behavior verified
- Visual baseline preserved
- Table-cell link detection verified (REQ-MD-INTEGRATE-024)

## Failure Recovery

If this phase fails:
1. Rollback: `git checkout -- src/ui_gpui/components/message_bubble.rs src/ui_gpui/views/chat_view/render.rs`
2. Re-apply P10 stubs from git history
3. Re-attempt implementation
4. Cannot proceed to Phase 12 until all tests pass

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P11.md`
Contents:
```markdown
Phase: P11
Completed: [timestamp]
Files Modified: message_bubble.rs [diff stats], render.rs [diff stats]
Tests Added: 0 (from P09, not modified)
Tests Passing: [count] / [total]
Verification: [paste of cargo test, clippy, fmt outputs]

## Holistic Functionality Assessment

### What was implemented?
[Describe the full integration: render_assistant_message delegation, AssistantBubble markdown pipeline, click-to-copy behavior, recursive table-cell link detection]

### Does it satisfy the requirements?
[For each REQ-MD-INTEGRATE requirement, explain how]

### What is the data flow?
[Trace: ChatMessage → render_assistant_message → AssistantBubble::new → into_element → parse_markdown_blocks → blocks_to_elements → GPUI elements]

### What could go wrong?
[Visual regressions, click handling edge cases, performance on long messages]

### Verdict
[PASS/FAIL with explanation]
```
