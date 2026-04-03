# Phase 04: Parser TDD Tests

## Phase ID

`PLAN-20260402-MARKDOWN.P04`

## Prerequisites

- Required: Phase 03a completed (parser stub verified)
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P03" src/ui_gpui/components/markdown_content.rs`
- Expected files from previous phase: `src/ui_gpui/components/markdown_content.rs` with type definitions and stub functions
- Preflight verification: Phase 0.5 completed

## Requirements Implemented (Expanded)

This phase writes behavioral tests for ALL parser requirements. Tests expect REAL behavior — they will fail naturally because the parser is still a `todo!()` stub.

### REQ-MD-PARSE-002: Paragraph Parsing

**Full Text**: WHEN the input contains one or more paragraphs separated by blank lines, the parser shall produce a separate `MarkdownBlock::Paragraph` for each paragraph, containing the paragraph's inline spans and collected link ranges.
**Behavior**:
- GIVEN: `"Hello world"` (single paragraph)
- WHEN: `parse_markdown_blocks()` is called
- THEN: Returns `vec![MarkdownBlock::Paragraph { spans: [MarkdownInline { text: "Hello world", bold: false, ... }], links: [] }]`
**Why This Matters**: Paragraphs are the most common markdown construct.

### REQ-MD-PARSE-003: Heading Parsing

**Full Text**: WHEN the input contains ATX headings (`#` through `######`), the parser shall produce `MarkdownBlock::Heading` with the correct `level` (1–6), inline spans, and collected link ranges.
**Behavior**:
- GIVEN: `"# Title"` (H1 heading)
- WHEN: `parse_markdown_blocks()` is called
- THEN: Returns `vec![MarkdownBlock::Heading { level: 1, spans: [...], links: [] }]`
**Why This Matters**: Headings are the primary structural element in markdown.

### REQ-MD-PARSE-004: Fenced Code Block Parsing

**Full Text**: WHEN the input contains a fenced code block (triple backticks or tildes), the parser shall produce `MarkdownBlock::CodeBlock` with `language` set to the info string (if present) and `code` set to the code content.
**Behavior**:
- GIVEN: `` ```rust\nfn main() {}\n``` ``
- WHEN: `parse_markdown_blocks()` is called
- THEN: Returns `vec![MarkdownBlock::CodeBlock { language: Some("rust".to_string()), code: "fn main() {}\n".to_string() }]`
**Why This Matters**: Code blocks are the highest-value rendering improvement for a developer AI assistant.

### REQ-MD-PARSE-005: Indented Code Block

**Full Text**: WHEN the input contains an indented code block (four-space or one-tab indent), the parser shall produce `MarkdownBlock::CodeBlock` with `language: None`.
**Behavior**:
- GIVEN: `"    let x = 1;\n"` (4-space indented)
- WHEN: `parse_markdown_blocks()` is called
- THEN: Returns `vec![MarkdownBlock::CodeBlock { language: None, code: "let x = 1;\n".to_string() }]`

### REQ-MD-PARSE-006: Block Quote

**Full Text**: WHEN the input contains a blockquote (lines prefixed with `>`), the parser shall produce `MarkdownBlock::BlockQuote` containing recursively parsed child blocks.
**Behavior**:
- GIVEN: `"> Some quote"` 
- WHEN: parsed
- THEN: Returns `MarkdownBlock::BlockQuote { children: [Paragraph { ... }] }`

### REQ-MD-PARSE-007: Unordered List

**Full Text**: WHEN the input contains an unordered list, the parser shall produce `MarkdownBlock::List` with `ordered: false`.
**Behavior**:
- GIVEN: `"- item 1\n- item 2\n- item 3"`
- WHEN: parsed
- THEN: Returns `MarkdownBlock::List { ordered: false, start: 0, items: [3 items] }`

### REQ-MD-PARSE-008: Ordered List

**Full Text**: WHEN the input contains an ordered list, the parser shall produce `MarkdownBlock::List` with `ordered: true` and correct start number.
**Behavior**:
- GIVEN: `"1. first\n2. second"`
- WHEN: parsed
- THEN: Returns `MarkdownBlock::List { ordered: true, start: 1, items: [2 items] }`

### REQ-MD-PARSE-009: Table

**Full Text**: WHEN the input contains a table, the parser shall produce `MarkdownBlock::Table` with correct alignments, header cells, and body rows.
**Behavior**:
- GIVEN: `"| A | B |\n|---|---|\n| 1 | 2 |"`
- WHEN: parsed
- THEN: Returns `MarkdownBlock::Table { alignments: [None, None], header: [[spans], [spans]], rows: [[[spans], [spans]]] }`

### REQ-MD-PARSE-010: Thematic Break

**Full Text**: WHEN the input contains a thematic break (`---`), the parser shall produce `MarkdownBlock::ThematicBreak`.
**Behavior**:
- GIVEN: `"---"`
- WHEN: parsed
- THEN: Returns `vec![MarkdownBlock::ThematicBreak]`

### REQ-MD-PARSE-020 through REQ-MD-PARSE-029: Inline Styles

**Full Text**: Bold, italic, strikethrough, inline code, links, task list markers, nested styles, soft/hard breaks.
**Behavior** (summarized):
- GIVEN: `"**bold**"` → span with `bold: true`
- GIVEN: `"*italic*"` → span with `italic: true`
- GIVEN: `"***bold-italic***"` → span with `bold: true, italic: true`
- GIVEN: `"~~strike~~"` → span with `strikethrough: true`
- GIVEN: `` "`code`" `` → span with `code: true`
- GIVEN: `"[click](https://example.com)"` → span with `link_url: Some("https://example.com")`; links field populated
- GIVEN: `"- [x] done"` → text contains U+2611; `"- [ ] todo"` → text contains U+2610

### REQ-MD-PARSE-026: Task List Marker

**Full Text**: WHEN the parser encounters a task list marker event, it shall insert a Unicode checkbox character ( U+2611 for checked,  U+2610 for unchecked) into the current text accumulation.
**Behavior**:
- GIVEN: `"- [x] Done task"`
- WHEN: `parse_markdown_blocks()` is called
- THEN: The list item's paragraph text begins with `` (U+2611)
- GIVEN: `"- [ ] Open task"`
- WHEN: `parse_markdown_blocks()` is called
- THEN: The list item's paragraph text begins with `` (U+2610)
**Why This Matters**: Task lists are common in AI assistant output for to-do lists and step-by-step guides.

### REQ-MD-PARSE-040 through REQ-MD-PARSE-051: Fallbacks

**Full Text**: Image fallback, footnotes, HTML stripping, math, metadata skip, definition lists, etc.
**Behavior** (summarized):
- GIVEN: `"![alt text](url)"` → `MarkdownBlock::ImageFallback { alt: "alt text" }`
- GIVEN: `"<div>text</div>"` → text extracted, tags stripped
- GIVEN: `"<script>alert(1)</script>"` → content stripped entirely
- GIVEN: `"$math$"` → span with `code: true`
- GIVEN: `"$$display$$"` → `MarkdownBlock::CodeBlock { language: None }`

### REQ-MD-PARSE-041: Footnote Definition Fallback

**Full Text**: WHEN the parser encounters a footnote definition, it shall render the footnote content inline as a paragraph rather than creating a separate footnote section.
**Behavior**:
- GIVEN: Markdown with a footnote definition `"[^1]: Footnote text"`
- WHEN: `parse_markdown_blocks()` is called
- THEN: The footnote text is captured as inline content (not lost or panicked on)
**Why This Matters**: Footnotes are uncommon in AI output but must not cause parsing failures.

### REQ-MD-PARSE-042: Footnote Reference Literal Text

**Full Text**: WHEN the parser encounters a footnote reference marker (e.g., `[^1]`), it shall emit the reference label as literal text rather than creating an interactive footnote link.
**Behavior**:
- GIVEN: `"See note[^1]"`
- WHEN: `parse_markdown_blocks()` is called
- THEN: Text includes the literal `[^1]` or footnote label as plain text
**Why This Matters**: Without footnote rendering support, references should degrade gracefully.

### REQ-MD-PARSE-047: Display Math to Code Block

**Full Text**: WHEN the parser encounters a display math block (`$$...$$`), it shall produce a `MarkdownBlock::CodeBlock` with `language: None` containing the math expression.
**Behavior**:
- GIVEN: `"$$E = mc^2$$"`
- WHEN: `parse_markdown_blocks()` is called
- THEN: Returns `MarkdownBlock::CodeBlock { language: None, code: "E = mc^2" }`
**Why This Matters**: Display math has no native rendering; code block provides readable fallback.

### REQ-MD-PARSE-049: Metadata Block Skip

**Full Text**: WHEN the parser encounters a YAML/TOML metadata block (front matter), it shall skip the block entirely without producing any output blocks.
**Behavior**:
- GIVEN: `"---
title: Hello
---

Content"`
- WHEN: `parse_markdown_blocks()` is called
- THEN: Metadata block is skipped; only `Content` paragraph is returned
**Why This Matters**: Front matter in AI output should be silently ignored.

### REQ-MD-PARSE-051: Definition Lists Fallback Rendering

**Full Text**: WHEN the parser encounters definition list markup, it shall render the terms and definitions as plain paragraphs rather than using a dedicated definition list construct.
**Behavior**:
- GIVEN: Definition list markup (if supported by pulldown-cmark)
- WHEN: `parse_markdown_blocks()` is called
- THEN: Content is rendered as paragraphs (fallback), not lost
**Why This Matters**: Definition lists are rare but should degrade gracefully.

### REQ-MD-PARSE-065: Unknown Event Fallback

**Full Text**: IF an unknown or unhandled pulldown-cmark event type is encountered, THEN the parser shall extract any text content and append it as plain text rather than panicking.
**Behavior**:
- GIVEN: Any markdown input
- WHEN: parsed
- THEN: Never panics (pulldown-cmark handles malformed input gracefully)

### REQ-MD-SEC-001 through REQ-MD-SEC-006: URL Validation

**Full Text**: URL scheme allowlist, dangerous scheme rejection, URL parsing via `url::Url::parse`.
**Behavior**:
- GIVEN: `"https://example.com"` → `is_safe_url()` returns `true`
- GIVEN: `"javascript:alert(1)"` → returns `false`
- GIVEN: `"file:///etc/passwd"` → returns `false`
- GIVEN: `""` → returns `false`
- GIVEN: `"not a url"` → returns `false`

## Implementation Tasks

### Files to Create/Modify

#### `src/ui_gpui/components/markdown_content.rs` — Add test module

Add `#[cfg(test)] mod tests { ... }` at the bottom of `markdown_content.rs` containing all behavioral tests.

- MUST include: `/// @plan:PLAN-20260402-MARKDOWN.P04`
- MUST include: `/// @requirement:REQ-MD-PARSE-XXX` on each test

### Test Categories (30+ tests)

#### Block-Level Parse Tests (10+ tests)

```rust
/// @plan:PLAN-20260402-MARKDOWN.P04
/// @requirement:REQ-MD-PARSE-002
/// @scenario Single paragraph
/// @given Markdown text "Hello world"
/// @when parse_markdown_blocks() is called
/// @then Returns one Paragraph block with correct text
#[test]
fn test_parse_single_paragraph() { ... }

/// @requirement:REQ-MD-PARSE-002
/// @scenario Multiple paragraphs
#[test]
fn test_parse_multiple_paragraphs() { ... }

/// @requirement:REQ-MD-PARSE-003
/// @scenario Headings levels 1 through 6
#[test]
fn test_parse_heading_levels() { ... }

/// @requirement:REQ-MD-PARSE-004
/// @scenario Fenced code block with language
#[test]
fn test_parse_fenced_code_block_with_language() { ... }

/// @requirement:REQ-MD-PARSE-004
/// @scenario Fenced code block without language
#[test]
fn test_parse_fenced_code_block_without_language() { ... }

/// @requirement:REQ-MD-PARSE-005
/// @scenario Indented code block
#[test]
fn test_parse_indented_code_block() { ... }

/// @requirement:REQ-MD-PARSE-006
/// @scenario Blockquote with nested paragraph
#[test]
fn test_parse_blockquote() { ... }

/// @requirement:REQ-MD-PARSE-007
/// @scenario Unordered list with three items
#[test]
fn test_parse_unordered_list() { ... }

/// @requirement:REQ-MD-PARSE-008
/// @scenario Ordered list with starting number
#[test]
fn test_parse_ordered_list() { ... }

/// @requirement:REQ-MD-PARSE-009
/// @scenario Table with header and body rows
#[test]
fn test_parse_table() { ... }

/// @requirement:REQ-MD-PARSE-010
/// @scenario Thematic break
#[test]
fn test_parse_thematic_break() { ... }
```

#### Inline Style Tests (9+ tests)

```rust
/// @requirement:REQ-MD-PARSE-020
#[test]
fn test_parse_bold_text() { ... }

/// @requirement:REQ-MD-PARSE-021
#[test]
fn test_parse_italic_text() { ... }

/// @requirement:REQ-MD-PARSE-022
#[test]
fn test_parse_bold_italic_text() { ... }

/// @requirement:REQ-MD-PARSE-023
#[test]
fn test_parse_strikethrough_text() { ... }

/// @requirement:REQ-MD-PARSE-024
#[test]
fn test_parse_inline_code() { ... }

/// @requirement:REQ-MD-PARSE-025
#[test]
fn test_parse_link_with_url() { ... }

/// @requirement:REQ-MD-PARSE-026
/// @scenario Task list markers produce checkbox characters
/// @given Markdown "- [x] Done
- [ ] Open"
/// @when parse_markdown_blocks() is called
/// @then Checked items contain U+2611, unchecked contain U+2610
#[test]
fn test_parse_task_list_markers() { ... }

/// @requirement:REQ-MD-PARSE-027
#[test]
fn test_parse_nested_inline_styles() { ... }

/// @requirement:REQ-MD-PARSE-028, REQ-MD-PARSE-029
#[test]
fn test_parse_soft_and_hard_breaks() { ... }
```

#### Fallback Tests (12+ tests)

```rust
/// @requirement:REQ-MD-PARSE-040
#[test]
fn test_parse_image_fallback() { ... }

/// @requirement:REQ-MD-PARSE-041
/// @scenario Footnote definition degrades to inline content
/// @given Markdown with footnote definition "[^1]: Footnote text"
/// @when parse_markdown_blocks() is called
/// @then Footnote text is captured (not lost or panicked on)
#[test]
fn test_parse_footnote_definition_fallback() { ... }

/// @requirement:REQ-MD-PARSE-042
/// @scenario Footnote reference degrades to literal text
/// @given Markdown "See note[^1]"
/// @when parse_markdown_blocks() is called
/// @then Output contains the footnote label as plain text
#[test]
fn test_parse_footnote_reference_literal() { ... }

/// @requirement:REQ-MD-PARSE-043
#[test]
fn test_parse_html_block_strips_tags() { ... }

/// @requirement:REQ-MD-PARSE-044
#[test]
fn test_parse_inline_html_strips_tags() { ... }

/// @requirement:REQ-MD-PARSE-045
#[test]
fn test_parse_script_tags_stripped_entirely() { ... }

/// @requirement:REQ-MD-PARSE-046
#[test]
fn test_parse_inline_math_as_code() { ... }

/// @requirement:REQ-MD-PARSE-047
/// @scenario Display math block degrades to code block
/// @given Markdown "$$E = mc^2$$"
/// @when parse_markdown_blocks() is called
/// @then Returns MarkdownBlock::CodeBlock { language: None, code: "E = mc^2" }
#[test]
fn test_parse_display_math_as_code_block() { ... }

/// @requirement:REQ-MD-PARSE-049
/// @scenario Metadata/front matter block is skipped
/// @given Markdown "---
title: Hello
---

Content"
/// @when parse_markdown_blocks() is called
/// @then Only the "Content" paragraph is returned; metadata is skipped
#[test]
fn test_parse_metadata_block_skip() { ... }

/// @requirement:REQ-MD-PARSE-050
#[test]
fn test_parse_malformed_html_no_panic() { ... }

/// @requirement:REQ-MD-PARSE-051
/// @scenario Definition list markup degrades to paragraphs
/// @given Markdown with definition list syntax
/// @when parse_markdown_blocks() is called
/// @then Content rendered as paragraphs (fallback), not lost
#[test]
fn test_parse_definition_list_fallback() { ... }
```

#### URL Validation Tests (4+ tests)

```rust
/// @requirement:REQ-MD-SEC-001
#[test]
fn test_safe_url_allows_http_https() { ... }

/// @requirement:REQ-MD-SEC-002
#[test]
fn test_safe_url_rejects_dangerous_schemes() { ... }

/// @requirement:REQ-MD-SEC-003
#[test]
fn test_safe_url_handles_malformed_input() { ... }

/// @requirement:REQ-MD-SEC-004
#[test]
fn test_safe_url_edge_cases() { ... }
```

#### Edge Case Tests (4+ tests)

```rust
/// @requirement:REQ-MD-PARSE-065
#[test]
fn test_parse_empty_content() { ... }

#[test]
fn test_parse_whitespace_only_content() { ... }

#[test]
fn test_parse_complex_mixed_markdown() { ... }

#[test]
fn test_parse_deeply_nested_lists() { ... }
```

### Required Code Markers

Every test MUST include:

```rust
/// @plan:PLAN-20260402-MARKDOWN.P04
/// @requirement:REQ-MD-PARSE-XXX
/// @scenario [description]
/// @given [input]
/// @when [action]
/// @then [expected output]
#[test]
fn test_name() {
    // test implementation
}
```

## Verification Commands

### Automated Checks (Structural)

```bash
# Check plan markers exist
grep -r "@plan:PLAN-20260402-MARKDOWN.P04" src/ui_gpui/components/markdown_content.rs | wc -l
# Expected: 30+ occurrences (one per test)

# Check requirements covered
grep -r "@requirement:REQ-MD-PARSE" src/ui_gpui/components/markdown_content.rs | wc -l
# Expected: 30+ occurrences

# Check requirements covered - security
grep -r "@requirement:REQ-MD-SEC" src/ui_gpui/components/markdown_content.rs | wc -l
# Expected: 4+ occurrences

# Compile (but tests will fail — that's expected)
cargo build --all-targets || exit 1

# Tests should fail naturally (not compile errors, but assertion failures or todo!() panics)
cargo test --lib -- markdown_content 2>&1 | head -40
# Expected: test failures from todo!() panic, NOT compile errors
```

### Structural Verification Checklist

- [ ] Phase 03 markers still present
- [ ] No skipped phases (P03 exists before P04)
- [ ] Test module added to `markdown_content.rs`
- [ ] 30+ tests created
- [ ] All tests tagged with plan and requirement markers
- [ ] Tests compile (`cargo build --all-targets`)
- [ ] Tests fail naturally (not compile errors)
- [ ] No `#[should_panic]` annotations (no reverse testing)
- [ ] No tests that pass with empty implementations
- [ ] Tests assert on concrete output values, not just `is_ok()` or `is_some()`

### Deferred Implementation Detection

```bash
# No reverse testing
grep -rn "should_panic" src/ui_gpui/components/markdown_content.rs && echo "FAIL: Reverse testing found"

# No tests checking for todo!() behavior
grep -rn "expect.*todo\|expect.*unimplemented\|expect.*panic" src/ui_gpui/components/markdown_content.rs && echo "FAIL: Testing for stub behavior"
```

### Semantic Verification Checklist

1. **Does each test verify real behavior?**
   - [ ] Tests expect specific `MarkdownBlock` variant output, not just that code ran
   - [ ] Tests compare against concrete expected values (specific strings, specific struct fields)
   - [ ] Tests would still fail if implementation returned wrong values (not just wrong type)

2. **Is reverse testing absent?**
   - [ ] No `#[should_panic]` attributes
   - [ ] No tests expecting `todo!()` behavior
   - [ ] All tests will fail with assertion errors when implementation is wrong

3. **Would tests catch a broken implementation?**
   - [ ] Bold test would fail if bold flag was false
   - [ ] Code block test would fail if language was wrong
   - [ ] Link test would fail if links vec was empty

4. **What's MISSING?**
   - [ ] All previously identified gaps now covered:
     - REQ-MD-PARSE-026 (task list markers) — test added
     - REQ-MD-PARSE-041 (footnote definition fallback) — test added
     - REQ-MD-PARSE-042 (footnote reference literal) — test added
     - REQ-MD-PARSE-047 (display math to code block) — test added
     - REQ-MD-PARSE-049 (metadata block skip) — test added
     - REQ-MD-PARSE-051 (definition list fallback) — test added
   - [ ] [Verify no other gaps remain]

## Success Criteria

- 36+ behavioral tests created (including coverage for REQ-MD-PARSE-026/041/042/047/049/051)
- All tests tagged with plan and requirement markers
- Tests compile but fail (todo!() panic in stubs)
- No reverse testing patterns
- Requirements REQ-MD-PARSE-001 through -065 and REQ-MD-SEC-001 through -006 fully covered

## Failure Recovery

If this phase fails:
1. Rollback: `git checkout -- src/ui_gpui/components/markdown_content.rs` (tests only — preserve stub code from P03)
2. Cannot proceed to Phase 05 until tests are written correctly

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P04.md`
Contents:
```markdown
Phase: P04
Completed: [timestamp]
Files Modified: markdown_content.rs [diff stats]
Tests Added: [count]
Verification: [paste of cargo build --all-targets output]
Test failure sample: [paste of cargo test output showing expected failures]
```
``
