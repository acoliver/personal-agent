# Phase 01: Domain Analysis

## Phase ID

`PLAN-20260402-MARKDOWN.P01`

## Prerequisites

- Required: Phase 0.5 (Preflight Verification) completed
- Verification: All preflight gate checkboxes checked
- Expected files from previous phase: `00a-preflight-verification.md` completed

## Purpose

Analyze the markdown rendering domain, GPUI rendering model, and integration points to produce artifacts that inform pseudocode and implementation. This phase produces understanding, not code.

## Requirements Implemented

This phase does not implement requirements directly. It produces the domain understanding needed to implement:

- **REQ-MD-PARSE group**: Understanding pulldown-cmark's event model, how events map to IR types
- **REQ-MD-RENDER group**: Understanding GPUI's text rendering API (`StyledText`, `TextRun`, `InteractiveText`, grid)
- **REQ-MD-INTEGRATE group**: Understanding the current rendering paths and integration touch points

## Analysis Tasks

### 1. Markdown Parsing Domain Model

Analyze pulldown-cmark 0.13's event stream model:

- **Event lifecycle**: `Start(Tag)` → content events → `End(Tag)` nesting
- **Block vs inline distinction**: Which events produce block-level IR, which produce inline IR
- **Nesting model**: How blockquotes contain paragraphs, lists contain items containing paragraphs
- **Text accumulation**: How `Text(cow)` events interleave with style Start/End events
- **Link model**: How link events carry URL, and how byte ranges are tracked
- **HTML handling**: Three event types (`Html`, `InlineHtml`, `HtmlBlock` start/end) and their distinct contexts
- **Edge cases**: What pulldown-cmark produces for malformed input, empty input, deeply nested structures

Output: `project-plans/issue62/analysis/domain-model.md`

### 2. GPUI Rendering Model Analysis

Analyze the GPUI APIs to be used:

- **`StyledText::with_runs()`**: How TextRun vec maps to styled text segments, len calculation (UTF-8 bytes)
- **`InteractiveText`**: How click ranges map to text positions, how to wrap StyledText
- **Grid layout**: How `div().grid().grid_cols(n)` maps children into a grid
- **Element composition**: How `div().child()`, `.children()`, `IntoElement` trait work
- **Font system**: `Font` struct fields, `FontWeight`, `FontStyle`, `SharedString` for family names
- **Color system**: `Hsla` type, how Theme methods return colors

Output: `project-plans/issue62/analysis/gpui-rendering-model.md`

### 3. Integration Point Analysis

Map exactly how new code connects to existing code:

- **Current `render_assistant_message()`**: Line-by-line analysis of current implementation, what it does, what the refactored version must preserve
- **Current `AssistantBubble::into_element()`**: Current implementation, what changes
- **Current streaming path in `render_chat_area()`**: How streaming bubbles are constructed today
- **Click-to-copy current behavior**: Exact clipboard copy mechanism
- **Width/styling differences**: Document the visual differences between `render_assistant_message()` (max_w 300px, specific padding) and `AssistantBubble` (w 400px, different padding)

Output: `project-plans/issue62/analysis/integration-analysis.md`

### 4. IR Type Design Analysis

Design the `MarkdownBlock` and `MarkdownInline` types:

- **Variant enumeration**: Map every block-level construct to an enum variant
- **Inline span model**: Design the `MarkdownInline` struct with style flags
- **Link collection**: How `(Range<usize>, String)` tuples are stored per block
- **Table model**: How alignments, header, and rows are represented
- **Derive requirements**: `Debug`, `Clone`, `PartialEq` for test assertions
- **Visibility**: `pub(crate)` — internal implementation detail

Output: `project-plans/issue62/analysis/ir-type-design.md`

## Verification

### Analysis Completeness Checklist

- [ ] Domain model covers all 13 pulldown-cmark Event variants
- [ ] Domain model covers all Tag variants relevant to Phase A
- [ ] GPUI rendering model covers `StyledText`, `InteractiveText`, grid, `TextRun`, `Font`
- [ ] Integration analysis identifies all files to modify with line numbers
- [ ] Integration analysis documents width/styling differences to normalize
- [ ] IR type design covers all MarkdownBlock variants
- [ ] IR type design covers MarkdownInline fields
- [ ] Link collection design is documented

## Success Criteria

- Four analysis documents produced
- Each document contains specific, actionable information (not generic descriptions)
- Analysis references actual code line numbers from the current codebase
- No unknowns remain that would block pseudocode creation

## Failure Recovery

If analysis reveals API mismatches or unexpected current behavior:
1. Document the finding in the analysis artifact
2. Update preflight verification with new blocking issues
3. Re-evaluate plan phases if needed before proceeding

## Phase Completion Marker

Create: `project-plans/issue62/.completed/P01.md`
