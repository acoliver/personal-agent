# Phase 03: Parser Stub

## Phase ID

`PLAN-20260402-MARKDOWN.P03`

## Prerequisites

- Required: Phase 02a completed (pseudocode verified)
- Verification: `ls project-plans/issue62/.completed/P02a.md`
- Expected files from previous phase: Pseudocode in `project-plans/issue62/analysis/pseudocode/`
- Preflight verification: Phase 0.5 MUST be completed before any implementation phase

## Requirements Implemented (Expanded)

### REQ-MD-PARSE-001: Parser Entry Point

**Full Text**: WHEN markdown text is passed to the parser entry point, the system shall produce a `Vec<MarkdownBlock>` intermediate representation with no GPUI dependency.
**Behavior**:
- GIVEN: Any markdown string input
- WHEN: `parse_markdown_blocks(content)` is called
- THEN: Returns `Vec<MarkdownBlock>` (may be empty for empty input)
**Why This Matters**: This is the foundation of the two-phase architecture. All parsing logic flows through this single entry point.

### REQ-MD-PARSE-060: IR Debug/Clone/PartialEq

**Full Text**: The `MarkdownBlock` enum shall derive `Debug`, `Clone`, and `PartialEq` to enable test assertions on parsed structure.
**Behavior**:
- GIVEN: Any `MarkdownBlock` value
- WHEN: Compared with `==`, cloned, or debug-printed
- THEN: Operations succeed (derive macros work)
**Why This Matters**: Without these derives, TDD tests in Phase 04 cannot assert on parser output.

### REQ-MD-PARSE-061: MarkdownInline Fields

**Full Text**: The `MarkdownInline` struct shall contain fields for: text content, bold flag, italic flag, strikethrough flag, code flag, and optional link URL.
**Behavior**:
- GIVEN: The `MarkdownInline` type definition
- WHEN: A span is constructed with any combination of style flags
- THEN: All fields are independently settable and readable
**Why This Matters**: Every inline text segment in every block uses this type.

### REQ-MD-PARSE-062: MarkdownBlock Variants

**Full Text**: The `MarkdownBlock` enum shall include variants for: paragraphs, headings, code blocks, blockquotes, lists, tables, thematic breaks, and image fallbacks.
**Behavior**:
- GIVEN: The `MarkdownBlock` enum definition
- WHEN: Any markdown construct is parsed
- THEN: A corresponding variant exists
**Why This Matters**: Each variant maps to a specific GPUI element construction in the renderer.

### REQ-MD-PARSE-063: Link Collection

**Full Text**: The `Paragraph` and `Heading` variants shall each contain a links collection recording byte ranges and URLs of link spans.
**Behavior**:
- GIVEN: A paragraph or heading with links
- WHEN: Parsed
- THEN: The `links` field contains `Vec<(Range<usize>, String)>` entries
**Why This Matters**: Link collection enables the conditional click-to-copy vs link-click behavior.

### REQ-MD-PARSE-064: pub(crate) Visibility

**Full Text**: The intermediate model types shall have `pub(crate)` visibility — they are internal implementation details, not part of the public API.
**Behavior**:
- GIVEN: The IR types
- WHEN: External crates attempt to use them
- THEN: Compilation fails (not public)
**Why This Matters**: Keeps the IR as an internal implementation detail that can be changed without API breakage.

### REQ-MD-RENDER-040: Public API

**Full Text**: The system shall provide a public function that composes parsing and element rendering into a single call, accepting markdown content and returning GPUI elements.
**Behavior**:
- GIVEN: A markdown string
- WHEN: `render_markdown(content)` is called
- THEN: Returns `Vec<AnyElement>` (stub: returns empty vec or todo!())
**Why This Matters**: This is the entry point that `AssistantBubble` will call.

### REQ-MD-RENDER-042: Module Export

**Full Text**: The markdown rendering function and its module shall be exported from the components module.
**Behavior**:
- GIVEN: The components module
- WHEN: `render_markdown` is imported
- THEN: Import resolves successfully
**Why This Matters**: The function must be reachable from `message_bubble.rs` and `render.rs`.

### REQ-MD-SEC-001: URL Scheme Allowlist

**Full Text**: The system shall allow only `https` and `http` URL schemes for link click handling. All other schemes shall be rejected.
**Behavior**:
- GIVEN: A URL string
- WHEN: `is_safe_url(url)` is called
- THEN: Returns true only for http/https schemes
**Why This Matters**: Prevents malicious links (javascript:, file://, etc.) from executing.

## Implementation Tasks

### Files to Create

#### `src/ui_gpui/components/markdown_content.rs`

- MUST include: `/// @plan:PLAN-20260402-MARKDOWN.P03`
- MUST include requirement markers on each type/function

Contents:

1. **IR Types** (from pseudocode parse-markdown-blocks.md):
   - `pub(crate) enum MarkdownBlock` with all 8 variants (Paragraph, Heading, CodeBlock, BlockQuote, List, Table, ThematicBreak, ImageFallback)
   - `pub(crate) struct MarkdownInline` with fields: text, bold, italic, strikethrough, code, link_url
   - Both derive `Debug, Clone, PartialEq`

2. **Function Signatures** (stubs that compile):
   - `pub(crate) fn parse_markdown_blocks(content: &str) -> Vec<MarkdownBlock>` → `todo!()`
   - `pub(crate) fn blocks_to_elements(blocks: &[MarkdownBlock]) -> Vec<gpui::AnyElement>` → `todo!()`
   - `pub fn render_markdown(content: &str) -> Vec<gpui::AnyElement>` → `todo!()`
   - `pub(crate) fn is_safe_url(raw: &str) -> bool` → `todo!()`

### Files to Modify

#### `Cargo.toml`

- ADD: `pulldown-cmark = "0.13"` to `[dependencies]`
- ADD: `url = "2"` to `[dependencies]`
- ADD comment: `# Issue 62: Markdown rendering`

#### `src/ui_gpui/components/mod.rs`

- ADD: `pub mod markdown_content;`
- ADD: `pub use markdown_content::render_markdown;`
- ADD comment: `// Issue 62: Markdown rendering`

### Required Code Markers

Every function/struct/enum created in this phase MUST include:

```rust
/// @plan:PLAN-20260402-MARKDOWN.P03
/// @requirement:REQ-MD-PARSE-060 (or appropriate requirement ID)
```

## Verification Commands

### Automated Checks (Structural)

```bash
# Check plan markers exist
grep -r "@plan:PLAN-20260402-MARKDOWN.P03" src/ui_gpui/components/markdown_content.rs | wc -l
# Expected: 6+ occurrences (one per type/function)

# Check requirements covered
grep -r "@requirement:REQ-MD-PARSE" src/ui_gpui/components/markdown_content.rs | wc -l
# Expected: 5+ occurrences

# Compile
cargo build || exit 1

# Clippy snapshot (non-gating in stub phases)
# Keep clippy config unchanged, but do not block stub phases on strict lint noise.
cargo clippy --all-targets -- -D warnings || true

# No test modifications
# (No tests exist yet, so this is N/A for stub phase)
```

### Structural Verification Checklist

- [ ] `markdown_content.rs` created at correct path
- [ ] `Cargo.toml` has `pulldown-cmark` and `url` dependencies
- [ ] `mod.rs` exports `markdown_content` module and `render_markdown`
- [ ] `MarkdownBlock` has all 8 variants
- [ ] `MarkdownInline` has all 6 fields (text, bold, italic, strikethrough, code, link_url)
- [ ] Both types derive `Debug, Clone, PartialEq`
- [ ] Types have `pub(crate)` visibility
- [ ] `render_markdown` has `pub` visibility
- [ ] `is_safe_url` has `pub(crate)` visibility
- [ ] All stubs use `todo!()` macro
- [ ] Plan markers added to all items
- [ ] Project compiles with `cargo build`

### Deferred Implementation Detection

```bash
# In stub phase, todo!() is expected. But NO TODO comments:
grep -rn "// TODO\|// FIXME\|// HACK" src/ui_gpui/components/markdown_content.rs
# Expected: No matches

# No version duplication
find src -name "*markdown*_v2*" -o -name "*markdown*_new*" -o -name "*markdown*_copy*"
# Expected: No matches
```

### Semantic Verification Checklist

1. **Does the code define what the requirement says?**
   - [ ] I read REQ-MD-PARSE-060, -061, -062, -063, -064 and confirmed types match
   - [ ] I read the MarkdownBlock enum and confirmed all 8 variants exist
   - [ ] I read the MarkdownInline struct and confirmed all 6 fields exist

2. **Is this a proper stub, not placeholder?**
   - [ ] Functions use `todo!()` (acceptable in stub phase)
   - [ ] Types are fully defined (not stub types)
   - [ ] No "will be implemented" comments

3. **Is the module reachable?**
   - [ ] `mod.rs` exports the module
   - [ ] `render_markdown` is publicly accessible
   - [ ] `cargo build` succeeds

## Success Criteria

- `cargo build` succeeds
- All IR types are fully defined with correct fields and derives
- All function signatures are correct (parameters and return types)
- Function bodies use `todo!()` (acceptable in stub phase)
- Module is exported from `components/mod.rs`
- Dependencies added to `Cargo.toml`

## Failure Recovery

If this phase fails:
1. Rollback: `git checkout -- src/ui_gpui/components/markdown_content.rs src/ui_gpui/components/mod.rs Cargo.toml`
2. Files to revert: `markdown_content.rs`, `mod.rs`, `Cargo.toml`
3. Cannot proceed to Phase 04 until fixed

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P03.md`
Contents:
```markdown
Phase: P03
Completed: [timestamp]
Files Created: markdown_content.rs [line count]
Files Modified: Cargo.toml [diff stats], mod.rs [diff stats]
Tests Added: 0 (stub phase)
Verification: [paste of cargo build output]
```
