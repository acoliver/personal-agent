# Phase 02: Pseudocode

## Phase ID

`PLAN-20260402-MARKDOWN.P02`

## Prerequisites

- Required: Phase 01a completed (analysis verified)
- Verification: `ls project-plans/issue62/.completed/P01a.md`
- Expected files from previous phase: All analysis artifacts in `project-plans/issue62/analysis/`

## Purpose

Create numbered-line pseudocode for three core functions that will drive all implementation:

1. `parse_markdown_blocks()` — the pulldown-cmark event walker producing `Vec<MarkdownBlock>`
2. `blocks_to_elements()` — the IR-to-GPUI translator
3. `render_markdown()` — the public API composing both

Implementation phases (P05, P08) MUST reference these pseudocode line numbers.

## Requirements Implemented (Context)

This pseudocode covers the logic for ALL Phase A requirements. Each pseudocode function maps to a requirement group:

- `parse_markdown_blocks()` → REQ-MD-PARSE-001 through REQ-MD-PARSE-065
- `blocks_to_elements()` → REQ-MD-RENDER-001 through REQ-MD-RENDER-053
- `render_markdown()` → REQ-MD-RENDER-040 through REQ-MD-RENDER-043
- `is_safe_url()` → REQ-MD-SEC-001 through REQ-MD-SEC-006

## Implementation Tasks

### Files to Create

#### `project-plans/issue62/analysis/pseudocode/parse-markdown-blocks.md`

Numbered-line pseudocode for the event walker. Must cover:

- Line 1-10: Function signature, pulldown-cmark Options setup, Parser construction
- Lines 11-20: Main event loop structure, block stack, inline style stack, list stack initialization
- Lines 21-60: Block-level event handling (`Start(Paragraph)`, `End(Paragraph)`, `Start(Heading)`, `End(Heading)`, `Start(CodeBlock)`, `End(CodeBlock)`, etc.)
- Lines 61-80: List handling (`Start(List)`, `End(List)`, `Start(Item)`, `End(Item)`)
- Lines 81-100: Table handling (`Start(Table)`, `Start(TableHead)`, `Start(TableRow)`, `Start(TableCell)`, etc.)
- Lines 101-120: Inline style events (`Start(Strong)`, `End(Strong)`, `Start(Emphasis)`, etc.)
- Lines 121-140: Text events (`Text`, `Code`, `SoftBreak`, `HardBreak`)
- Lines 141-160: Link events (`Start(Link)`, `End(Link)`) with byte range tracking
- Lines 161-180: Fallback events (Image, Footnote, HTML, Math, Metadata, etc.)
- Lines 181-190: HTML tag-stripping state machine
- Lines 191-200: Return accumulated blocks

Each line must include:
- The pseudocode logic
- Comment noting which REQ-MD-PARSE-* requirement it implements

#### `project-plans/issue62/analysis/pseudocode/blocks-to-elements.md`

Numbered-line pseudocode for the IR-to-GPUI translator. Must cover:

- Lines 1-10: Function signature, result accumulator
- Lines 11-30: Paragraph rendering (spans → TextRun, InteractiveText for links)
- Lines 31-50: Heading rendering (level-based font sizing, bold weight)
- Lines 51-70: CodeBlock rendering (div with bg, monospace font, language label)
- Lines 71-90: BlockQuote rendering (left border, bg, recursive children)
- Lines 91-120: List rendering (per-item div, bullet/number prefix, depth indentation)
- Lines 121-150: Table rendering (grid container, header cells, body cells, alternating rows)
- Lines 151-160: ThematicBreak rendering (horizontal rule div)
- Lines 161-170: ImageFallback rendering (muted text)
- Lines 171-190: Helper: spans_to_text_runs() — MarkdownInline vec → TextRun vec + plain text string
- Lines 191-200: Helper: make_text_element() — TextRun vec → StyledText or InteractiveText

Each line must include:
- The pseudocode logic
- Comment noting which REQ-MD-RENDER-* requirement it implements
- Theme color method being used (e.g., `Theme::bg_darker()`)

#### `project-plans/issue62/analysis/pseudocode/render-markdown.md`

Numbered-line pseudocode for the public API and helpers:

- Lines 1-10: `render_markdown()` — compose parse + render
- Lines 11-25: `is_safe_url()` — URL parsing and scheme validation
- Lines 26-35: `has_links()` — recursive link detection across all block variants

Each line must include the REQ-* being implemented.

## Verification Commands

### Structural Verification

```bash
# Verify pseudocode files exist
ls project-plans/issue62/analysis/pseudocode/parse-markdown-blocks.md || echo "FAIL"
ls project-plans/issue62/analysis/pseudocode/blocks-to-elements.md || echo "FAIL"
ls project-plans/issue62/analysis/pseudocode/render-markdown.md || echo "FAIL"
```

### Content Verification

```bash
# Verify line numbers are present (numbered lines)
grep -c "^[0-9]" project-plans/issue62/analysis/pseudocode/parse-markdown-blocks.md
# Expected: 150+ numbered lines

grep -c "^[0-9]" project-plans/issue62/analysis/pseudocode/blocks-to-elements.md
# Expected: 150+ numbered lines

grep -c "^[0-9]" project-plans/issue62/analysis/pseudocode/render-markdown.md
# Expected: 25+ numbered lines

# Verify REQ references
grep -c "REQ-MD-PARSE" project-plans/issue62/analysis/pseudocode/parse-markdown-blocks.md
# Expected: 30+ references

grep -c "REQ-MD-RENDER" project-plans/issue62/analysis/pseudocode/blocks-to-elements.md
# Expected: 20+ references

grep -c "REQ-MD-SEC\|REQ-MD-RENDER" project-plans/issue62/analysis/pseudocode/render-markdown.md
# Expected: 5+ references
```

## Success Criteria

- Three pseudocode files with numbered lines
- Every line references a requirement ID
- All pulldown-cmark events covered in parser pseudocode
- All MarkdownBlock variants covered in renderer pseudocode
- HTML tag-stripping state machine pseudocode included
- URL validation pseudocode included
- Implementation phases can cite specific line numbers

## Failure Recovery

If pseudocode reveals gaps in analysis:
1. Return to Phase 01 to fill the analysis gap
2. Update pseudocode with the new understanding
3. Proceed only when pseudocode is complete

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P02.md`
