# Phase 08: Renderer Implementation

## Phase ID

`PLAN-20260402-MARKDOWN.P08`

## Prerequisites

- Required: Phase 07a completed (renderer TDD verified)
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P07" src/ui_gpui/components/markdown_content.rs`
- Expected files from previous phase: `markdown_content.rs` with stub render functions and 12+ failing renderer tests
- Preflight verification: Phase 0.5 completed

## Requirements Implemented (Expanded)

This phase implements all REQ-MD-RENDER requirements to make all P07 tests pass.

### REQ-MD-RENDER-001 through REQ-MD-RENDER-011: Element Mapping

**Full Text** (summarized): Each MarkdownBlock variant maps to specific GPUI element construction:
- Paragraph → styled text in div with vertical margin; InteractiveText if links
- Heading → text with level-scaled font size, bold weight
- CodeBlock → div with bg_darker(), rounded corners, monospace font, optional language label
- BlockQuote → div with left border (accent), bg_base(), recursive children
- List → per-item div with depth indentation, bullet/number prefix
- Table → grid container with header, body, borders, alternating rows
- ThematicBreak → thin horizontal line div
- ImageFallback → styled muted text `[image: {alt}]`

### REQ-MD-RENDER-020 through REQ-MD-RENDER-026: Inline Style Rendering

**Full Text**: Each MarkdownInline style flag maps to TextRun properties:
- bold → `FontWeight::BOLD`
- italic → `FontStyle::Italic`
- strikethrough → `StrikethroughStyle` with `Theme::text_muted()` color
- code → monospace font family + `Theme::bg_darker()` background
- link_url → underline + `Theme::accent()` color
- list bullets/numbers → `Theme::text_muted()` color

### REQ-MD-RENDER-030 through REQ-MD-RENDER-033: Theme Integration

**Full Text**: All colors from Theme::*, correct font tokens (FONT_SIZE_MD for body, scaled sizes for headings, "Menlo" for code).

### REQ-MD-RENDER-004: Heading Sizes

**Full Text**: H1=24.0px, H2=20.0px, H3=18.0px, H4=16.0px, H5=14.0px, H6=13.0px.

### REQ-MD-RENDER-050 through REQ-MD-RENDER-053: Table Rendering Detail

**Full Text**: Grid columns match header, header bg `Theme::bg_dark()`, alternating rows `Theme::bg_base()`, borders `Theme::border()`.

## Implementation Tasks

### Files to Modify

#### `src/ui_gpui/components/markdown_content.rs`

- UPDATE all helper render functions: Replace `todo!()` with full GPUI element construction
  - MUST follow pseudocode from `project-plans/issue62/analysis/pseudocode/blocks-to-elements.md`
  - Add `/// @plan:PLAN-20260402-MARKDOWN.P08` marker
  - Add `/// @pseudocode lines X-Y` references

- Implement each helper function following pseudocode:

  **`render_paragraph()`** (pseudocode lines 11-30):
  - Build plain text string from spans
  - Build TextRun vec from spans (using spans_to_text_runs helper)
  - Create `StyledText::new(text).with_runs(runs)`
  - If links non-empty: wrap in `InteractiveText::new(id, styled_text).on_click(ranges, handler)`
  - Handler calls `is_safe_url()` then `cx.open_url()`
  - Wrap in div with vertical margin

  **`render_heading()`** (pseudocode lines 31-50):
  - Same as paragraph but with level-scaled font size and bold weight
  - Font sizes: match level to [24.0, 20.0, 18.0, 16.0, 14.0, 13.0]

  **`render_code_block()`** (pseudocode lines 51-70):
  - Outer div: `bg(Theme::bg_darker())`, rounded corners, padding
  - Optional language label: `Theme::text_muted()`, small size
  - Code text: monospace font ("Menlo"), `Theme::text_primary()`

  **`render_blockquote()`** (pseudocode lines 71-90):
  - Outer div: `border_l_2()` with `Theme::accent()`, `bg(Theme::bg_base())`, padding
  - Recursively call `blocks_to_elements()` for children

  **`render_list()`** (pseudocode lines 91-120):
  - Per-item div with `pl(px(depth * indent))`
  - Bullet/number prefix in `Theme::text_muted()`
  - Recursively render item's child blocks

  **`render_table()`** (pseudocode lines 121-150):
  - `div().grid().grid_cols(col_count as u16)`
  - Header cells: `bg(Theme::bg_dark())`, border
  - Body cells: alternating `bg(Theme::bg_base())`, border
  - Cell content: spans → styled text

  **`render_thematic_break()`** (pseudocode lines 151-160):
  - `div().h(px(1.0)).w_full().bg(Theme::border())`

  **`render_image_fallback()`** (pseudocode lines 161-170):
  - Styled text `[image: {alt}]` in `Theme::text_muted()`

  **`spans_to_text_runs()`** (pseudocode lines 171-190):
  - Convert `Vec<MarkdownInline>` to plain text String + `Vec<TextRun>`
  - Each span: calculate UTF-8 byte len, map style flags to Font/TextRun properties
  - bold → `FontWeight::BOLD`
  - italic → `FontStyle::Italic`
  - code → `Font { family: "Menlo".into(), ... }`
  - link → underline + accent color
  - strikethrough → `StrikethroughStyle { thickness: px(1.0), color: Some(Theme::text_muted()) }`

### Required Code Markers

```rust
/// @plan:PLAN-20260402-MARKDOWN.P08
/// @requirement:REQ-MD-RENDER-001
/// @pseudocode blocks-to-elements.md lines 11-30
fn render_paragraph(spans: &[MarkdownInline], links: &[(Range<usize>, String)]) -> gpui::AnyElement {
    // Implementation following pseudocode
}
```

### FORBIDDEN

- Modifying any existing tests
- Creating duplicate files
- `println!()` or `dbg!()` in production code
- `// TODO` comments
- `todo!()` or `unimplemented!()` remaining after implementation
- Hardcoded hex colors (must use Theme::*)

## Verification Commands

### Automated Checks (Structural)

```bash
# 1. P08 markers present
grep -c "@plan:PLAN-20260402-MARKDOWN.P08" src/ui_gpui/components/markdown_content.rs
# Expected: 8+ (one per render function)

# 2. Pseudocode references present
grep -c "@pseudocode" src/ui_gpui/components/markdown_content.rs
# Expected: 10+ (parser + renderer)

# 3. All tests pass
cargo test --lib -- markdown_content || exit 1

# 4. No test modifications
git diff src/ui_gpui/components/markdown_content.rs | grep -E "^[-+].*#\[test\]\|^[-+].*#\[gpui::test\]" | head -5
# Expected: No test additions or removals

# 5. Clippy clean
cargo clippy --all-targets -- -D warnings || exit 1

# 6. Formatted
cargo fmt --all -- --check || exit 1
```

### Deferred Implementation Detection (MANDATORY)

```bash
# No todo!/unimplemented! in production code
grep -rn "todo!\|unimplemented!" src/ui_gpui/components/markdown_content.rs | grep -v "mod tests" | grep -v "#\[cfg(test)\]"
# Expected: No matches (all stubs replaced with real implementations)

# No cop-out comments
grep -rn -E "(// TODO|// FIXME|// HACK|placeholder|not yet)" src/ui_gpui/components/markdown_content.rs
# Expected: No matches

# No empty implementations
grep -rn "fn .* \{\s*\}" src/ui_gpui/components/markdown_content.rs | grep -v test
# Expected: No matches

# No debug code
grep -rn "println!\|dbg!" src/ui_gpui/components/markdown_content.rs
# Expected: No matches

# No hardcoded colors
grep -rn "rgb(0x\|rgb!(0x\|hsla(0\." src/ui_gpui/components/markdown_content.rs | grep -v "mod tests"
# Expected: No matches (all colors from Theme::*)
```

### Semantic Verification Checklist

1. **Does the code DO what the requirements say?**
   - [ ] Paragraphs render as styled text in divs (REQ-MD-RENDER-001)
   - [ ] Links produce InteractiveText with click handlers (REQ-MD-RENDER-002)
   - [ ] Headings have scaled font sizes (REQ-MD-RENDER-003, -004)
   - [ ] Code blocks have bg_darker(), monospace font (REQ-MD-RENDER-005)
   - [ ] Tables use grid layout (REQ-MD-RENDER-009, -050)
   - [ ] All colors from Theme::* methods (REQ-MD-RENDER-030)
   - [ ] Empty content returns empty vec (REQ-MD-RENDER-041)

2. **Is this REAL implementation, not placeholder?**
   - [ ] Deferred implementation detection passed
   - [ ] Every render function produces real GPUI elements
   - [ ] TextRun vec properly maps inline styles to font properties

3. **Does pseudocode match implementation?**
   - [ ] Each render function follows its pseudocode lines
   - [ ] spans_to_text_runs() correctly calculates UTF-8 byte lengths
   - [ ] InteractiveText wrapping follows pseudocode pattern

4. **What's MISSING?**
   - [ ] [List any gaps]

## Success Criteria

- All P04 (parser) and P07 (renderer) tests pass
- No tests modified
- No `todo!()` or `unimplemented!()` remaining in production code
- No hardcoded colors
- No debug code
- Clippy and fmt clean
- Pseudocode references present on all render functions

## Failure Recovery

If this phase fails:
1. Rollback render function implementations, restore to P06 stubs
2. Re-attempt following pseudocode more carefully
3. Cannot proceed to Phase 09 until all tests pass

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P08.md`
Contents:
```markdown
Phase: P08
Completed: [timestamp]
Files Modified: markdown_content.rs [diff stats]
Tests Added: 0 (tests from P04+P07, not modified)
Tests Passing: [count] / [total]
Verification: [paste of cargo test, clippy, fmt outputs]

## Holistic Functionality Assessment

### What was implemented?
[Describe blocks_to_elements() and all render helper functions]

### Does it satisfy the requirements?
[For each REQ-MD-RENDER requirement, explain how]

### What is the data flow?
[Trace: MarkdownBlock → match variant → render_xxx() → AnyElement]

### What could go wrong?
[Edge cases, rendering issues]

### Verdict
[PASS/FAIL with explanation]
```
