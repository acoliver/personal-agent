# Phase 03a: Parser Stub Verification

## Phase ID

`PLAN-20260402-MARKDOWN.P03a`

## Prerequisites

- Required: Phase 03 completed
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P03" src/ui_gpui/components/markdown_content.rs`
- Expected files from previous phase:
  - `src/ui_gpui/components/markdown_content.rs` (new)
  - `src/ui_gpui/components/mod.rs` (modified)
  - `Cargo.toml` (modified)

## Verification Commands

### Automated Checks

```bash
# 1. Plan markers exist
grep -c "@plan:PLAN-20260402-MARKDOWN.P03" src/ui_gpui/components/markdown_content.rs
# Expected: 6+

# 2. Requirement markers exist
grep -c "@requirement" src/ui_gpui/components/markdown_content.rs
# Expected: 6+

# 3. Compiles
cargo build || exit 1

# 3a. Clippy snapshot (non-gating in stub verification)
cargo clippy --all-targets -- -D warnings || true

# 4. No TODO comments (todo!() macro is OK)
grep -rn "// TODO\|// FIXME\|// HACK" src/ui_gpui/components/markdown_content.rs && echo "FAIL: TODO comments found"

# 5. No duplicate files
find src -name "*markdown_content*" | wc -l
# Expected: 1

# 6. Dependencies in Cargo.toml
grep "pulldown-cmark" Cargo.toml || echo "FAIL: pulldown-cmark missing"
grep 'url = "2"' Cargo.toml || echo "FAIL: url missing"

# 7. Module export
grep "pub mod markdown_content" src/ui_gpui/components/mod.rs || echo "FAIL: module not exported"
grep "pub use markdown_content::render_markdown" src/ui_gpui/components/mod.rs || echo "FAIL: render_markdown not re-exported"

# 8. No reverse testing (should not exist yet)
grep -rn "should_panic\|#\[should_panic\]" src/ui_gpui/components/markdown_content.rs && echo "FAIL: Reverse testing found"
```

### Structural Verification Checklist

- [ ] Phase 0.5 preflight completed
- [ ] `markdown_content.rs` exists at `src/ui_gpui/components/markdown_content.rs`
- [ ] `MarkdownBlock` enum has 8 variants: Paragraph, Heading, CodeBlock, BlockQuote, List, Table, ThematicBreak, ImageFallback
- [ ] `MarkdownInline` struct has fields: text (String), bold (bool), italic (bool), strikethrough (bool), code (bool), link_url (Option<String>)
- [ ] Both types derive Debug, Clone, PartialEq
- [ ] `parse_markdown_blocks` signature: `pub(crate) fn parse_markdown_blocks(content: &str) -> Vec<MarkdownBlock>`
- [ ] `blocks_to_elements` signature: `pub(crate) fn blocks_to_elements(blocks: &[MarkdownBlock]) -> Vec<gpui::AnyElement>`
- [ ] `render_markdown` signature: `pub fn render_markdown(content: &str) -> Vec<gpui::AnyElement>`
- [ ] `is_safe_url` signature: `pub(crate) fn is_safe_url(raw: &str) -> bool`
- [ ] All function bodies are `todo!()` (stub phase)
- [ ] Plan markers on all items
- [ ] `cargo build` succeeds

## Success Criteria

- All structural checks pass
- No TODO comments (only todo!() macros)
- No duplicate files created
- Dependencies correctly added
- Module correctly exported

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P03a.md`
