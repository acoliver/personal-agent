# Requirements Document: Markdown Rendering for Assistant Messages

**Issue:** [#62 -- Markdown rendering for assistant messages](https://github.com/acoliver/personal-agent/issues/62)
**Format:** EARS (Easy Approach to Requirements Syntax)
**Derived From:** `project-plans/issue62/overview.md` (Technical Specification, Revised -- post-review round 4)
**Last Updated:** 2026-04-02
**Revision:** 4 (comprehensive-review reconciliation: harmonized IR schema, added Type classification, tightened Phase B language)

---

## Requirement Count Summary

| Group | Total | Behavioral | Constraint | Phase A | Phase B |
|-------|-------|------------|------------|---------|---------|
| REQ-MD-PARSE | 29 | 23 | 6 | 29 | 0 |
| REQ-MD-RENDER | 25 | 19 | 6 | 25 | 0 |
| REQ-MD-INTEGRATE | 15 | 10 | 5 | 13 | 2 |
| REQ-MD-STREAM | 26 | 19 | 7 | 2 | 24 |
| REQ-MD-PERF | 13 | 9 | 4 | 11 | 2 |
| REQ-MD-SEC | 8 | 6 | 2 | 6 | 2 |
| REQ-MD-TEST | 31 | 0 | 31 | 22 | 9 |
| **Total** | **147** | **86** | **61** | **108** | **39** |

---

## EARS Pattern Reference

| Pattern | Template | Usage |
|---|---|---|
| **Ubiquitous** | The [system] shall [requirement] | Always active |
| **Event-driven** | WHEN [trigger], the [system] shall [requirement] | Triggered by event |
| **Unwanted behavior** | IF [condition], THEN the [system] shall [requirement] | Error/edge handling |
| **State-driven** | WHILE [state], the [system] shall [requirement] | Active during state |
| **Optional feature** | WHERE [feature], the [system] shall [requirement] | Conditional on feature |

---

## Requirement Type Classification

> Each requirement is tagged with a **Type** to distinguish verifiable behavioral outcomes from implementation-prescriptive design constraints:
>
> - **Behavioral** -- Verifiable from user-visible outcomes, API contracts, or black-box input/output behavior. These are independently testable without knowledge of implementation internals.
> - **Constraint** -- Prescribes specific implementation choices (derive traits, visibility modifiers, specific crates, internal architecture, coding style). These constrain *how* the system is built rather than *what* it does. They are still valid requirements but are verified by code inspection rather than behavioral testing.

---

## Phase B Inactivity Rule

> **All requirements with Phase: B are inactive until the Dependency Validation Gate (Appendix D) passes.** Phase B requirements shall not be implemented, tested against, or used as acceptance criteria until the gate procedure in Appendix D 1 has been executed and the result documented. Phase A is fully self-contained and shall not depend on any Phase B requirement.

---

## REQ-MD-PARSE: Markdown Parsing (Phase A)

### Block-Level Constructs

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-PARSE-001 | WHEN markdown text is passed to the parser entry point, the system shall produce a `Vec<MarkdownBlock>` intermediate representation with no GPUI dependency. | Must | A | Behavioral | S3.1, S5.1, Top-Level Preconditions (Normative Architecture) |
| REQ-MD-PARSE-002 | WHEN the input contains one or more paragraphs separated by blank lines, the parser shall produce a separate `MarkdownBlock::Paragraph` for each paragraph, containing the paragraph's inline spans and collected link ranges. | Must | A | Behavioral | S2.1 Block-Level Constructs, S5.1 Event Table (`Start(Paragraph)` / `End(Paragraph)`) |
| REQ-MD-PARSE-003 | WHEN the input contains ATX headings (`#` through `######`), the parser shall produce `MarkdownBlock::Heading` with the correct `level` (1-6), inline spans, and collected link ranges. | Must | A | Behavioral | S2.1 Block-Level Constructs, S5.1 Event Table (`Start(Heading)` / `End(Heading)`) |
| REQ-MD-PARSE-004 | WHEN the input contains a fenced code block (triple backticks or tildes), the parser shall produce `MarkdownBlock::CodeBlock` with `language` set to the info string (if present) and `code` set to the code content. | Must | A | Behavioral | S2.1 Block-Level Constructs, S5.1 Event Table (`Start(CodeBlock)` / `End(CodeBlock)`) |
| REQ-MD-PARSE-005 | WHEN the input contains an indented code block (four-space or one-tab indent), the parser shall produce `MarkdownBlock::CodeBlock` with `language: None` and `code` set to the indented content. | Must | A | Behavioral | S2.1 Block-Level Constructs, S5.1 Event Table |
| REQ-MD-PARSE-006 | WHEN the input contains a blockquote (lines prefixed with `>`), the parser shall produce `MarkdownBlock::BlockQuote` containing recursively parsed child blocks. | Must | A | Behavioral | S2.1 Block-Level Constructs, S5.1 Event Table (`Start(BlockQuote)` / `End(BlockQuote)`) |
| REQ-MD-PARSE-007 | WHEN the input contains an unordered list (lines prefixed with `-`, `*`, or `+`), the parser shall produce `MarkdownBlock::List` with `ordered: false`, `start: 0` (u64), and each item as a `Vec<MarkdownBlock>`. | Must | A | Behavioral | S2.1 Block-Level Constructs, S5.1 Event Table (`Start(List(None))`) |
| REQ-MD-PARSE-008 | WHEN the input contains an ordered list (lines prefixed with `1.`, `2.`, etc.), the parser shall produce `MarkdownBlock::List` with `ordered: true`, `start` set to the first item number as `u64`, and each item as a `Vec<MarkdownBlock>`. | Must | A | Behavioral | S2.1 Block-Level Constructs, S5.1 Event Table (`Start(List(Some(n)))`) |
| REQ-MD-PARSE-009 | WHEN the input contains a table (pipe-delimited rows with a delimiter row), the parser shall produce `MarkdownBlock::Table` with correct `alignments`, `header` cells, and `rows` of body cells, each cell containing inline spans. | Must | A | Behavioral | S2.1 Block-Level Constructs, S5.1 Event Table (`Start(Table)` through `End(Table)`) |
| REQ-MD-PARSE-010 | WHEN the input contains a thematic break (`---`, `***`, or `___`), the parser shall produce `MarkdownBlock::ThematicBreak`. | Must | A | Behavioral | S2.1 Block-Level Constructs, S5.1 Event Table (`Rule`) |
| REQ-MD-PARSE-011 | The parser shall enable pulldown-cmark options `ENABLE_TABLES`, `ENABLE_STRIKETHROUGH`, and `ENABLE_TASKLISTS`. | Must | A | Constraint | S5.1 Builder Internals |

### Inline Constructs

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-PARSE-020 | WHEN the input contains bold text (`**text**` or `__text__`), the parser shall produce a `MarkdownInline` span with `bold: true`. | Must | A | Behavioral | S2.1 Inline Constructs, S5.1 Event Table (`Start(Strong)` / `End(Strong)`) |
| REQ-MD-PARSE-021 | WHEN the input contains italic text (`*text*` or `_text_`), the parser shall produce a `MarkdownInline` span with `italic: true`. | Must | A | Behavioral | S2.1 Inline Constructs, S5.1 Event Table (`Start(Emphasis)` / `End(Emphasis)`) |
| REQ-MD-PARSE-022 | WHEN the input contains bold-italic text (`***text***`), the parser shall produce a `MarkdownInline` span with both `bold: true` and `italic: true`. | Must | A | Behavioral | S2.1 Inline Constructs |
| REQ-MD-PARSE-023 | WHEN the input contains strikethrough text (`~~text~~`), the parser shall produce a `MarkdownInline` span with `strikethrough: true`. | Must | A | Behavioral | S2.1 Inline Constructs, S5.1 Event Table (`Start(Strikethrough)` / `End(Strikethrough)`) |
| REQ-MD-PARSE-024 | WHEN the input contains inline code (`` `text` ``), the parser shall produce a `MarkdownInline` span with `code: true`. | Must | A | Behavioral | S2.1 Inline Constructs, S5.1 Event Table (`Code(text)`) |
| REQ-MD-PARSE-025 | WHEN the input contains a link (`[text](url)`), the parser shall produce a `MarkdownInline` span with `link_url: Some(url)` and register a `(Range<usize>, String)` tuple in the containing block's `links` field. | Must | A | Behavioral | S2.1 Inline Constructs, S5.1 Event Table (`Start(Link)` / `End(Link)`), S5.1 Text Accumulation Strategy |
| REQ-MD-PARSE-026 | WHEN the input contains task list markers (`- [x]` or `- [ ]`), the parser shall prepend the appropriate Unicode ballot box character (U+2611 for checked, U+2610 for unchecked) to the span text. | Should | A | Behavioral | S2.1 Inline Constructs, S5.1 Event Table (`TaskListMarker(checked)`) |
| REQ-MD-PARSE-027 | WHEN inline styles are nested (e.g., bold within italic, code within bold), the parser shall correctly track the style stack and produce spans reflecting all active styles. | Must | A | Behavioral | S5.1 Builder Internals (Inline style stack) |
| REQ-MD-PARSE-028 | WHEN a `SoftBreak` event is encountered, the parser shall append a space to the current span text. | Must | A | Behavioral | S5.1 Event Table (`SoftBreak`) |
| REQ-MD-PARSE-029 | WHEN a `HardBreak` event is encountered, the parser shall append a newline to the current span text. | Must | A | Behavioral | S5.1 Event Table (`HardBreak`) |

### Graceful Fallbacks

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-PARSE-040 | WHEN the input contains an image (`![alt](url)`), the parser shall produce `MarkdownBlock::ImageFallback` with the `alt` text extracted. No image rendering shall be attempted. | Must | A | Behavioral | S2.1 Graceful Fallbacks, S5.1 Event Table (`Start(Image)` / `End(Image)`) |
| REQ-MD-PARSE-041 | WHEN the input contains a footnote definition (`[^label]: content`), the parser shall render the footnote as inline text with the label prepended as `[^{label}]: `. | Should | A | Behavioral | S2.1 Graceful Fallbacks, S5.1 Event Table (`Start(FootnoteDefinition)` / `End(FootnoteDefinition)`) |
| REQ-MD-PARSE-042 | WHEN the input contains a footnote reference (`[^label]`), the parser shall append `[^label]` as literal text to the current span. | Should | A | Behavioral | S5.1 Event Table (`FootnoteReference(label)`) |
| REQ-MD-PARSE-043 | WHEN the input contains block-level HTML, the parser shall strip all HTML tags via the tag-stripping state machine (see REQ-MD-SEC-010) and emit the extracted text content as a `MarkdownBlock::Paragraph`. | Must | A | Behavioral | S5.1.1 pulldown-cmark HTML Event Handling, S8.3 |
| REQ-MD-PARSE-044 | WHEN the input contains inline HTML, the parser shall strip HTML tags via the tag-stripping state machine (see REQ-MD-SEC-010) and append the extracted text content to the current inline text buffer, preserving surrounding text flow. | Must | A | Behavioral | S5.1.1 pulldown-cmark HTML Event Handling |
| REQ-MD-PARSE-045 | WHEN the input contains `<script>` or `<style>` HTML tags, the parser shall strip both the tags and their enclosed content entirely -- no text shall be extracted from script or style elements. | Must | A | Behavioral | S5.1.1 pulldown-cmark HTML Event Handling (test coverage), S8.3 |
| REQ-MD-PARSE-046 | WHEN the input contains inline math (`$...$`), the parser shall produce a `MarkdownInline` span with `code: true` (rendered as code-styled monospace text). | Should | A | Behavioral | S2.1 Graceful Fallbacks, S5.1 Event Table (`InlineMath(text)`) |
| REQ-MD-PARSE-047 | WHEN the input contains display math (`$$...$$`), the parser shall produce `MarkdownBlock::CodeBlock` with `language: None` and the math content as code. | Should | A | Behavioral | S2.1 Graceful Fallbacks, S5.1 Event Table (`DisplayMath(text)`) |
| REQ-MD-PARSE-048 | WHEN the input contains superscript or subscript markup, the parser shall render the content as plain text (no-op for start/end events, text content flows through normally). | Should | A | Behavioral | S2.1 Graceful Fallbacks, S5.1 Event Table (`Start(Superscript)` / `Start(Subscript)`) |
| REQ-MD-PARSE-049 | WHEN the input contains a metadata block (YAML/TOML front matter), the parser shall skip the block entirely (no output produced). | Should | A | Behavioral | S2.1 Graceful Fallbacks, S5.1 Event Table (`Start(MetadataBlock)` / `End(MetadataBlock)`) |
| REQ-MD-PARSE-050 | IF the input contains malformed HTML (e.g., unmatched `<` without `>`), THEN the parser shall treat the `<` as literal text and append it to the current span without panic. | Must | A | Behavioral | S5.1.1 Tag-stripping state machine |
| REQ-MD-PARSE-051 | WHEN the input contains definition lists, the parser shall render definition titles as bold-styled paragraphs and definitions as indented paragraphs. | Could | A | Behavioral | S5.1 Event Table (`Start(DefinitionList)` / `Start(DefinitionListTitle)` / `Start(DefinitionListDefinition)`) |

### IR Model Structure

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-PARSE-060 | The `MarkdownBlock` enum shall derive `Debug`, `Clone`, and `PartialEq` to enable test assertions on parsed structure. | Must | A | Constraint | S5.1 Intermediate model definition |
| REQ-MD-PARSE-061 | The `MarkdownInline` struct shall contain fields for: text content, bold flag, italic flag, strikethrough flag, code flag, and optional link URL. | Must | A | Constraint | S5.1 Intermediate model definition |
| REQ-MD-PARSE-062 | The `MarkdownBlock` enum shall include variants for: paragraphs, headings, code blocks, blockquotes, lists, tables, thematic breaks, and image fallbacks. | Must | A | Constraint | S5.1 Intermediate model definition |
| REQ-MD-PARSE-063 | The `Paragraph` and `Heading` variants shall each contain a links collection recording byte ranges and URLs of link spans. | Must | A | Constraint | S5.1 Intermediate model, S2.5 Click Event Precedence |
| REQ-MD-PARSE-064 | The intermediate model types shall have `pub(crate)` visibility -- they are internal implementation details, not part of the public API. | Should | A | Constraint | S5.1 Intermediate model definition |
| REQ-MD-PARSE-065 | IF an unknown or unhandled pulldown-cmark event type is encountered, THEN the parser shall extract any text content and append it as plain text rather than panicking. | Must | A | Behavioral | S9.2 Graceful Fallback for Unsupported Elements |

---

## REQ-MD-RENDER: GPUI Rendering (Phase A)

### Element Mapping

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-RENDER-001 | WHEN the renderer receives a `MarkdownBlock::Paragraph`, the system shall produce a styled text element wrapped in a paragraph div with vertical margin. | Must | A | Behavioral | S5.1 blocks_to_elements() Rendering table |
| REQ-MD-RENDER-002 | WHEN the paragraph contains links (non-empty `links` field), the renderer shall produce an interactive text element with click handlers for each link range, instead of plain styled text. | Must | A | Behavioral | S5.1 blocks_to_elements(), S4.3 InteractiveText |
| REQ-MD-RENDER-003 | WHEN the renderer receives a `MarkdownBlock::Heading`, the system shall produce a text element with font size scaled by heading level and bold font weight. | Must | A | Behavioral | S5.1 blocks_to_elements() Rendering table, S7 Font Tokens |
| REQ-MD-RENDER-004 | The renderer shall apply heading sizes as follows: H1 = 24.0px, H2 = 20.0px, H3 = 18.0px, H4 = 16.0px, H5 = 14.0px, H6 = 13.0px. | Must | A | Constraint | S7 Font Tokens table |
| REQ-MD-RENDER-005 | WHEN the renderer receives a `MarkdownBlock::CodeBlock`, the system shall produce a div with `Theme::bg_darker()` background, rounded corners, and monospace font family. | Must | A | Behavioral | S5.1 blocks_to_elements() Rendering table, S7 Color Token Mapping |
| REQ-MD-RENDER-006 | WHEN a code block has a non-None `language`, the renderer shall display a language label in `Theme::text_muted()` color. | Should | A | Behavioral | S5.1 blocks_to_elements() Rendering table, S7 Color Token Mapping |
| REQ-MD-RENDER-007 | WHEN the renderer receives a `MarkdownBlock::BlockQuote`, the system shall produce a div with a left border styled with `Theme::accent()` and background `Theme::bg_base()`, with children recursively rendered. | Must | A | Behavioral | S5.1 blocks_to_elements() Rendering table, S7 Color Token Mapping |
| REQ-MD-RENDER-008 | WHEN the renderer receives a `MarkdownBlock::List`, the system shall produce a div per item with depth-based indentation and appropriate bullet character (unordered) or number prefix (ordered). | Must | A | Behavioral | S5.1 blocks_to_elements() Rendering table |
| REQ-MD-RENDER-009 | WHEN the renderer receives a `MarkdownBlock::Table`, the system shall produce a CSS grid container where the column count matches the header row, with header cells, body cells, cell borders, optional header background, and alternating row striping. | Must | A | Behavioral | S5.1 blocks_to_elements() Rendering table, S4.4 CSS Grid |
| REQ-MD-RENDER-010 | WHEN the renderer receives a `MarkdownBlock::ThematicBreak`, the system shall produce a horizontal rule element with `Theme::border()` color. | Must | A | Behavioral | S5.1 blocks_to_elements() Rendering table, S7 Color Token Mapping |
| REQ-MD-RENDER-011 | WHEN the renderer receives a `MarkdownBlock::ImageFallback`, the system shall produce styled text `[image: {alt}]` in `Theme::text_muted()` color. | Must | A | Behavioral | S5.1 blocks_to_elements() Rendering table, S7 Color Token Mapping |

### Inline Style Rendering

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-RENDER-020 | WHEN rendering a `MarkdownInline` span with `bold: true`, the renderer shall produce a text run with bold font weight. | Must | A | Behavioral | S4.5 TextRun, S4.6 Font |
| REQ-MD-RENDER-021 | WHEN rendering a `MarkdownInline` span with `italic: true`, the renderer shall produce a text run with italic font style. | Must | A | Behavioral | S4.5 TextRun, S4.6 Font |
| REQ-MD-RENDER-022 | WHEN rendering a `MarkdownInline` span with `strikethrough: true`, the renderer shall produce a text run with strikethrough styling in `Theme::text_muted()` color. | Must | A | Behavioral | S4.5 TextRun, S4.7 StrikethroughStyle, S7 Color Token Mapping |
| REQ-MD-RENDER-023 | WHEN rendering a `MarkdownInline` span with `code: true`, the renderer shall produce a text run with monospace font family and `Theme::bg_darker()` background color. | Must | A | Behavioral | S4.5 TextRun, S7 Color Token Mapping |
| REQ-MD-RENDER-024 | WHEN rendering a `MarkdownInline` span with `link_url: Some(url)`, the renderer shall produce a text run with underline style and `Theme::accent()` color. | Must | A | Behavioral | S4.5 TextRun, S7 Color Token Mapping |
| REQ-MD-RENDER-025 | WHEN rendering list items, the renderer shall style bullet and number prefixes in `Theme::text_muted()` color. | Should | A | Behavioral | S7 Color Token Mapping |

### Font Fallback

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-RENDER-026 | WHEN the renderer specifies a monospace font family for code blocks and inline code, the system shall rely on GPUI's built-in font fallback chain if the specified font is unavailable on the host system. No application-level fallback list is required. | Should | A | Constraint | S7 Font Tokens, Review finding #9 |

### Theme Integration

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-RENDER-030 | The markdown renderer shall source all colors exclusively from `Theme::*` static methods. No hardcoded hex values or `rgb(0x...)` literals shall appear in markdown rendering code. | Must | A | Constraint | S7 Theme Integration |
| REQ-MD-RENDER-031 | The markdown renderer shall use the color token mapping defined in S7: body text -> `Theme::text_primary()`, code block bg -> `Theme::bg_darker()`, blockquote border -> `Theme::accent()`, link text -> `Theme::accent()`, table header bg -> `Theme::bg_dark()`, table border -> `Theme::border()`, table alt row -> `Theme::bg_base()`, thematic break -> `Theme::border()`, muted elements -> `Theme::text_muted()`. | Must | A | Constraint | S7 Color Token Mapping (full table) |
| REQ-MD-RENDER-032 | The markdown renderer shall not require any new `Theme` methods -- all color needs shall be covered by existing accessors: `bg_base()`, `bg_dark()`, `bg_darker()`, `text_primary()`, `text_muted()`, `accent()`, `border()`. | Should | A | Constraint | S7 New Theme Methods Required |
| REQ-MD-RENDER-033 | The markdown renderer shall use body text at `FONT_SIZE_MD` (13.0px), system UI font family, normal weight as the default text style. | Must | A | Constraint | S7 Font Tokens table |

### Public API

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-RENDER-040 | The system shall provide a public function that composes parsing and element rendering into a single call, accepting markdown content and returning GPUI elements. | Must | A | Behavioral | S5.1 Public API |
| REQ-MD-RENDER-041 | IF the input content is empty or whitespace-only, THEN the markdown rendering function shall return an empty element collection without panicking. | Must | A | Behavioral | S9.3 Empty Content |
| REQ-MD-RENDER-042 | The markdown rendering function and its module shall be exported from the components module. | Must | A | Constraint | S5.4 Modifications to `components/mod.rs` |
| REQ-MD-RENDER-043 | The markdown rendering function shall be a pure view-layer transformation -- it shall not read from or write to the store or presenter layers. | Must | A | Constraint | S10.2, S10.3, S3.1 |

### Table Rendering Detail

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-RENDER-050 | WHEN rendering a table, the renderer shall produce a CSS grid container where the column count equals the number of columns in the header row. | Must | A | Behavioral | S4.4 CSS Grid, S5.1 blocks_to_elements() |
| REQ-MD-RENDER-051 | WHEN rendering table header cells, the renderer shall apply `Theme::bg_dark()` background. | Should | A | Behavioral | S7 Color Token Mapping |
| REQ-MD-RENDER-052 | WHEN rendering table body rows, the renderer shall apply alternating row striping using `Theme::bg_base()` on even rows. | Should | A | Behavioral | S7 Color Token Mapping, S2.1 Block-Level Constructs |
| REQ-MD-RENDER-053 | WHEN rendering table cells, the renderer shall apply `Theme::border()` for cell borders. | Should | A | Behavioral | S7 Color Token Mapping |

---

## REQ-MD-INTEGRATE: Integration (Phase A)

### AssistantBubble as Canonical Owner

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-INTEGRATE-001 | The system shall route all assistant content -- both completed messages and streaming messages -- through a single canonical markdown rendering owner. | Must | A | Constraint | S3.4 Canonical Rendering Owner, S5.2 |
| REQ-MD-INTEGRATE-002 | WHEN the assistant bubble renders, the system shall parse markdown from the content text and produce GPUI elements, replacing the current raw string rendering pattern. | Must | A | Behavioral | S5.2 Modifications to message_bubble.rs |
| REQ-MD-INTEGRATE-003 | The `AssistantBubble` struct shall gain no new public fields -- its external API shall remain unchanged. | Must | A | Constraint | S5.2, S10.4 AssistantBubble Compatibility |

### render_assistant_message Delegation

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-INTEGRATE-010 | The completed-message rendering path shall delegate to the canonical assistant bubble rather than building its own raw-text div, eliminating the dual rendering path. | Must | A | Constraint | S5.3 Modifications to render.rs |
| REQ-MD-INTEGRATE-011 | WHEN the completed-message rendering path delegates to the assistant bubble, the system shall pass model identity, thinking content, and show-thinking preference to the bubble builder. | Must | A | Behavioral | S5.3 After code example |
| REQ-MD-INTEGRATE-012 | WHEN the refactored completed-message rendering path produces output through the assistant bubble, the system shall normalize width, padding, gap, text size, and cursor style to match the existing completed-message visual baseline. Any intentional divergences (e.g., streaming cursor) shall be gated on streaming state. | Must | A | Behavioral | S1 Width/Styling Divergence Note |

### Model Label Fallback

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-INTEGRATE-015 | WHEN the model identifier for an assistant message is absent, the system shall render the model label as `"Assistant"` to preserve the current baseline visual behavior. | Must | A | Behavioral | S1, S5.3, Review finding #4 |

### Click-to-Copy Behavior

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-INTEGRATE-020 | WHEN rendered markdown contains no links (all blocks have empty `links` fields), the assistant bubble's outermost container div shall have a click handler that copies the raw markdown source to the clipboard, and pointer cursor style. | Must | A | Behavioral | S2.4 Click-to-Copy Behavior, Top-Level Preconditions (Normative Click Handling) |
| REQ-MD-INTEGRATE-021 | WHEN rendered markdown contains one or more links (any block has a non-empty `links` field), the assistant bubble's outermost container div shall NOT have a click handler and shall NOT have pointer cursor style. Interactive text click handlers handle link clicks exclusively. | Must | A | Behavioral | S2.4, S2.5, Top-Level Preconditions (Normative Click Handling) |
| REQ-MD-INTEGRATE-022 | WHILE streaming is active, the assistant bubble shall NOT attach a bubble-level click handler regardless of link content. | Must | A | Behavioral | S2.4 Click-to-Copy Behavior (streaming messages) |
| REQ-MD-INTEGRATE-023 | The click-to-copy handler shall copy the raw markdown source string (not the rendered text) to the system clipboard. | Must | A | Behavioral | S2.4, S2.5 Implementation code |
| REQ-MD-INTEGRATE-024 | The link detection in the assistant bubble shall recursively inspect the parsed `MarkdownBlock` IR for non-empty `links` fields across any descendant block -- including but not limited to paragraphs, headings, list items, blockquote children, and table cells. | Must | A | Behavioral | S2.5 Implementation code, Review finding #5 |

### User Message Handling

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-INTEGRATE-030 | The system shall render user messages as raw text with no markdown processing. | Must | A | Behavioral | S2.2 User Messages |

### Streaming Cursor

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-INTEGRATE-040 | WHILE streaming is active, the assistant bubble shall append the streaming cursor character (U+258B) to the content string before passing it to markdown parsing. | Must | A | Behavioral | S5.2, S6.7 Cursor Invariants |
| REQ-MD-INTEGRATE-041 | The system shall ensure the streaming cursor character never appears in committed block text, finalized message content, or any persisted state -- it shall exist only in the render path while streaming is active. | Must | A | Behavioral | S6.7 Cursor Invariants |

### Store Layer Independence

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-INTEGRATE-050 | The markdown rendering implementation shall not modify the store layer. | Must | A | Constraint | S10.2 Store Layer -- No Changes |
| REQ-MD-INTEGRATE-051 | The markdown rendering implementation shall not modify the presenter layer. | Must | A | Constraint | S10.3 Presenter Layer -- No Changes |

### Implementation Boundary Constraints

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-INTEGRATE-061 | [Phase B -- Conditional] WHERE Phase B is active, streaming state fields and their helper methods shall reside in the chat view module. They shall not leak into store or presenter layers. | Must | B | Constraint | S17 Files Touched (Phase B), S6.1, S10.2, S10.3, Review finding #1 |

### Phase Isolation Guard

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-INTEGRATE-070 | WHILE only Phase A is deployed, no production code path shall reference the `mdstream` crate or any Phase B dependency. Phase B imports and logic shall not exist in the codebase until Phase B is explicitly activated. | Must | A | Constraint | S3.3, Review finding #6 |

---

## REQ-MD-STREAM: Streaming (Phase B -- Conditional)

> **[Phase B -- Conditional] All requirements in this section are CONDITIONAL DRAFT -- pending mdstream validation gate (S4.9).** Phase A is fully self-contained without any requirements from this section. See also: Phase B Inactivity Rule at the top of this document.

### mdstream Integration

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-STREAM-001 | [Phase B -- Conditional] WHERE Phase B is active, the system shall place the streaming markdown state as a view-local field on the chat view, not in the store. The store shall remain rendering-agnostic. | Must | B | Constraint | S6.1 MdStream Placement |
| REQ-MD-STREAM-002 | [Phase B -- Conditional] WHERE Phase B is active, the chat view shall maintain streaming markdown state, a feed-offset tracker, and a finalization flag. | Must | B | Constraint | S6.1 MdStream Placement |
| REQ-MD-STREAM-003 | [Phase B -- Conditional] WHERE Phase B is active, the system shall add `mdstream = "= 0.2.0"` to `Cargo.toml` and verify that the crate compiles and its API surface matches S4.9 assumptions via a minimal integration test. | Must | B | Constraint | S4.9 Dependency Validation Gate |
| REQ-MD-STREAM-004 | [Phase B -- Conditional] IF the mdstream API diverges cosmetically from S4.9 assumptions but is structurally compatible, THEN the system shall provide a thin adapter module normalizing the actual API to the expected interface. | Must | B | Behavioral | Top-Level Preconditions (Normative Fallback -- option 1) |
| REQ-MD-STREAM-005 | [Phase B -- Conditional] IF the mdstream crate fails to compile or diverges structurally from S4.9 assumptions, THEN the system shall implement a minimal custom block splitter using pulldown-cmark block-level event boundaries directly. | Must | B | Behavioral | Top-Level Preconditions (Normative Fallback -- options 2 & 3) |

### Delta Feeding

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-STREAM-010 | [Phase B -- Conditional] WHILE streaming is active, the system shall compute the new delta bytes since the last feed (by comparing the stream buffer length against the feed-offset tracker), feed only the delta to the streaming markdown parser, and shall perform this feeding as part of store-driven transition handling. | Must | B | Behavioral | S6.6 Delta Feeding with UTF-8 Safety, S6.4 Authoritative Transition Handler Location |
| REQ-MD-STREAM-011 | [Phase B -- Conditional] The delta-feeding code shall use `assert!` (not `debug_assert!`) to verify that the feed offset is a valid char boundary in the stream buffer before slicing. | Must | B | Constraint | S6.6, S9.1 Assert Panic Policy, S14 Item 1. _Note: This is the normative statement of the char-boundary invariant._ |
| REQ-MD-STREAM-012 | [Phase B -- Conditional] The `assert!` message shall include the feed offset and buffer length for diagnostic purposes. | Should | B | Constraint | S6.6 code example |

### Committed/Pending Block Split

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-STREAM-020 | [Phase B -- Conditional] WHILE streaming is active and the streaming markdown parser is producing output, the system shall render committed blocks via the markdown rendering function for each committed block. | Must | B | Behavioral | S6.8 Rendering from mdstream Output |
| REQ-MD-STREAM-021 | [Phase B -- Conditional] WHILE streaming is active, the system shall render the pending block via the markdown rendering function using the pending block's display content, re-parsing it each frame. | Must | B | Behavioral | S6.8 Rendering from mdstream Output |
| REQ-MD-STREAM-022 | [Phase B -- Conditional] The system shall concatenate committed block elements and pending block elements as children of the streaming message div. | Must | B | Behavioral | S6.8 Rendering from mdstream Output |

### Finalization and Reset

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-STREAM-030 | [Phase B -- Conditional] WHEN the stream completes normally (transition from streaming to idle with new message in transcript), the system shall finalize and reset the streaming markdown state, committing any trailing pending content before resetting. | Must | B | Behavioral | S6.5 Finalization Transition Table (F1, F7), S6.9 |
| REQ-MD-STREAM-031 | [Phase B -- Conditional] WHEN the user explicitly aborts streaming (Escape key or Stop button), the system shall reset the streaming markdown state WITHOUT finalizing -- partial content is discarded. | Must | B | Behavioral | S6.5 Finalization Transition Table (F2, F3) |
| REQ-MD-STREAM-032 | [Phase B -- Conditional] WHEN a conversation switch occurs during streaming, the system shall reset the streaming markdown state without finalizing. | Must | B | Behavioral | S6.5 Finalization Transition Table (F4) |
| REQ-MD-STREAM-033 | [Phase B -- Conditional] WHEN a new conversation is started during streaming, the system shall reset the streaming markdown state without finalizing. | Must | B | Behavioral | S6.5 Finalization Transition Table (F5) |
| REQ-MD-STREAM-034 | [Phase B -- Conditional] WHEN a stream error occurs, the system shall reset the streaming markdown state without finalizing. | Must | B | Behavioral | S6.5 Finalization Transition Table (F6) |
| REQ-MD-STREAM-035 | [Phase B -- Conditional] WHEN the conversation-cleared command is received, the system shall reset the streaming markdown state without finalizing. | Must | B | Behavioral | S6.5 Finalization Transition Table (F8) |
| REQ-MD-STREAM-036 | [Phase B -- Conditional] The streaming markdown reset operation shall clear the parser state, set the feed-offset tracker to zero, and set the finalization flag to false. It shall be idempotent (safe to call multiple times). | Must | B | Behavioral | S6.4 Canonical reset helper |
| REQ-MD-STREAM-037 | [Phase B -- Conditional] The finalize-and-reset operation shall call finalize only if the finalization flag is false, then set the flag to true, then perform a full reset. This shall ensure finalize is called at most once per stream lifecycle. | Must | B | Behavioral | S6.4 Canonical reset helper, S6.5 Key principle |
| REQ-MD-STREAM-038 | [Phase B -- Conditional] IF the feed-offset tracker is non-zero when a new stream starts, THEN the system shall log a warning and reset the streaming markdown state as a defensive guard against stale state. | Should | B | Behavioral | S6.4.1 Transition: Stream Start, S6.5 (F9) |

### Transition Handler Locations

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-STREAM-040 | [Phase B -- Conditional] The system shall handle store-driven transitions (token arrival, stream completion, stream error) within the store-snapshot application path. | Must | B | Constraint | S6.4 Authoritative Transition Handler Location |
| REQ-MD-STREAM-041 | [Phase B -- Conditional] The system shall handle user-action transitions (Escape, Stop button, new conversation, conversation switch, conversation-cleared) in their respective local action hooks immediately after setting the streaming state to idle. | Must | B | Constraint | S6.4 Authoritative Transition Handler Location |
| REQ-MD-STREAM-042 | [Phase B -- Conditional] IF the streaming state is idle or error, THEN the system shall not perform streaming rendering regardless of streaming markdown parser state. The streaming state shall be the sole source of truth for stream lifecycle. | Must | B | Behavioral | S6.3 Streaming State Precedence |
| REQ-MD-STREAM-043 | [Phase B -- Conditional] IF the streaming markdown parser and the streaming state disagree (e.g., parser has pending blocks but streaming state is idle), THEN the system shall reset the parser -- the streaming state takes precedence. | Must | B | Behavioral | S6.3 Streaming State Precedence (rule 4) |

### Phase A Streaming Fallback

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-STREAM-050 | WHILE Phase A is deployed (no mdstream), the system shall invoke the markdown rendering function on the full stream buffer content each frame for streaming messages (O(n^2) behavior is an accepted Phase A limitation). | Must | A | Behavioral | S3.3 Data Flow: Streaming Messages (Phase A) |
| REQ-MD-STREAM-051 | WHILE Phase A is deployed, the system shall emit a `tracing::debug!` log reporting stream buffer length on each streaming frame to provide telemetry for Phase B prioritization (pending validation gate). | Should | A | Constraint | S12 Phase A -> Phase B Transition Decision |

### Table Rendering During Streaming

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-STREAM-060 | [Phase B -- Conditional] WHILE streaming, IF a table header row is incomplete, THEN the system shall treat the partial text as a plain paragraph in the pending block (pulldown-cmark does not emit `Start(Table)` until header + delimiter row are available). | Must | B | Behavioral | S2.1 Table Rendering During Incomplete Streaming, S14 Item 5 |
| REQ-MD-STREAM-061 | [Phase B -- Conditional] WHILE streaming, IF a table header row is complete but body rows are still arriving, THEN the system shall render the table as a grid with known column count and incomplete trailing cells rendered as empty. | Should | B | Behavioral | S2.1 Table Rendering During Incomplete Streaming |

---

## REQ-MD-PERF: Performance

### Phase A Acceptance Criteria

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-PERF-001 | WHEN rendering a completed message of up to ~10KB, the frame-time delta shall be <= 2ms above baseline (baseline = same message count with raw text rendering). | Must | A | Behavioral | S19.1 Phase A table (row 1) |
| REQ-MD-PERF-002 | WHEN rendering a single completed message of up to ~50KB, the frame-time delta shall be <= 8ms above baseline. A single-frame spike on first render is acceptable. | Should | A | Behavioral | S19.1 Phase A table (row 2) |
| REQ-MD-PERF-003 | WHEN rendering a conversation of up to 50 messages, the frame time for 50 messages shall be <= 2x the frame time for 10 messages (for same average message size). | Must | A | Behavioral | S19.1 Phase A table (row 3) |
| REQ-MD-PERF-004 | WHILE streaming responses of up to ~2,000 tokens (Phase A), full re-parse per token is acceptable and the frame-time delta shall be <= 2ms above baseline during steady-state streaming. | Must | A | Behavioral | S19.1 Phase A table (row 4) |
| REQ-MD-PERF-005 | WHILE streaming responses of 5,000-10,000 tokens (Phase A), the frame-time delta at 10,000 tokens shall be documented to inform Phase B prioritization (pending validation gate). This is not a pass/fail gate. | Should | A | Behavioral | S19.1 Phase A table (row 5) |
| REQ-MD-PERF-006 | The memory delta per completed message shall be <= 2x the raw string size (accounting for intermediate model + GPUI elements). The system shall exhibit no unbounded memory growth. | Must | A | Behavioral | S19.1 Phase A table (row 6) |

### Performance Measurement Protocol

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-PERF-007 | Performance acceptance tests shall use a fixed set of fixture conversations: (a) 10 messages averaging ~1KB each, (b) a single ~50KB message, (c) 50 messages averaging ~1KB each. These fixtures shall be committed to the test suite for reproducibility. | Must | A | Constraint | S19.1, Review finding #3 |
| REQ-MD-PERF-008 | Performance measurements shall be taken from release builds (`cargo build --release`) to reflect production performance characteristics. Debug builds shall not be used for pass/fail determination. | Must | A | Constraint | S19.1, Review finding #3 |
| REQ-MD-PERF-009 | Each performance measurement shall be the median of at least 5 runs, with 1 warmup run discarded, to reduce variance from system load and caching effects. | Must | A | Constraint | S19.1, Review finding #3 |
| REQ-MD-PERF-009a | The pass/fail calculation for frame-time delta thresholds (REQ-MD-PERF-001 through REQ-MD-PERF-004) shall be: `median_with_markdown - median_without_markdown <= threshold`. The baseline (`median_without_markdown`) and measured (`median_with_markdown`) values shall both be reported. | Must | A | Constraint | S19.1, Review finding #3 |
| REQ-MD-PERF-009b | Performance tests shall measure GPUI frame timing by capturing `std::time::Instant::now()` immediately before and after the render call under test. The test shall execute 5 measured iterations (after 1 warmup), report the median duration, and use a fixture of 20 messages with 2KB of markdown content each. All measurements shall be taken from release builds (`cargo build --release`). | Must | A | Constraint | S19.1, Review finding #3 |

### Phase B Acceptance Criteria

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-PERF-010 | [Phase B -- Conditional] WHERE Phase B (mdstream) is active, WHILE streaming responses of up to 50,000+ tokens, the frame-time at 50,000 tokens shall be <= 2x the frame-time at 1,000 tokens (O(n) total parsing, no progressive degradation). | Must | B | Behavioral | S19.2 Phase B table (row 1) |
| REQ-MD-PERF-011 | [Phase B -- Conditional] WHERE Phase B is active, the system shall ensure committed blocks stabilize visually -- they shall not flicker or re-layout once committed. | Must | B | Behavioral | S19.2 Phase B table (row 2) |
| REQ-MD-PERF-012 | [Phase B -- Conditional] WHERE Phase B is active, the system shall feed multi-byte UTF-8 tokens through the delta-feeding pipeline without panics. | Must | B | Behavioral | S19.2 Phase B table (row 3) |

### Profiling

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-PERF-020 | IF any Phase A performance threshold is exceeded during manual testing, THEN the system shall be profiled and the bottleneck addressed by either accelerating Phase B (pending validation gate) or adding content-hash caching. See Appendix D for profiling procedure. | Should | A | Constraint | S19.3 Profiling Trigger and Method |

---

## REQ-MD-SEC: Security

### URL Sanitization

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-SEC-001 | WHEN a link click handler fires, the system shall validate the URL via `url::Url::parse()` and allow only `https` and `http` schemes before opening the URL. All other schemes shall be rejected (positive allowlist). | Must | A | Behavioral | S8.1 URL Sanitization Policy, Allowed Schemes |
| REQ-MD-SEC-002 | IF a URL has a rejected scheme (e.g., `javascript:`, `file://`, `data:`), THEN the click handler shall silently do nothing -- no URL shall be opened and no error message shall be shown. | Must | A | Behavioral | S8.1 Rejected Schemes, Click Handler Implementation |
| REQ-MD-SEC-003 | IF a URL fails `url::Url::parse()` (malformed), THEN the system shall still render the link text with link styling (underline, accent color) but clicking shall be a no-op -- no URL shall be opened. | Must | A | Behavioral | S8.1 Malformed URL Handling |
| REQ-MD-SEC-004 | The URL validation shall use the `url` crate (`url::Url::parse`) rather than simple `starts_with` checks, to handle whitespace, control characters, embedded newlines, and scheme case normalization. | Must | A | Constraint | S8.1 URL Parsing and Validation |
| REQ-MD-SEC-005 | The `url` crate (version 2) shall be added to `Cargo.toml` as a Phase A dependency. | Must | A | Constraint | S16 Dependencies (Phase A) |
| REQ-MD-SEC-006 | IF a link URL is a relative URL (i.e., `url::Url::parse()` returns `Err` because there is no scheme), THEN the click handler shall treat it as a no-op -- no URL shall be opened. Relative URLs shall not be resolved against any base. | Must | A | Behavioral | S8.1, Review finding #7 |

### HTML Stripping

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-SEC-010 | The system shall strip all HTML tags from raw HTML in markdown and render only the extracted text content. HTML shall never be interpreted, sanitized-and-rendered, or executed. A single tag-stripping state machine shall implement this behavior for both block-level HTML (REQ-MD-PARSE-043) and inline HTML (REQ-MD-PARSE-044). | Must | A | Behavioral | S8.3 HTML Content, S5.1.1 |
| REQ-MD-SEC-011 | The HTML tag-stripping state machine shall strip `<script>` and `<style>` tags along with their enclosed content -- no text content shall be extracted from script or style elements. | Must | A | Behavioral | S5.1.1 Test coverage bullets |

### UTF-8 Safety

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-SEC-020 | _(Derived -- non-normative cross-reference.)_ [Phase B -- Conditional] WHERE Phase B delta-feeding is active, the char-boundary invariant specified in REQ-MD-STREAM-011 shall apply (see REQ-MD-STREAM-011). REQ-MD-STREAM-011 is the authoritative statement of this requirement. | -- | B | Constraint | S6.6, S9.1 (derived from REQ-MD-STREAM-011) |
| REQ-MD-SEC-021 | [Phase B -- Conditional] IF the char-boundary assertion fails, THEN the system shall panic with a clear diagnostic message including feed offset and buffer length rather than producing garbled text. | Must | B | Behavioral | S6.6 code example, S9.1 |

---

## REQ-MD-TEST: Testing

### Unit Tests for IR Model (`#[test]`)

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-TEST-001 | The test suite shall include `#[test]` tests for the markdown parser verifying each supported inline style: bold, italic, bold-italic, strikethrough, inline code, and links (with `link_url` and `links` field populated). | Must | A | Constraint | S11.1 Inline style tests |
| REQ-MD-TEST-002 | The test suite shall include `#[test]` tests for the markdown parser verifying each supported block type: paragraph, multi-paragraph separation, headings (levels 1-6), fenced code block (with and without language), indented code block, list (ordered and unordered with items), and table (column count, header, body rows). | Must | A | Constraint | S11.1 Block structure tests |
| REQ-MD-TEST-003 | The test suite shall include `#[test]` tests verifying link collection: paragraphs with links produce correct `(Range, url)` tuples in the `links` field, headings with links populate their `links` field, and links in list items are propagated through the list structure. | Must | A | Constraint | S11.1 Link collection tests |
| REQ-MD-TEST-004 | The test suite shall include `#[test]` tests for all graceful fallbacks: images -> `ImageFallback`, block-level HTML -> text extracted with tags stripped, inline HTML -> text preserved in flow, `<script>` tags -> content stripped entirely, math -> code-styled content. | Must | A | Constraint | S11.1 Fallback tests |
| REQ-MD-TEST-005 | The test suite shall include `#[test]` tests for URL sanitization: valid `https`/`http` URLs pass, dangerous schemes (`javascript:`, `file://`, `data:`, `vbscript:`) are rejected, malformed/empty URLs are rejected, whitespace is trimmed, scheme case is insensitive, relative URLs are rejected, IDN URLs pass, percent-encoded URLs pass. | Must | A | Constraint | S11.3 URL Sanitization Tests |

### GPUI Behavioral Tests (`#[gpui::test]`)

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-TEST-010 | The test suite shall include a `#[gpui::test]` verifying that a paragraph containing a link produces an interactive text element (not plain styled text). | Must | A | Constraint | S11.2 Clickable link dispatch |
| REQ-MD-TEST-011 | The test suite shall include a `#[gpui::test]` verifying that a markdown table produces a grid container with the expected number of child cell elements. | Must | A | Constraint | S11.2 Table grid construction |
| REQ-MD-TEST-012 | The test suite shall include a `#[gpui::test]` verifying that a fenced code block produces a container div with monospace-styled text content. | Must | A | Constraint | S11.2 Code block container creation |
| REQ-MD-TEST-013 | The test suite shall include `#[gpui::test]` tests verifying click event precedence: (1) message with links -> bubble-level click handler NOT attached, (2) message without links -> bubble-level click handler IS attached, (3) link click fires only the interactive text handler, (4) non-link click on link-free message fires bubble handler. | Must | A | Constraint | S2.5 Pass/Fail Criteria, S11.2 Click event precedence gate |
| REQ-MD-TEST-014 | The test suite shall include `#[gpui::test]` smoke tests verifying: non-empty element output for basic inputs, no panic on any standard markdown construct, correct element count for simple multi-block inputs. | Must | A | Constraint | S11.2 Smoke tests |

### Streaming Lifecycle Tests (`#[gpui::test]`)

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-TEST-020 | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include tests for delta feeding: feed tokens one-at-a-time and verify committed blocks stabilize. | Must | B | Constraint | S11.4 Delta feeding |
| REQ-MD-TEST-021 | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include a test verifying finalization captures trailing pending content as committed. | Must | B | Constraint | S11.4 Finalization |
| REQ-MD-TEST-022a | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include a test verifying that normal stream completion (F1) triggers finalize-and-reset and produces committed output. | Must | B | Constraint | S6.5 Finalization Transition Table (F1) |
| REQ-MD-TEST-022b | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include a test verifying that Escape key abort (F2) triggers reset without finalize. | Must | B | Constraint | S6.5 Finalization Transition Table (F2) |
| REQ-MD-TEST-022c | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include a test verifying that Stop button abort (F3) triggers reset without finalize. | Must | B | Constraint | S6.5 Finalization Transition Table (F3) |
| REQ-MD-TEST-022d | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include a test verifying that conversation switch during streaming (F4) triggers reset without finalize. | Must | B | Constraint | S6.5 Finalization Transition Table (F4) |
| REQ-MD-TEST-022e | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include a test verifying that new conversation during streaming (F5) triggers reset without finalize. | Must | B | Constraint | S6.5 Finalization Transition Table (F5) |
| REQ-MD-TEST-022f | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include a test verifying that stream error (F6) triggers reset without finalize. | Must | B | Constraint | S6.5 Finalization Transition Table (F6) |
| REQ-MD-TEST-022g | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include a test verifying that idle-inferred completion (F7) triggers finalize-and-reset. | Must | B | Constraint | S6.5 Finalization Transition Table (F7) |
| REQ-MD-TEST-022h | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include a test verifying that conversation-cleared (F8) triggers reset without finalize. | Must | B | Constraint | S6.5 Finalization Transition Table (F8) |
| REQ-MD-TEST-022i | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include a test verifying that a new stream starting with stale state (F9) triggers defensive reset. | Must | B | Constraint | S6.5 Finalization Transition Table (F9) |
| REQ-MD-TEST-023 | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include tests verifying the streaming cursor never appears in committed block text or in finalized message content. | Must | B | Constraint | S6.7 Cursor Invariants, S11.4 Cursor invariant |
| REQ-MD-TEST-024 | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall include tests verifying that the reset operation is safe to call multiple times and finalize-and-reset only calls finalize once per stream (double-reset and double-finalize safety). | Must | B | Constraint | S11.4 Idempotency |

> **Naming convention (non-normative):** Tests for the finalization transition table (REQ-MD-TEST-022a through 022i) are recommended to follow the pattern `test_<action>_on_<trigger>` (e.g., `test_finalize_on_normal_completion`, `test_reset_on_escape`), but exact names are at implementor discretion provided behavioral coverage is met.

### Non-Regression Tests

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-TEST-030 | The test suite shall include a `#[gpui::test]` verifying that pressing Escape during streaming transitions to idle state and emits the stop-streaming event without panic or orphaned elements. | Must | A | Constraint | S11.5 Non-Regression (streaming stop Escape) |
| REQ-MD-TEST-031 | The test suite shall include a `#[gpui::test]` verifying that clicking the Stop button during streaming transitions correctly. | Must | A | Constraint | S11.5 Non-Regression (streaming stop button) |
| REQ-MD-TEST-032 | The test suite shall include a `#[gpui::test]` verifying that switching conversations clears messages, stops active streams, and loads the new conversation's transcript with correctly rendered markdown. | Must | A | Constraint | S11.5 Non-Regression (conversation switching) |
| REQ-MD-TEST-033 | The test suite shall include a `#[gpui::test]` verifying that autoscroll continues to function during streaming after markdown rendering replaces raw text rendering. | Should | A | Constraint | S11.5 Non-Regression (autoscroll) |
| REQ-MD-TEST-034 | The test suite shall include a `#[gpui::test]` verifying that clicking a completed assistant message without links copies its content to the clipboard. | Must | A | Constraint | S11.5 Non-Regression (click-to-copy) |
| REQ-MD-TEST-035 | The test suite shall include a `#[gpui::test]` verifying that creating a new conversation clears all messages and streaming state, and the empty state renders correctly. | Should | A | Constraint | S11.5 Non-Regression (new conversation) |
| REQ-MD-TEST-036 | The test suite shall include a `#[gpui::test]` verifying that empty or whitespace-only assistant messages render as empty bubbles without panic or layout breakage. | Must | A | Constraint | S11.5 Non-Regression (empty messages), S9.3 |

### Edge Case Coverage

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-TEST-040 | The test suite shall verify that empty content produces an empty element vector without panic. | Must | A | Constraint | S11.6 Edge Cases |
| REQ-MD-TEST-041 | The test suite shall verify that whitespace-only content produces an empty element vector or whitespace-only output without panic. | Must | A | Constraint | S11.6 Edge Cases |
| REQ-MD-TEST-042 | The test suite shall verify that unclosed markdown (e.g., `**bold without closing`) produces output without panic (pulldown-cmark handles recovery). | Must | A | Constraint | S11.6 Edge Cases |
| REQ-MD-TEST-043 | [Phase B -- Conditional] WHERE Phase B is active, the test suite shall verify that multi-byte UTF-8 characters split across streaming chunks trigger the char-boundary assertion correctly. | Must | B | Constraint | S11.6 Edge Cases |
| REQ-MD-TEST-044 | The test suite shall verify that deeply nested lists (5+ levels) continue to increase indentation without panic. | Should | A | Constraint | S11.6 Edge Cases |
| REQ-MD-TEST-045 | The test suite shall verify that large tables (20+ columns) render via the grid without panic. | Should | A | Constraint | S11.6 Edge Cases |
| REQ-MD-TEST-046 | The test suite shall verify that very long code blocks render without panic. | Should | A | Constraint | S11.6 Edge Cases |
| REQ-MD-TEST-047 | The test suite shall verify that mixed nested inline styles (e.g., `**bold _bold-italic_**`) produce correctly stacked style flags. | Must | A | Constraint | S11.6 Edge Cases |

### Testing Philosophy Alignment

| ID | Requirement (EARS) | Priority | Phase | Type | Spec Trace |
|---|---|---|---|---|---|
| REQ-MD-TEST-050 | The test suite shall follow the project's `dev-docs/RUST-RULES.md` behavioral testing philosophy: tests shall verify user-visible behavior and semantic contracts, not implementation internals (stack sizes, internal method call order). | Must | A | Constraint | S11.0 Testing Philosophy Alignment, RUST-RULES.md Core Principles |
| REQ-MD-TEST-051 | The test suite shall name tests for the behavior they verify (e.g., `test_bold_text_produces_bold_span`), not for implementation details (e.g., not `test_style_stack_push_pop`). Specific names are recommended conventions, not mandates -- behavioral coverage is the binding obligation. | Should | A | Constraint | S11.0, RUST-RULES.md, Review finding #11 |
| REQ-MD-TEST-052 | The test suite shall use real `pulldown_cmark::Parser` output and real GPUI contexts (for `#[gpui::test]`) -- no mocks of internal components. | Must | A | Constraint | S11.0, RUST-RULES.md Core Principles S3 |
| REQ-MD-TEST-053 | The testing pyramid shall be weighted toward the testable intermediate model layer (`#[test]`) with targeted `#[gpui::test]` behavioral tests for key interactive and structural behaviors. | Should | A | Constraint | S11.7 Test Framework |

---

## Appendix A: Requirement Summary Counts

| Group | Must | Should | Could | Total | Phase A | Phase B | Behavioral | Constraint |
|---|---|---|---|---|---|---|---|---|
| REQ-MD-PARSE | 22 | 6 | 1 | 29 | 29 | 0 | 23 | 6 |
| REQ-MD-RENDER | 18 | 7 | 0 | 25 | 25 | 0 | 19 | 6 |
| REQ-MD-INTEGRATE | 15 | 0 | 0 | 15 | 13 | 2 | 10 | 5 |
| REQ-MD-STREAM | 23 | 3 | 0 | 26 | 2 | 24 | 19 | 7 |
| REQ-MD-PERF | 9 | 4 | 0 | 13 | 11 | 2 | 9 | 4 |
| REQ-MD-SEC | 7 | 1 | 0 | 8 | 6 | 2 | 6 | 2 |
| REQ-MD-TEST | 26 | 5 | 0 | 31 | 22 | 9 | 0 | 31 |
| **Total** | **120** | **26** | **1** | **147** | **108** | **39** | **86** | **61** |

> _Note: REQ-MD-SEC-020 is a non-normative cross-reference and is not counted in the Must/Should/Could tallies. REQ-MD-INTEGRATE-060 was demoted to Appendix D implementation guidance in Revision 3. All REQ-MD-TEST requirements are classified as Constraint because they prescribe specific test coverage rather than user-visible behavior._

## Appendix B: Phase Allocation

- **Phase A (108 requirements):** All parsing, rendering, integration, security, and testing requirements for completed and streaming message display via the two-phase IR pipeline. Fully self-contained -- no mdstream dependency. Includes phase isolation guard (REQ-MD-INTEGRATE-070), performance measurement protocol, and model label fallback.
- **Phase B (39 requirements):** [Phase B -- Conditional] Streaming optimization via mdstream or fallback block splitter. Conditional on Dependency Validation Gate (see Phase B Inactivity Rule). All finalization, reset matrix, delta-feeding, and streaming lifecycle requirements. Includes streaming state file-boundary constraint (REQ-MD-INTEGRATE-061).

## Appendix C: Traceability to Non-Goals

The following items are explicitly **out of scope** per the specification and have no requirements:

| Non-Goal | Spec Reference |
|---|---|
| Syntax highlighting of code blocks | Non-Goals, S13 |
| Interactive task-list checkboxes | Non-Goals, S14 Item 10 |
| Markdown rendering for user messages | S2.2 (intentional decision) |
| Code block copy buttons | S13 (Phase 2) |
| Image rendering | Non-Goals, S13 |
| Math/LaTeX rendering (beyond code-style fallback) | Non-Goals |
| Bubble copy affordance for link-containing messages | S13 (Phase 2) |
| Completed message parse caching | S13 (Phase 2) |
| Scroll anchoring beyond existing autoscroll | S13 (Phase 2) |

## Appendix D: Implementation Notes (Non-Normative)

> **This appendix is non-normative.** It collects process guidance, workflow steps, and sizing estimates referenced by requirements but not themselves testable system behaviors. No content in this appendix constitutes a binding requirement.

### S1. Dependency Validation Gate Procedure (REQ-MD-STREAM-003)

1. Add `mdstream = "= 0.2.0"` to `Cargo.toml`.
2. Run `cargo check` to verify compilation.
3. Write a minimal integration test validating the API surface matches S4.9 assumptions.
4. If deviations are found, update the spec before proceeding.
5. Document the result. Phase B requirements remain inactive until this gate passes (see Phase B Inactivity Rule).

### S2. Adapter/Fallback Sizing Guidance (REQ-MD-STREAM-004, REQ-MD-STREAM-005)

- **Cosmetic API adapter** (REQ-MD-STREAM-004): expected ~50-100 lines in an adapter module.
- **Custom block splitter fallback** (REQ-MD-STREAM-005): expected ~100-200 lines using pulldown-cmark block-level event boundaries.

### S3. Performance Profiling Procedure (REQ-MD-PERF-020)

If any Phase A performance threshold is exceeded during manual testing, profile with `cargo instruments` to identify the bottleneck. Based on findings, either accelerate Phase B implementation (pending validation gate) or add content-hash caching to the completed-message render path.

### S4. Telemetry for Phase B Prioritization (REQ-MD-STREAM-051)

The `tracing::debug!` log for stream buffer length should use target `markdown::streaming` and include the key `stream_buffer_len` for structured log querying.

### S5. Phase A File-Touch Guidance (formerly REQ-MD-INTEGRATE-060)

> _Demoted from normative requirement in Revision 3. This is implementation guidance, not a binding constraint._

Phase A changes are expected to be confined to: `Cargo.toml`, `components/mod.rs`, `components/markdown_content.rs` (new), `components/message_bubble.rs`, and `views/chat_view/render.rs`. However, legitimate refactors that serve behavioral requirements may touch additional files. The binding obligation is behavioral correctness per the normative requirements above, not a strict file-touch list.

---

## Appendix E: Revision History

| Revision | Date | Changes |
|---|---|---|
| 1 | 2026-04-02 | Initial requirements derived from overview.md (post-review round 4). |
| 2 | 2026-04-02 | Revised per requirements-review.md findings: (1) added REQ-MD-INTEGRATE-060/061 for file-boundary constraints from S17; (2) split compound requirements REQ-MD-STREAM-037->037a/b/c, REQ-MD-TEST-022->022a-i, REQ-MD-STREAM-010->010a/b/c into atomic EARS clauses; (3) added REQ-MD-PERF-007/008/009/009a for performance measurement protocol; (4) added REQ-MD-INTEGRATE-015 for model label "Assistant" fallback; (5) broadened REQ-MD-INTEGRATE-024 link detection to any descendant block; (6) added REQ-MD-INTEGRATE-070 phase isolation guard; (7) added REQ-MD-SEC-006 for explicit relative URL click behavior; (8) deduplicated REQ-MD-SEC-020/REQ-MD-STREAM-011 with cross-reference annotations; (9) added REQ-MD-RENDER-026 for Menlo font fallback behavior; (10) marked Appendix D explicitly as non-normative; (11) relaxed exact test-name mandates to behavioral obligations with recommended naming convention (REQ-MD-TEST-051, REQ-MD-TEST-022 note). Updated counts in Appendix A. |
| 3 | 2026-04-02 | Polish pass per requirements-review.md: (1) demoted REQ-MD-INTEGRATE-060 from normative to Appendix D S5 implementation guidance; (2) converted REQ-MD-SEC-020 to non-normative cross-reference; (3) added REQ-MD-PERF-009b with measurable perf test protocol; (4) normalized PARSE-061/062/063 and consolidated STREAM-037a/b/c->037; (5) consolidated STREAM-010a/b/c back into single REQ-MD-STREAM-010; (6) softened method-name references; (7) added global Phase B Inactivity Rule. |
| 4 | 2026-04-02 | Comprehensive-review reconciliation: (1) harmonized IR schema -- List.start is `u64` (not `Option<u64>`) per canonical spec definition in overview.md S5.1, updated REQ-MD-PARSE-007/008; (2) added Type column classifying each requirement as Behavioral or Constraint per review finding B.2; (3) added "[Phase B -- Conditional]" prefix to all Phase B requirement texts for unambiguous identification; (4) updated Appendix A summary counts to include Behavioral/Constraint breakdown; (5) added "(pending validation gate)" suffix to Phase B cross-references in Phase A requirements. |
