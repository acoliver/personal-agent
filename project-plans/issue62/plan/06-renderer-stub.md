# Phase 06: Renderer Stub

## Phase ID

`PLAN-20260402-MARKDOWN.P06`

## Prerequisites

- Required: Phase 05a completed (parser implementation verified)
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P05" src/ui_gpui/components/markdown_content.rs`
- Expected files from previous phase: `markdown_content.rs` with working parser and URL validator
- Preflight verification: Phase 0.5 completed

## Requirements Implemented (Expanded)

### REQ-MD-RENDER-001: Paragraph Element

**Full Text**: WHEN the renderer receives a `MarkdownBlock::Paragraph`, the system shall produce a styled text element wrapped in a paragraph div with vertical margin.
**Behavior**:
- GIVEN: A `MarkdownBlock::Paragraph` with spans and links
- WHEN: `blocks_to_elements()` is called
- THEN: Returns a div containing styled text (stub: returns placeholder div)
**Why This Matters**: Paragraphs are the most common element.

### REQ-MD-RENDER-002: Interactive Text for Links

**Full Text**: WHEN the paragraph contains links (non-empty `links` field), the renderer shall produce an interactive text element with click handlers for each link range.
**Behavior**:
- GIVEN: A Paragraph with non-empty links field
- WHEN: Rendered
- THEN: Uses `InteractiveText` instead of plain `StyledText`
**Why This Matters**: Click-to-open-URL for links.

### REQ-MD-RENDER-040: Public API (render_markdown)

**Full Text**: The system shall provide a public function that composes parsing and element rendering into a single call.
**Behavior**:
- GIVEN: A markdown string
- WHEN: `render_markdown(content)` is called
- THEN: Returns `Vec<AnyElement>` (stub: calls parse then todo!() for render)
**Why This Matters**: This is the single entry point for all callers.

### REQ-MD-RENDER-041: Empty Content

**Full Text**: IF the input content is empty or whitespace-only, THEN the markdown rendering function shall return an empty element collection without panicking.
**Behavior**:
- GIVEN: Empty string `""`
- WHEN: `render_markdown("")` is called
- THEN: Returns empty `Vec<AnyElement>`
**Why This Matters**: Empty assistant messages must not crash.

## Implementation Tasks

### Files to Modify

#### `src/ui_gpui/components/markdown_content.rs`

- UPDATE `blocks_to_elements()`: Replace `todo!()` with stub that returns `vec![]` or a basic placeholder per block. This is NOT the final implementation — it provides compilable function signatures for the TDD phase.
  - Add `/// @plan:PLAN-20260402-MARKDOWN.P06`
  - Return type: `Vec<gpui::AnyElement>`
  - Stub approach: Return `vec![]` (empty) — tests in P07 will fail on this

- UPDATE `render_markdown()`: Replace `todo!()` with composition stub:
  ```rust
  pub fn render_markdown(content: &str) -> Vec<gpui::AnyElement> {
      let blocks = parse_markdown_blocks(content);
      blocks_to_elements(&blocks)
  }
  ```
  - Add `/// @plan:PLAN-20260402-MARKDOWN.P06`

- ADD helper function stubs (all returning `todo!()` or default values):
  - `fn spans_to_styled_text(spans: &[MarkdownInline], links: &[(Range<usize>, String)]) -> gpui::AnyElement` → `todo!()`
  - `fn render_paragraph(spans: &[MarkdownInline], links: &[(Range<usize>, String)]) -> gpui::AnyElement` → `todo!()`
  - `fn render_heading(level: u8, spans: &[MarkdownInline], links: &[(Range<usize>, String)]) -> gpui::AnyElement` → `todo!()`
  - `fn render_code_block(language: &Option<String>, code: &str) -> gpui::AnyElement` → `todo!()`
  - `fn render_blockquote(children: &[MarkdownBlock]) -> gpui::AnyElement` → `todo!()`
  - `fn render_list(ordered: bool, start: u64, items: &[Vec<MarkdownBlock>]) -> gpui::AnyElement` → `todo!()`
  - `fn render_table(alignments: &[Option<pulldown_cmark::Alignment>], header: &[Vec<MarkdownInline>], rows: &[Vec<Vec<MarkdownInline>>]) -> gpui::AnyElement` → `todo!()`

### Required Code Markers

```rust
/// @plan:PLAN-20260402-MARKDOWN.P06
/// @requirement:REQ-MD-RENDER-001
fn render_paragraph(...) -> gpui::AnyElement {
    todo!() // Stub — implementation in P08
}
```

## Verification Commands

### Automated Checks (Structural)

```bash
# Check P06 markers
grep -c "@plan:PLAN-20260402-MARKDOWN.P06" src/ui_gpui/components/markdown_content.rs
# Expected: 8+ (one per stub function)

# Compile
cargo build || exit 1

# Clippy snapshot (non-gating in stub phases)
cargo clippy --all-targets -- -D warnings || true

# Existing parser tests still pass
cargo test --lib -- markdown_content::tests::test_parse 2>&1 | tail -5
# Expected: All parser tests pass

# No TODO comments
grep -rn "// TODO\|// FIXME\|// HACK" src/ui_gpui/components/markdown_content.rs && echo "FAIL"

# No duplicate files
find src -name "*markdown*_v2*" -o -name "*markdown*_new*" && echo "FAIL"
```

### Structural Verification Checklist

- [ ] P03, P04, P05 markers still present
- [ ] P06 markers on all new stub functions
- [ ] `blocks_to_elements()` has a compilable body (not todo!() at top level — delegates to helpers that may todo!())
- [ ] `render_markdown()` composes `parse_markdown_blocks()` + `blocks_to_elements()`
- [ ] Helper render functions exist with correct signatures
- [ ] `cargo build` succeeds
- [ ] All parser tests (P04) still pass
- [ ] No existing tests broken

### Deferred Implementation Detection

```bash
# In stub phase, todo!() in helper render functions is expected
# But render_markdown() should NOT have todo!() — it should compose real calls
grep -n "todo!()" src/ui_gpui/components/markdown_content.rs | grep -v "mod tests"
# Expected: Only in helper render functions (render_paragraph, render_heading, etc.)
# NOT in render_markdown() or blocks_to_elements() top-level
```

### Semantic Verification Checklist

1. **Is the stub properly structured?**
   - [ ] `render_markdown()` calls `parse_markdown_blocks()` then `blocks_to_elements()` (real composition)
   - [ ] `blocks_to_elements()` matches over MarkdownBlock variants and calls helper functions
   - [ ] Helper functions have correct parameter types matching IR model

2. **Is the module still reachable?**
   - [ ] `render_markdown` is still exported from `mod.rs`
   - [ ] `cargo build` succeeds

## Success Criteria

- `cargo build` succeeds
- All parser tests still pass
- `render_markdown()` composes parse + render (real composition, not todo!())
- `blocks_to_elements()` delegates to helper functions
- Helper render functions exist as stubs
- P06 markers present

## Failure Recovery

If this phase fails:
1. Rollback renderer changes only: restore blocks_to_elements and render_markdown to `todo!()`
2. Re-attempt with corrected stub structure
3. Cannot proceed to Phase 07 until fixed

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P06.md`
