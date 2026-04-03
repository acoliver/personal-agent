# Phase 07: Renderer TDD Tests

## Phase ID

`PLAN-20260402-MARKDOWN.P07`

## Prerequisites

- Required: Phase 06a completed (renderer stub verified)
- Verification: `grep -r "@plan:PLAN-20260402-MARKDOWN.P06" src/ui_gpui/components/markdown_content.rs`
- Expected files from previous phase: `markdown_content.rs` with renderer stubs and helper function signatures
- Preflight verification: Phase 0.5 completed

## Requirements Implemented (Expanded)

This phase writes behavioral tests for the GPUI renderer. These tests exercise the full `render_markdown()` pipeline and verify element construction. Tests require `#[gpui::test]` because they produce GPUI elements.

### REQ-MD-RENDER-001: Paragraph Element Construction

**Full Text**: WHEN the renderer receives a `MarkdownBlock::Paragraph`, the system shall produce a styled text element wrapped in a paragraph div with vertical margin.
**Behavior**:
- GIVEN: Markdown text `"Hello world"`
- WHEN: `render_markdown("Hello world")` is called
- THEN: Returns non-empty `Vec<AnyElement>` with 1 element
**Why This Matters**: Verifies basic element construction works.

### REQ-MD-RENDER-003: Heading Element Construction

**Full Text**: WHEN the renderer receives a `MarkdownBlock::Heading`, the system shall produce a text element with font size scaled by heading level and bold font weight.
**Behavior**:
- GIVEN: `"# Title"`
- WHEN: `render_markdown("# Title")` is called
- THEN: Returns non-empty `Vec<AnyElement>` with 1 element
**Why This Matters**: Headings must render with distinct visual treatment.

### REQ-MD-RENDER-005: Code Block Container

**Full Text**: WHEN the renderer receives a `MarkdownBlock::CodeBlock`, the system shall produce a div with `Theme::bg_darker()` background, rounded corners, and monospace font family.
**Behavior**:
- GIVEN: A fenced code block
- WHEN: Rendered
- THEN: Produces a container div element (not bare text)
**Why This Matters**: Code blocks need background styling.

### REQ-MD-RENDER-009: Table Grid

**Full Text**: WHEN the renderer receives a `MarkdownBlock::Table`, the system shall produce a CSS grid container.
**Behavior**:
- GIVEN: A 3-column table
- WHEN: Rendered
- THEN: Produces a grid container with header + body cell children
**Why This Matters**: Tables require structured layout.

### REQ-MD-RENDER-030: Theme Color Usage

**Full Text**: The markdown renderer shall source all colors exclusively from `Theme::*` static methods. No hardcoded hex values.
**Behavior**:
- GIVEN: Any markdown rendering code
- WHEN: Inspected
- THEN: No `rgb(0x...)` or hardcoded hex literals found
**Why This Matters**: Theme compliance for dark/light mode.

### REQ-MD-RENDER-041: Empty Content Handling

**Full Text**: IF the input content is empty or whitespace-only, THEN the markdown rendering function shall return an empty element collection without panicking.
**Behavior**:
- GIVEN: `""`
- WHEN: `render_markdown("")` is called
- THEN: Returns empty vec, no panic
**Why This Matters**: Edge case safety.

## Implementation Tasks

### Files to Modify

#### `src/ui_gpui/components/markdown_content.rs` — Add renderer tests

Add renderer-specific tests to the existing test module. These tests exercise the full `render_markdown()` pipeline.

**Note on test approach**: Since GPUI's `AnyElement` is opaque (no public inspection API), renderer tests verify:
1. Element count — correct number of elements produced
2. Non-empty output — elements are produced for valid input
3. No panics — all construct types render without error
4. Structural tests via `#[gpui::test]` where GPUI context is needed

The heavy behavioral testing is already done at the IR level in P04. Renderer tests provide confidence that the translation layer works.

### Test Categories (12+ tests)

#### Element Count / Smoke Tests (4+ tests using `#[gpui::test]`)

```rust
/// @plan:PLAN-20260402-MARKDOWN.P07
/// @requirement:REQ-MD-RENDER-001
/// @scenario Single paragraph produces one element
/// @given Markdown "Hello world"
/// @when render_markdown() is called
/// @then Returns Vec with 1 element
#[gpui::test]
fn test_render_paragraph_produces_element(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-RENDER-003
/// @scenario Heading produces one element
#[gpui::test]
fn test_render_heading_produces_element(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-RENDER-005
/// @scenario Code block produces one element
#[gpui::test]
fn test_render_code_block_produces_element(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-RENDER-041
/// @scenario Empty content returns empty vec
#[gpui::test]
fn test_render_empty_content_returns_empty(cx: &mut gpui::TestAppContext) { ... }
```

#### Multi-Block Tests (3+ tests)

```rust
/// @requirement:REQ-MD-RENDER-001
/// @scenario Multiple paragraphs produce multiple elements
#[gpui::test]
fn test_render_multiple_paragraphs(cx: &mut gpui::TestAppContext) { ... }

/// @scenario Mixed blocks (heading + paragraph + code) produce correct count
#[gpui::test]
fn test_render_mixed_blocks_correct_count(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-RENDER-010
/// @scenario Thematic break produces element
#[gpui::test]
fn test_render_thematic_break(cx: &mut gpui::TestAppContext) { ... }
```

#### Complex Construct Tests (3+ tests)

```rust
/// @requirement:REQ-MD-RENDER-008
/// @scenario List renders with correct item count
#[gpui::test]
fn test_render_list_produces_elements(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-RENDER-007
/// @scenario Blockquote renders children
#[gpui::test]
fn test_render_blockquote_produces_element(cx: &mut gpui::TestAppContext) { ... }

/// @requirement:REQ-MD-RENDER-009
/// @scenario Table renders as grid
#[gpui::test]
fn test_render_table_produces_element(cx: &mut gpui::TestAppContext) { ... }
```

#### Theme Compliance Tests (2+ tests — these can be `#[test]` static analysis)

```rust
/// @requirement:REQ-MD-RENDER-030
/// @scenario No hardcoded colors in markdown rendering code
#[test]
fn test_no_hardcoded_colors_in_markdown_content() {
    // Read the source file and verify no rgb(0x...) or hex color literals
    let source = include_str!("markdown_content.rs");
    assert!(!source.contains("rgb(0x"), "Hardcoded rgb color found in markdown_content.rs");
    assert!(!source.contains("rgb!("), "Hardcoded rgb! macro found in markdown_content.rs");
    // Allow Theme:: references only
}
```

### Required Code Markers

Every test MUST include:

```rust
/// @plan:PLAN-20260402-MARKDOWN.P07
/// @requirement:REQ-MD-RENDER-XXX
/// @scenario [description]
/// @given [input]
/// @when [action]
/// @then [expected output]
```

## Verification Commands

### Automated Checks (Structural)

```bash
# Check P07 markers
grep -c "@plan:PLAN-20260402-MARKDOWN.P07" src/ui_gpui/components/markdown_content.rs
# Expected: 12+ (one per test)

# Check RENDER requirement markers
grep -c "@requirement:REQ-MD-RENDER" src/ui_gpui/components/markdown_content.rs
# Expected: 12+

# Compile with test targets
cargo build --all-targets || exit 1

# Parser tests still pass
cargo test --lib -- markdown_content::tests::test_parse || exit 1

# Renderer tests should fail (stubs return empty/todo)
cargo test --lib -- markdown_content::tests::test_render 2>&1 | head -20
# Expected: Failures from stub behavior
```

### Structural Verification Checklist

- [ ] Previous phase markers intact (P03, P04, P05, P06)
- [ ] P07 markers on all new tests
- [ ] 12+ renderer tests created
- [ ] Tests use `#[gpui::test]` where GPUI context is needed
- [ ] Tests compile (`cargo build --all-targets`)
- [ ] Parser tests (P04) still pass
- [ ] Renderer tests fail naturally (stubs)
- [ ] No `#[should_panic]` annotations
- [ ] Tests assert on element count or non-emptiness (behavioral, not structural)

### Deferred Implementation Detection

```bash
# No reverse testing
grep -rn "should_panic" src/ui_gpui/components/markdown_content.rs && echo "FAIL"
```

### Semantic Verification Checklist

1. **Do tests verify real rendering behavior?**
   - [ ] Tests check element count matches expected block count
   - [ ] Tests verify non-empty output for valid input
   - [ ] Tests verify empty output for empty input

2. **Is coverage appropriate for opaque elements?**
   - [ ] IR-level tests (P04) cover structural correctness
   - [ ] Renderer tests (P07) cover element construction and integration
   - [ ] Theme compliance test verifies no hardcoded colors

3. **Would tests catch a broken renderer?**
   - [ ] If renderer returned empty vec for paragraphs, test would fail
   - [ ] If renderer panicked on code blocks, test would fail
   - [ ] If renderer produced wrong element count, test would fail

## Success Criteria

- 12+ renderer tests created
- Tests compile and fail naturally
- Parser tests unbroken
- No reverse testing

## Failure Recovery

If this phase fails:
1. Rollback new tests only
2. Re-attempt with corrected test approach
3. Cannot proceed to Phase 08 until tests are correct

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P07.md`
